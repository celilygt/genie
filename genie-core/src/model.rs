//! Shared data models for Genie.
//!
//! This module contains types used across the application for
//! requests, responses, and internal data structures.

use serde::{Deserialize, Serialize};

/// Types of requests that Genie can handle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Ask,
    Json,
    Chat,
    Map,
    Transform,
    SummarizePdf,
    SummarizeBook,
    RepoSummary,
    Template,
    Custom(String),
}

impl std::fmt::Display for RequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestKind::Ask => write!(f, "ask"),
            RequestKind::Json => write!(f, "json"),
            RequestKind::Chat => write!(f, "chat"),
            RequestKind::Map => write!(f, "map"),
            RequestKind::Transform => write!(f, "transform"),
            RequestKind::SummarizePdf => write!(f, "summarize_pdf"),
            RequestKind::SummarizeBook => write!(f, "summarize_book"),
            RequestKind::RepoSummary => write!(f, "repo_summary"),
            RequestKind::Template => write!(f, "template"),
            RequestKind::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

impl From<&str> for RequestKind {
    fn from(s: &str) -> Self {
        match s {
            "ask" => RequestKind::Ask,
            "json" => RequestKind::Json,
            "chat" => RequestKind::Chat,
            "map" => RequestKind::Map,
            "transform" => RequestKind::Transform,
            "summarize_pdf" => RequestKind::SummarizePdf,
            "summarize_book" => RequestKind::SummarizeBook,
            "repo_summary" => RequestKind::RepoSummary,
            "template" => RequestKind::Template,
            other => RequestKind::Custom(other.to_string()),
        }
    }
}

/// OpenAI-compatible chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// OpenAI-compatible chat completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stream: bool,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Choice in a chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Quota status response
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorDetail {
    pub message: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl ApiError {
    pub fn new(message: impl Into<String>, error_type: impl Into<String>) -> Self {
        Self {
            error: ApiErrorDetail {
                message: message.into(),
                r#type: error_type.into(),
                code: None,
            },
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.error.code = Some(code.into());
        self
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(message, "invalid_request_error")
    }

    pub fn quota_exceeded(message: impl Into<String>) -> Self {
        Self::new(message, "quota_exceeded").with_code("rate_limit_exceeded")
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(message, "internal_error")
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub gemini_available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_kind_display() {
        assert_eq!(RequestKind::Ask.to_string(), "ask");
        assert_eq!(RequestKind::Json.to_string(), "json");
        assert_eq!(
            RequestKind::Custom("test".to_string()).to_string(),
            "custom:test"
        );
    }

    #[test]
    fn test_chat_message_constructors() {
        let system = ChatMessage::system("You are helpful");
        assert_eq!(system.role, "system");

        let user = ChatMessage::user("Hello");
        assert_eq!(user.role, "user");

        let assistant = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant.role, "assistant");
    }

    #[test]
    fn test_api_error_serialization() {
        let error = ApiError::quota_exceeded("Rate limit exceeded");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("quota_exceeded"));
        assert!(json.contains("rate_limit_exceeded"));
    }
}
