//! Document summarization commands

use super::CommandError;
use crate::state::AppState;
use genie_core::docs::{BookSummary, DocumentSummary, SummarizeOptions, SummaryStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct SummarizePdfRequest {
    pub path: String,
    pub style: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeBookRequest {
    pub path: String,
    pub style: Option<String>,
    pub language: Option<String>,
    pub use_gemini_chapters: Option<bool>,
}

fn parse_style(style: Option<&str>) -> SummaryStyle {
    match style {
        Some("detailed") => SummaryStyle::Detailed,
        Some("exam-notes") | Some("exam_notes") => SummaryStyle::ExamNotes,
        Some("bullet") => SummaryStyle::Bullet,
        _ => SummaryStyle::Concise,
    }
}

#[tauri::command]
pub async fn summarize_pdf(
    state: State<'_, Arc<RwLock<AppState>>>,
    request: SummarizePdfRequest,
) -> Result<DocumentSummary, CommandError> {
    let state = state.read().await;
    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(CommandError::new(format!("File not found: {}", request.path)));
    }

    let options = SummarizeOptions::new()
        .with_style(parse_style(request.style.as_deref()))
        .with_language(request.language.unwrap_or_else(|| "en".to_string()));

    let summary = genie_core::docs::summarize_pdf(&state.gemini, &path, &options)
        .await
        .map_err(|e| CommandError::new(e.to_string()))?;

    Ok(summary)
}

#[tauri::command]
pub async fn summarize_book(
    state: State<'_, Arc<RwLock<AppState>>>,
    request: SummarizeBookRequest,
) -> Result<BookSummary, CommandError> {
    let state = state.read().await;
    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(CommandError::new(format!("File not found: {}", request.path)));
    }

    let options = SummarizeOptions::new()
        .with_style(parse_style(request.style.as_deref()))
        .with_language(request.language.unwrap_or_else(|| "en".to_string()))
        .with_gemini_chapters(request.use_gemini_chapters.unwrap_or(false));

    let summary = genie_core::docs::summarize_book(&state.gemini, &path, &options)
        .await
        .map_err(|e| CommandError::new(e.to_string()))?;

    Ok(summary)
}

