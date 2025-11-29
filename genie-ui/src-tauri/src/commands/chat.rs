//! Chat commands

use super::CommandError;
use crate::state::AppState;
use genie_core::{GeminiRequest, RequestKind, UsageEvent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub message: String,
    pub model: String,
    pub tokens_used: u32,
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<RwLock<AppState>>>,
    request: ChatRequest,
) -> Result<ChatResponse, CommandError> {
    let state = state.read().await;

    let model = request
        .model
        .as_deref()
        .unwrap_or(&state.config.gemini.default_model);

    // Check quota
    state
        .quota
        .check_before_request(&RequestKind::Chat, model)
        .await?;

    // Build request
    let mut gemini_req = GeminiRequest::new(model, &request.message);
    if let Some(sys) = &request.system_prompt {
        gemini_req = gemini_req.with_system_prompt(sys);
    }

    // Call Gemini
    let response = state.gemini.call_text(&gemini_req).await?;

    // Record usage
    let event = UsageEvent::new(
        model,
        RequestKind::Chat,
        request.message.len(),
        response.text.len(),
        true,
    );
    let _ = state.quota.record_event(event).await;

    let tokens = (request.message.len() + response.text.len()) / 4;

    Ok(ChatResponse {
        message: response.text,
        model: response.model,
        tokens_used: tokens as u32,
    })
}

