//! Repository summarization commands

use super::CommandError;
use crate::state::AppState;
use genie_core::repo::{RepoOptions, RepoSummary};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct SummarizeRepoRequest {
    pub path: String,
    pub max_files: Option<u32>,
}

#[tauri::command]
pub async fn summarize_repo(
    state: State<'_, Arc<RwLock<AppState>>>,
    request: SummarizeRepoRequest,
) -> Result<RepoSummary, CommandError> {
    let state = state.read().await;
    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(CommandError::new(format!("Path not found: {}", request.path)));
    }

    if !path.is_dir() {
        return Err(CommandError::new(format!(
            "Path is not a directory: {}",
            request.path
        )));
    }

    let mut options = RepoOptions::default();
    if let Some(max) = request.max_files {
        options = options.with_max_files(max);
    }

    let summary = genie_core::repo::summarize_repo(&state.gemini, &path, &options)
        .await
        .map_err(|e| CommandError::new(e.to_string()))?;

    Ok(summary)
}

