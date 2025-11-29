//! Gemini CLI wrapper.
//!
//! This module provides an async interface to the official `gemini` CLI,
//! handling process spawning, stdin/stdout piping, and error parsing.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error, instrument, warn};

/// Errors that can occur when interacting with the Gemini CLI
#[derive(Debug, Error)]
pub enum GeminiError {
    #[error("Gemini binary not found at '{0}'. Please ensure the gemini CLI is installed and accessible.")]
    BinaryNotFound(String),

    #[error("Failed to spawn Gemini process: {0}")]
    SpawnError(#[from] std::io::Error),

    #[error("Gemini process failed with exit code {exit_code}: {stderr}")]
    ProcessFailed { exit_code: i32, stderr: String },

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    #[error("Invalid JSON response from Gemini: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Gemini returned an error: {0}")]
    GeminiApiError(String),

    #[error("Request timed out after {0} seconds")]
    Timeout(u64),
}

/// Request to send to Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiRequest {
    /// The model to use (e.g., "gemini-2.5-pro")
    pub model: String,

    /// The prompt to send
    pub prompt: String,

    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Temperature for response generation (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens in response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Whether to request JSON output
    #[serde(default)]
    pub json_output: bool,
}

impl GeminiRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            json_output: false,
        }
    }

    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_json_output(mut self) -> Self {
        self.json_output = true;
        self
    }

    /// Estimate input tokens (rough approximation: chars / 4)
    pub fn estimate_input_tokens(&self) -> u32 {
        let total_chars =
            self.prompt.len() + self.system_prompt.as_ref().map(|s| s.len()).unwrap_or(0);
        (total_chars / 4) as u32
    }
}

/// Response from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    /// Raw output from the CLI
    pub raw_output: String,

    /// Extracted text content
    pub text: String,

    /// Parsed JSON if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,

    /// Model used
    pub model: String,
}

impl GeminiResponse {
    /// Estimate output tokens (rough approximation: chars / 4)
    pub fn estimate_output_tokens(&self) -> u32 {
        (self.text.len() / 4) as u32
    }
}

/// Client for interacting with the Gemini CLI
pub struct GeminiClient {
    /// Path to the gemini binary
    binary_path: PathBuf,

    /// Default model to use
    #[allow(dead_code)]
    default_model: String,

    /// Default system prompt
    default_system_prompt: Option<String>,

    /// Timeout in seconds (0 = no timeout)
    timeout_secs: u64,
}

impl GeminiClient {
    /// Create a new GeminiClient
    pub fn new(binary_path: impl Into<PathBuf>, default_model: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
            default_model: default_model.into(),
            default_system_prompt: None,
            timeout_secs: 300, // 5 minutes default
        }
    }

    /// Set the default system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.default_system_prompt = Some(prompt.into());
        self
    }

    /// Set the timeout in seconds
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Check if the gemini binary is available
    pub async fn check_available(&self) -> bool {
        Command::new(&self.binary_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Get the version of the gemini CLI
    pub async fn version(&self) -> Result<String, GeminiError> {
        let output = Command::new(&self.binary_path)
            .arg("--version")
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(GeminiError::BinaryNotFound(
                self.binary_path.display().to_string(),
            ))
        }
    }

    /// Call Gemini with a text prompt
    #[instrument(skip(self, req), fields(model = %req.model, prompt_len = req.prompt.len()))]
    pub async fn call_text(&self, req: &GeminiRequest) -> Result<GeminiResponse, GeminiError> {
        // Build the full prompt including system prompt
        let full_prompt = self.build_prompt(req);

        debug!("Calling Gemini CLI with model: {}", req.model);

        // Build command arguments
        let mut cmd = Command::new(&self.binary_path);

        // Use non-interactive mode with -p flag
        cmd.arg("-p").arg(&full_prompt);

        // Specify model
        cmd.arg("-m").arg(&req.model);

        // Request JSON output format for easier parsing
        cmd.arg("--output-format").arg("json");

        // Set up process I/O
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn and wait for result
        let output = if self.timeout_secs > 0 {
            tokio::time::timeout(
                std::time::Duration::from_secs(self.timeout_secs),
                cmd.output(),
            )
            .await
            .map_err(|_| GeminiError::Timeout(self.timeout_secs))??
        } else {
            cmd.output().await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        debug!("Gemini CLI exit code: {:?}", output.status.code());

        // Check for process failure
        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);

            // Try to categorize the error
            if stderr.contains("authentication")
                || stderr.contains("auth")
                || stderr.contains("login")
            {
                return Err(GeminiError::AuthenticationError(stderr));
            }
            if stderr.contains("rate limit") || stderr.contains("quota") || stderr.contains("429") {
                return Err(GeminiError::RateLimitError(stderr));
            }

            return Err(GeminiError::ProcessFailed { exit_code, stderr });
        }

        // Parse the response
        self.parse_response(&stdout, &req.model, req.json_output)
    }

    /// Call Gemini and expect JSON output
    #[instrument(skip(self, req), fields(model = %req.model))]
    pub async fn call_json(&self, req: &GeminiRequest) -> Result<GeminiResponse, GeminiError> {
        let mut json_req = req.clone();
        json_req.json_output = true;

        // Add JSON instruction to prompt if not already present
        if !json_req.prompt.to_lowercase().contains("json") {
            json_req.prompt = format!(
                "{}\n\nRespond with valid JSON only. Do not include markdown code fences or any other text.",
                json_req.prompt
            );
        }

        let response = self.call_text(&json_req).await?;

        // Validate that we got valid JSON
        if response.json.is_none() {
            warn!("Expected JSON response but got plain text");
        }

        Ok(response)
    }

    /// Build the full prompt including system prompt
    fn build_prompt(&self, req: &GeminiRequest) -> String {
        let system = req
            .system_prompt
            .as_ref()
            .or(self.default_system_prompt.as_ref());

        match system {
            Some(sys) => format!("{}\n\n{}", sys, req.prompt),
            None => req.prompt.clone(),
        }
    }

    /// Parse the response from Gemini CLI
    fn parse_response(
        &self,
        output: &str,
        model: &str,
        expect_json: bool,
    ) -> Result<GeminiResponse, GeminiError> {
        let trimmed = output.trim();

        // Try to parse as JSON first (since we request --output-format json)
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Extract the text content from the JSON response
            // The gemini CLI JSON format typically has a "response" or "text" field
            let text = extract_text_from_gemini_json(&json_value);

            // If expecting JSON output from the model, try to parse the text as JSON
            let inner_json = if expect_json {
                extract_json_from_text(&text)
            } else {
                None
            };

            return Ok(GeminiResponse {
                raw_output: output.to_string(),
                text,
                json: inner_json,
                model: model.to_string(),
            });
        }

        // If not JSON, treat as plain text
        let json = if expect_json {
            extract_json_from_text(trimmed)
        } else {
            None
        };

        Ok(GeminiResponse {
            raw_output: output.to_string(),
            text: trimmed.to_string(),
            json,
            model: model.to_string(),
        })
    }
}

/// Extract text content from Gemini CLI JSON output
fn extract_text_from_gemini_json(value: &serde_json::Value) -> String {
    // Try common field names
    if let Some(text) = value.get("response").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(text) = value.get("content").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(text) = value.get("output").and_then(|v| v.as_str()) {
        return text.to_string();
    }

    // If it's a string at the top level
    if let Some(text) = value.as_str() {
        return text.to_string();
    }

    // Fallback: convert the whole thing to string
    value.to_string()
}

/// Try to extract JSON from text that might contain markdown code fences
fn extract_json_from_text(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();

    // Try direct parse first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(json);
    }

    // Try to extract from markdown code fence
    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            let json_str = after_fence[..end].trim();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Some(json);
            }
        }
    }

    // Try to find JSON object or array
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Some(json);
            }
        }
    }

    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            let json_str = &trimmed[start..=end];
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Some(json);
            }
        }
    }

    None
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new("gemini", "gemini-2.5-pro")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_request_builder() {
        let req = GeminiRequest::new("gemini-2.5-pro", "Hello")
            .with_system_prompt("You are helpful")
            .with_temperature(0.7)
            .with_json_output();

        assert_eq!(req.model, "gemini-2.5-pro");
        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(req.temperature, Some(0.7));
        assert!(req.json_output);
    }

    #[test]
    fn test_estimate_tokens() {
        let req = GeminiRequest::new("model", "This is a test prompt with some words");
        // ~40 chars / 4 = ~10 tokens
        assert!(req.estimate_input_tokens() >= 8 && req.estimate_input_tokens() <= 12);
    }

    #[test]
    fn test_extract_json_from_text() {
        // Direct JSON
        let json = extract_json_from_text(r#"{"key": "value"}"#);
        assert!(json.is_some());
        assert_eq!(json.unwrap()["key"], "value");

        // With markdown fence
        let json = extract_json_from_text(
            r#"Here's the result:
```json
{"key": "value"}
```"#,
        );
        assert!(json.is_some());

        // Array
        let json = extract_json_from_text(r#"[1, 2, 3]"#);
        assert!(json.is_some());
        assert!(json.unwrap().is_array());
    }

    #[test]
    fn test_client_default() {
        let client = GeminiClient::default();
        assert_eq!(client.binary_path.to_str().unwrap(), "gemini");
        assert_eq!(client.default_model, "gemini-2.5-pro");
    }
}
