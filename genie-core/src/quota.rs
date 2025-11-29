//! Quota tracking and enforcement.
//!
//! This module provides SQLite-based usage tracking and
//! quota enforcement for Genie → Gemini calls.

use crate::config::QuotaConfig;
use crate::model::RequestKind;
use chrono::{DateTime, Local, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Errors related to quota management
#[derive(Debug, Error)]
pub enum QuotaError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Minute quota exceeded: {current}/{limit} requests in the last minute")]
    MinuteQuotaExceeded { current: u32, limit: u32 },

    #[error("Daily quota exceeded: {current}/{limit} requests today")]
    DailyQuotaExceeded { current: u32, limit: u32 },

    #[error("Failed to initialize database: {0}")]
    InitializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// A usage event recorded in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageEvent {
    #[serde(default)]
    pub id: Option<i64>,
    pub timestamp: String,
    pub model: String,
    pub kind: String,
    pub prompt_chars: i32,
    pub response_chars: i32,
    pub approx_input_tokens: i32,
    pub approx_output_tokens: i32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl UsageEvent {
    /// Create a new usage event
    pub fn new(
        model: impl Into<String>,
        kind: RequestKind,
        prompt_chars: usize,
        response_chars: usize,
        success: bool,
    ) -> Self {
        let prompt_chars = prompt_chars as i32;
        let response_chars = response_chars as i32;

        Self {
            id: None,
            timestamp: Utc::now().to_rfc3339(),
            model: model.into(),
            kind: kind.to_string(),
            prompt_chars,
            response_chars,
            approx_input_tokens: prompt_chars / 4,
            approx_output_tokens: response_chars / 4,
            success,
            error_code: None,
        }
    }

    /// Add an error code to the event
    pub fn with_error(mut self, error_code: impl Into<String>) -> Self {
        self.error_code = Some(error_code.into());
        self.success = false;
        self
    }
}

/// Usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    pub requests_today: u32,
    pub requests_last_minute: u32,
    pub input_tokens_today: u32,
    pub output_tokens_today: u32,
    pub last_error: Option<String>,
}

/// Manages quota tracking and enforcement
pub struct QuotaManager {
    pool: SqlitePool,
    config: Arc<RwLock<QuotaConfig>>,
    last_error: Arc<RwLock<Option<String>>>,
}

impl QuotaManager {
    /// Initialize the quota manager with a database at the given path
    #[instrument(skip_all)]
    pub async fn new(db_path: &Path, config: QuotaConfig) -> Result<Self, QuotaError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                QuotaError::InitializationError(format!("Failed to create directory: {}", e))
            })?;
        }

        // Create connection URL
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        debug!("Connecting to SQLite database at: {}", db_path.display());
        let pool = SqlitePool::connect(&db_url).await?;

        // Run migrations
        Self::init_schema(&pool).await?;

        info!("Quota manager initialized successfully");

        Ok(Self {
            pool,
            config: Arc::new(RwLock::new(config)),
            last_error: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize the database schema
    async fn init_schema(pool: &SqlitePool) -> Result<(), QuotaError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS usage_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                model TEXT NOT NULL,
                kind TEXT NOT NULL,
                prompt_chars INTEGER NOT NULL,
                response_chars INTEGER NOT NULL,
                approx_input_tokens INTEGER NOT NULL,
                approx_output_tokens INTEGER NOT NULL,
                success BOOLEAN NOT NULL,
                error_code TEXT
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create index for faster time-based queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_usage_timestamp 
            ON usage_events(timestamp)
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update the quota configuration
    pub async fn update_config(&self, config: QuotaConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Get the current quota configuration
    pub async fn get_config(&self) -> QuotaConfig {
        self.config.read().await.clone()
    }

    /// Check if a request is allowed under current quotas
    #[instrument(skip(self))]
    pub async fn check_before_request(
        &self,
        kind: &RequestKind,
        model: &str,
    ) -> Result<(), QuotaError> {
        let config = self.config.read().await;

        // Check minute quota
        let minute_count = self.count_requests_last_minute().await?;
        if minute_count >= config.per_minute {
            warn!(
                "Minute quota exceeded: {}/{} for {}",
                minute_count, config.per_minute, kind
            );
            return Err(QuotaError::MinuteQuotaExceeded {
                current: minute_count,
                limit: config.per_minute,
            });
        }

        // Check daily quota
        let daily_count = self.count_requests_today(&config.reset_time).await?;
        if daily_count >= config.per_day {
            warn!(
                "Daily quota exceeded: {}/{} for {}",
                daily_count, config.per_day, kind
            );
            return Err(QuotaError::DailyQuotaExceeded {
                current: daily_count,
                limit: config.per_day,
            });
        }

        debug!(
            "Quota check passed: minute={}/{}, day={}/{}, model={}",
            minute_count, config.per_minute, daily_count, config.per_day, model
        );

        Ok(())
    }

    /// Record a usage event after a request
    #[instrument(skip(self, event))]
    pub async fn record_event(&self, event: UsageEvent) -> Result<i64, QuotaError> {
        // Update last error if this was a failure
        if !event.success {
            let mut last_error = self.last_error.write().await;
            *last_error = event.error_code.clone();
        }

        let result = sqlx::query(
            r#"
            INSERT INTO usage_events 
                (timestamp, model, kind, prompt_chars, response_chars, 
                 approx_input_tokens, approx_output_tokens, success, error_code)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&event.timestamp)
        .bind(&event.model)
        .bind(&event.kind)
        .bind(event.prompt_chars)
        .bind(event.response_chars)
        .bind(event.approx_input_tokens)
        .bind(event.approx_output_tokens)
        .bind(event.success)
        .bind(&event.error_code)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        debug!("Recorded usage event with id: {}", id);

        Ok(id)
    }

    /// Count requests in the last minute
    pub async fn count_requests_last_minute(&self) -> Result<u32, QuotaError> {
        let one_minute_ago = Utc::now() - chrono::Duration::minutes(1);
        let timestamp = one_minute_ago.to_rfc3339();

        let result: (i32,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM usage_events 
            WHERE timestamp >= ? AND success = 1
            "#,
        )
        .bind(timestamp)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.0 as u32)
    }

    /// Count requests since the daily reset time
    pub async fn count_requests_today(&self, reset_time: &str) -> Result<u32, QuotaError> {
        let reset_timestamp = self.calculate_reset_timestamp(reset_time)?;

        let result: (i32,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM usage_events 
            WHERE timestamp >= ? AND success = 1
            "#,
        )
        .bind(reset_timestamp)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.0 as u32)
    }

    /// Get token usage for today
    pub async fn tokens_today(&self, reset_time: &str) -> Result<(u32, u32), QuotaError> {
        let reset_timestamp = self.calculate_reset_timestamp(reset_time)?;

        let result: (i64, i64) = sqlx::query_as(
            r#"
            SELECT 
                COALESCE(SUM(approx_input_tokens), 0),
                COALESCE(SUM(approx_output_tokens), 0)
            FROM usage_events 
            WHERE timestamp >= ? AND success = 1
            "#,
        )
        .bind(reset_timestamp)
        .fetch_one(&self.pool)
        .await?;

        Ok((result.0 as u32, result.1 as u32))
    }

    /// Get recent usage events
    pub async fn get_recent_events(&self, limit: u32) -> Result<Vec<UsageEvent>, QuotaError> {
        let events = sqlx::query_as::<_, UsageEvent>(
            r#"
            SELECT id, timestamp, model, kind, prompt_chars, response_chars,
                   approx_input_tokens, approx_output_tokens, success, error_code
            FROM usage_events 
            ORDER BY timestamp DESC 
            LIMIT ?
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get comprehensive usage statistics
    pub async fn get_stats(&self) -> Result<UsageStats, QuotaError> {
        let config = self.config.read().await;

        let requests_today = self.count_requests_today(&config.reset_time).await?;
        let requests_last_minute = self.count_requests_last_minute().await?;
        let (input_tokens, output_tokens) = self.tokens_today(&config.reset_time).await?;
        let last_error = self.last_error.read().await.clone();

        Ok(UsageStats {
            requests_today,
            requests_last_minute,
            input_tokens_today: input_tokens,
            output_tokens_today: output_tokens,
            last_error,
        })
    }

    /// Calculate the timestamp for the daily reset
    fn calculate_reset_timestamp(&self, reset_time: &str) -> Result<String, QuotaError> {
        // Parse reset time (HH:MM format)
        let reset_time = NaiveTime::parse_from_str(reset_time, "%H:%M").map_err(|e| {
            QuotaError::ConfigError(format!("Invalid reset_time format '{}': {}", reset_time, e))
        })?;

        let now = Local::now();
        let today_reset = now.date_naive().and_time(reset_time);

        // Convert to DateTime<Local>
        let today_reset_local = today_reset
            .and_local_timezone(Local)
            .single()
            .ok_or_else(|| QuotaError::ConfigError("Failed to calculate reset time".to_string()))?;

        // If we haven't reached today's reset time yet, use yesterday's
        let reset_datetime: DateTime<Local> = if now.time() < reset_time {
            today_reset_local - chrono::Duration::days(1)
        } else {
            today_reset_local
        };

        Ok(reset_datetime.with_timezone(&Utc).to_rfc3339())
    }

    /// Clear all usage events (for testing)
    #[cfg(test)]
    pub async fn clear_all(&self) -> Result<(), QuotaError> {
        sqlx::query("DELETE FROM usage_events")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_manager() -> QuotaManager {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let config = QuotaConfig {
            per_minute: 3,
            per_day: 10,
            reset_time: "00:00".to_string(),
        };

        // We need to leak the tempdir to prevent cleanup during test
        let db_path_owned = db_path.to_path_buf();
        std::mem::forget(dir);

        QuotaManager::new(&db_path_owned, config).await.unwrap()
    }

    #[tokio::test]
    async fn test_record_event() {
        let manager = create_test_manager().await;

        let event = UsageEvent::new("gemini-2.5-pro", RequestKind::Ask, 100, 200, true);
        let id = manager.record_event(event).await.unwrap();

        assert!(id > 0);
    }

    #[tokio::test]
    async fn test_count_requests() {
        let manager = create_test_manager().await;
        manager.clear_all().await.unwrap();

        // Record some events
        for _ in 0..3 {
            let event = UsageEvent::new("gemini-2.5-pro", RequestKind::Ask, 100, 200, true);
            manager.record_event(event).await.unwrap();
        }

        let minute_count = manager.count_requests_last_minute().await.unwrap();
        assert_eq!(minute_count, 3);
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let manager = create_test_manager().await;
        manager.clear_all().await.unwrap();

        // Record 3 events (at the minute limit)
        for _ in 0..3 {
            let event = UsageEvent::new("gemini-2.5-pro", RequestKind::Ask, 100, 200, true);
            manager.record_event(event).await.unwrap();
        }

        // Should fail - minute quota exceeded
        let result = manager
            .check_before_request(&RequestKind::Ask, "gemini-2.5-pro")
            .await;

        assert!(matches!(
            result,
            Err(QuotaError::MinuteQuotaExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_usage_stats() {
        let manager = create_test_manager().await;
        manager.clear_all().await.unwrap();

        let event = UsageEvent::new("gemini-2.5-pro", RequestKind::Ask, 400, 800, true);
        manager.record_event(event).await.unwrap();

        let stats = manager.get_stats().await.unwrap();
        assert_eq!(stats.requests_today, 1);
        assert_eq!(stats.input_tokens_today, 100); // 400 / 4
        assert_eq!(stats.output_tokens_today, 200); // 800 / 4
    }
}
