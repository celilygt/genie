//! Configuration commands

use super::CommandError;
use crate::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Serialize)]
pub struct AppConfig {
    pub gemini_binary: String,
    pub default_model: String,
    pub server_host: String,
    pub server_port: u16,
    pub quota_per_minute: u32,
    pub quota_per_day: u32,
    pub quota_reset_time: String,
}

#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub gemini_available: bool,
}

#[tauri::command]
pub async fn get_config(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<AppConfig, CommandError> {
    let state = state.read().await;

    Ok(AppConfig {
        gemini_binary: state.config.gemini.binary.clone(),
        default_model: state.config.gemini.default_model.clone(),
        server_host: state.config.server.host.clone(),
        server_port: state.config.server.port,
        quota_per_minute: state.config.quota.per_minute,
        quota_per_day: state.config.quota.per_day,
        quota_reset_time: state.config.quota.reset_time.clone(),
    })
}

#[tauri::command]
pub async fn health_check(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<HealthStatus, CommandError> {
    let state = state.read().await;
    let gemini_available = state.gemini.check_available().await;

    Ok(HealthStatus {
        status: if gemini_available { "ok" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        gemini_available,
    })
}

