//! Application state management

use anyhow::Result;
use genie_core::{Config, GeminiClient, QuotaManager};

/// Shared application state
pub struct AppState {
    pub config: Config,
    pub gemini: GeminiClient,
    pub quota: QuotaManager,
}

impl AppState {
    /// Create a new AppState with initialized components
    pub fn new() -> Result<Self> {
        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Ensure directories exist
        Config::ensure_dirs()?;

        // Create Gemini client
        let gemini = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

        // Create quota manager (blocking initialization for setup)
        let db_path = Config::default_db_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;

        let quota = tokio::runtime::Runtime::new()?.block_on(async {
            QuotaManager::new(&db_path, config.quota.clone()).await
        })?;

        Ok(Self {
            config,
            gemini,
            quota,
        })
    }
}

