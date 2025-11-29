//! Quota management commands

use super::CommandError;
use crate::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Serialize)]
pub struct QuotaStatus {
    pub requests_today: u32,
    pub requests_per_day_limit: u32,
    pub requests_last_minute: u32,
    pub requests_per_minute_limit: u32,
    pub approx_input_tokens_today: u32,
    pub approx_output_tokens_today: u32,
    pub last_error: Option<String>,
    pub reset_time: String,
}

#[derive(Debug, Serialize)]
pub struct UsageLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub model: String,
    pub kind: String,
    pub prompt_chars: u32,
    pub response_chars: u32,
    pub approx_input_tokens: u32,
    pub approx_output_tokens: u32,
    pub success: bool,
    pub error_code: Option<String>,
}

#[tauri::command]
pub async fn get_quota_status(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<QuotaStatus, CommandError> {
    let state = state.read().await;
    let stats = state.quota.get_stats().await?;

    Ok(QuotaStatus {
        requests_today: stats.requests_today,
        requests_per_day_limit: state.config.quota.per_day,
        requests_last_minute: stats.requests_last_minute,
        requests_per_minute_limit: state.config.quota.per_minute,
        approx_input_tokens_today: stats.input_tokens_today,
        approx_output_tokens_today: stats.output_tokens_today,
        last_error: stats.last_error,
        reset_time: state.config.quota.reset_time.clone(),
    })
}

#[tauri::command]
pub async fn get_usage_log(
    state: State<'_, Arc<RwLock<AppState>>>,
    limit: Option<u32>,
) -> Result<Vec<UsageLogEntry>, CommandError> {
    let state = state.read().await;
    let events = state.quota.get_recent_events(limit.unwrap_or(50)).await?;

    Ok(events
        .into_iter()
        .map(|e| UsageLogEntry {
            id: e.id.unwrap_or(0),
            timestamp: e.timestamp,
            model: e.model,
            kind: e.kind,
            prompt_chars: e.prompt_chars as u32,
            response_chars: e.response_chars as u32,
            approx_input_tokens: e.approx_input_tokens as u32,
            approx_output_tokens: e.approx_output_tokens as u32,
            success: e.success,
            error_code: e.error_code,
        })
        .collect())
}

