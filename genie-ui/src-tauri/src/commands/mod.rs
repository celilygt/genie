//! Tauri commands bridging to genie-core

pub mod chat;
pub mod config;
pub mod docs;
pub mod quota;
pub mod repo;
pub mod templates;

use serde::{Deserialize, Serialize};

/// Common error type for Tauri commands
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandError {
    pub message: String,
    pub code: Option<String>,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<genie_core::GeminiError> for CommandError {
    fn from(e: genie_core::GeminiError) -> Self {
        Self::new(e.to_string()).with_code("gemini_error")
    }
}

impl From<genie_core::QuotaError> for CommandError {
    fn from(e: genie_core::QuotaError) -> Self {
        Self::new(e.to_string()).with_code("quota_error")
    }
}

