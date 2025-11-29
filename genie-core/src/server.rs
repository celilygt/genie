//! HTTP server for Genie API.
//!
//! Provides an OpenAI-compatible `/v1/chat/completions` endpoint
//! and Genie-specific endpoints for quota and health checks.

use crate::config::Config;
use crate::gemini::{GeminiClient, GeminiError, GeminiRequest};
use crate::model::{
    ApiError, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, HealthResponse,
    QuotaStatus, RequestKind, Usage,
};
use crate::quota::{QuotaError, QuotaManager, UsageEvent};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

/// Shared application state
pub struct AppState {
    pub gemini: GeminiClient,
    pub quota: QuotaManager,
    pub config: Arc<RwLock<Config>>,
}

impl AppState {
    pub fn new(gemini: GeminiClient, quota: QuotaManager, config: Config) -> Self {
        Self {
            gemini,
            quota,
            config: Arc::new(RwLock::new(config)),
        }
    }
}

/// Create the Axum router with all routes
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // OpenAI-compatible endpoints
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        // Genie-specific endpoints
        .route("/v1/quota", get(get_quota))
        .route("/v1/json", post(json_completion))
        // Health & status
        .route("/health", get(health_check))
        .route("/", get(root))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Root endpoint
async fn root() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "Genie",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Local Gemini-as-a-service backend",
        "endpoints": {
            "chat": "/v1/chat/completions",
            "json": "/v1/json",
            "models": "/v1/models",
            "quota": "/v1/quota",
            "health": "/health"
        }
    }))
}

/// Health check endpoint
#[instrument(skip(state))]
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let gemini_available = state.gemini.check_available().await;

    Json(HealthResponse {
        status: if gemini_available { "ok" } else { "degraded" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        gemini_available,
    })
}

/// List available models
async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;

    Json(serde_json::json!({
        "object": "list",
        "data": [
            {
                "id": config.gemini.default_model,
                "object": "model",
                "owned_by": "google",
                "permission": []
            },
            {
                "id": "gemini-2.5-pro",
                "object": "model",
                "owned_by": "google",
                "permission": []
            },
            {
                "id": "gemini-2.5-flash",
                "object": "model",
                "owned_by": "google",
                "permission": []
            }
        ]
    }))
}

/// Get quota status
#[instrument(skip(state))]
async fn get_quota(State(state): State<Arc<AppState>>) -> Result<Json<QuotaStatus>, AppError> {
    let config = state.config.read().await;
    let stats = state.quota.get_stats().await?;

    Ok(Json(QuotaStatus {
        requests_today: stats.requests_today,
        requests_per_day_limit: config.quota.per_day,
        requests_last_minute: stats.requests_last_minute,
        requests_per_minute_limit: config.quota.per_minute,
        approx_input_tokens_today: stats.input_tokens_today,
        approx_output_tokens_today: stats.output_tokens_today,
        last_error: stats.last_error,
        reset_time: config.quota.reset_time.clone(),
    }))
}

/// OpenAI-compatible chat completions endpoint
#[instrument(skip(state, request), fields(model = %request.model, messages = request.messages.len()))]
async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, AppError> {
    debug!("Received chat completion request");

    // Validate request
    if request.messages.is_empty() {
        return Err(AppError::InvalidRequest(
            "messages array cannot be empty".to_string(),
        ));
    }

    // Check quota
    state
        .quota
        .check_before_request(&RequestKind::Chat, &request.model)
        .await?;

    // Build prompt from messages
    let (system_prompt, user_prompt) = messages_to_prompt(&request.messages);
    let prompt_chars = user_prompt.len() + system_prompt.as_ref().map(|s| s.len()).unwrap_or(0);

    // Build Gemini request
    let mut gemini_req = GeminiRequest::new(&request.model, &user_prompt);
    if let Some(sys) = system_prompt {
        gemini_req = gemini_req.with_system_prompt(sys);
    }
    if let Some(temp) = request.temperature {
        gemini_req = gemini_req.with_temperature(temp);
    }
    if let Some(max) = request.max_tokens {
        gemini_req = gemini_req.with_max_tokens(max);
    }

    // Call Gemini
    let result = state.gemini.call_text(&gemini_req).await;

    match result {
        Ok(response) => {
            let response_chars = response.text.len();

            // Record successful usage
            let event = UsageEvent::new(
                &request.model,
                RequestKind::Chat,
                prompt_chars,
                response_chars,
                true,
            );
            if let Err(e) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", e);
            }

            // Build OpenAI-compatible response
            let completion = ChatCompletionResponse {
                id: format!("chatcmpl-{}", Uuid::new_v4()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: response.model,
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage::assistant(&response.text),
                    finish_reason: "stop".to_string(),
                }],
                usage: Some(Usage {
                    prompt_tokens: (prompt_chars / 4) as u32,
                    completion_tokens: (response_chars / 4) as u32,
                    total_tokens: ((prompt_chars + response_chars) / 4) as u32,
                }),
            };

            info!("Chat completion successful");
            Ok(Json(completion))
        }
        Err(e) => {
            // Record failed usage
            let event = UsageEvent::new(&request.model, RequestKind::Chat, prompt_chars, 0, false)
                .with_error(e.to_string());
            if let Err(re) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", re);
            }

            Err(e.into())
        }
    }
}

/// JSON completion endpoint (guaranteed JSON output)
#[instrument(skip(state, request), fields(model = %request.model))]
async fn json_completion(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, AppError> {
    debug!("Received JSON completion request");

    // Validate request
    if request.messages.is_empty() {
        return Err(AppError::InvalidRequest(
            "messages array cannot be empty".to_string(),
        ));
    }

    // Check quota
    state
        .quota
        .check_before_request(&RequestKind::Json, &request.model)
        .await?;

    // Build prompt from messages
    let (system_prompt, user_prompt) = messages_to_prompt(&request.messages);
    let prompt_chars = user_prompt.len() + system_prompt.as_ref().map(|s| s.len()).unwrap_or(0);

    // Build Gemini request with JSON output
    let mut gemini_req = GeminiRequest::new(&request.model, &user_prompt).with_json_output();
    if let Some(sys) = system_prompt {
        gemini_req = gemini_req.with_system_prompt(sys);
    }
    if let Some(temp) = request.temperature {
        gemini_req = gemini_req.with_temperature(temp);
    }

    // Call Gemini
    let result = state.gemini.call_json(&gemini_req).await;

    match result {
        Ok(response) => {
            let response_chars = response.text.len();

            // Record successful usage
            let event = UsageEvent::new(
                &request.model,
                RequestKind::Json,
                prompt_chars,
                response_chars,
                true,
            );
            if let Err(e) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", e);
            }

            // Use parsed JSON if available, otherwise use text
            let content = response
                .json
                .map(|j| j.to_string())
                .unwrap_or(response.text);

            let completion = ChatCompletionResponse {
                id: format!("chatcmpl-{}", Uuid::new_v4()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: response.model,
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage::assistant(&content),
                    finish_reason: "stop".to_string(),
                }],
                usage: Some(Usage {
                    prompt_tokens: (prompt_chars / 4) as u32,
                    completion_tokens: (response_chars / 4) as u32,
                    total_tokens: ((prompt_chars + response_chars) / 4) as u32,
                }),
            };

            info!("JSON completion successful");
            Ok(Json(completion))
        }
        Err(e) => {
            // Record failed usage
            let event = UsageEvent::new(&request.model, RequestKind::Json, prompt_chars, 0, false)
                .with_error(e.to_string());
            if let Err(re) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", re);
            }

            Err(e.into())
        }
    }
}

/// Convert chat messages to a prompt string
fn messages_to_prompt(messages: &[ChatMessage]) -> (Option<String>, String) {
    let mut system_prompt = None;
    let mut conversation = Vec::new();

    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                system_prompt = Some(msg.content.clone());
            }
            "user" => {
                conversation.push(format!("User: {}", msg.content));
            }
            "assistant" => {
                conversation.push(format!("Assistant: {}", msg.content));
            }
            _ => {
                conversation.push(msg.content.clone());
            }
        }
    }

    let user_prompt =
        if conversation.len() == 1 && messages.last().map(|m| m.role.as_str()) == Some("user") {
            // Single user message - just use the content directly
            messages.last().unwrap().content.clone()
        } else {
            conversation.join("\n\n")
        };

    (system_prompt, user_prompt)
}

/// Application error type
#[derive(Debug)]
pub enum AppError {
    InvalidRequest(String),
    QuotaExceeded(QuotaError),
    GeminiError(GeminiError),
    InternalError(String),
}

impl From<QuotaError> for AppError {
    fn from(e: QuotaError) -> Self {
        match e {
            QuotaError::MinuteQuotaExceeded { .. } | QuotaError::DailyQuotaExceeded { .. } => {
                AppError::QuotaExceeded(e)
            }
            _ => AppError::InternalError(e.to_string()),
        }
    }
}

impl From<GeminiError> for AppError {
    fn from(e: GeminiError) -> Self {
        AppError::GeminiError(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error) = match self {
            AppError::InvalidRequest(msg) => {
                (StatusCode::BAD_REQUEST, ApiError::invalid_request(msg))
            }
            AppError::QuotaExceeded(e) => (
                StatusCode::TOO_MANY_REQUESTS,
                ApiError::quota_exceeded(e.to_string()),
            ),
            AppError::GeminiError(e) => {
                let status = match &e {
                    GeminiError::AuthenticationError(_) => StatusCode::UNAUTHORIZED,
                    GeminiError::RateLimitError(_) => StatusCode::TOO_MANY_REQUESTS,
                    GeminiError::BinaryNotFound(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, ApiError::internal_error(e.to_string()))
            }
            AppError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::internal_error(msg),
            ),
        };

        (status, Json(error)).into_response()
    }
}

/// Start the HTTP server
pub async fn start_server(state: Arc<AppState>) -> Result<(), std::io::Error> {
    let config = state.config.read().await;
    let addr = config.server_addr();
    drop(config);

    let router = create_router(state);

    info!("Starting Genie server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_to_prompt_single() {
        let messages = vec![ChatMessage::user("Hello")];
        let (system, prompt) = messages_to_prompt(&messages);

        assert!(system.is_none());
        assert_eq!(prompt, "Hello");
    }

    #[test]
    fn test_messages_to_prompt_with_system() {
        let messages = vec![
            ChatMessage::system("You are helpful"),
            ChatMessage::user("Hello"),
        ];
        let (system, prompt) = messages_to_prompt(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(prompt, "Hello");
    }

    #[test]
    fn test_messages_to_prompt_conversation() {
        let messages = vec![
            ChatMessage::system("You are helpful"),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
            ChatMessage::user("How are you?"),
        ];
        let (system, prompt) = messages_to_prompt(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.contains("Assistant: Hi there!"));
        assert!(prompt.contains("User: How are you?"));
    }
}
