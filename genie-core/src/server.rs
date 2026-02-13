//! HTTP server for Genie API.
//!
//! Provides an OpenAI-compatible `/v1/chat/completions` endpoint
//! and Genie-specific endpoints for quota, docs, and repo summarization.

use crate::config::Config;
use crate::docs::{self, BookSummary, DocumentSummary, SummarizeOptions, SummaryStyle};
use crate::embeddings::{EmbeddingsError, LocalEmbeddings};
use crate::gemini::{GeminiClient, GeminiError, GeminiRequest};
use crate::model::{
    ApiError, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    Choice, CompletionChoice, CompletionRequest, CompletionResponse, EmbeddingRequest,
    EmbeddingResponse, HealthResponse, QuotaStatus, RequestKind, Usage,
};
use crate::quota::{QuotaError, QuotaManager, UsageEvent};
use crate::rag::{IngestOptions, QueryOptions, RagCollection, RagManager, RagQueryResponse};
use crate::repo::{self, RepoOptions, RepoSummary};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, instrument};
use utoipa::OpenApi;
use uuid::Uuid;

/// OpenAPI documentation for Genie API
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Genie API",
        version = "0.1.0",
        description = "Local Gemini-as-a-service with OpenAI-compatible endpoints. \
                       Genie wraps the official Gemini CLI to provide a local AI backend \
                       with quota management, PDF/book summarization, repository analysis, \
                       RAG capabilities, and prompt templates.",
        license(name = "MIT"),
        contact(name = "Genie Contributors")
    ),
    servers(
        (url = "http://127.0.0.1:11435", description = "Local development server")
    ),
    paths(
        health_check,
        list_models,
        get_quota,
        chat_completions,
        text_completions,
        json_completion,
        create_embeddings,
        summarize_docs,
        summarize_repo,
        rag_list_collections,
        rag_ingest,
        rag_query,
    ),
    components(schemas(
        crate::model::ChatMessage,
        crate::model::ChatCompletionRequest,
        crate::model::ChatCompletionResponse,
        crate::model::Choice,
        crate::model::Usage,
        crate::model::CompletionRequest,
        crate::model::CompletionResponse,
        crate::model::CompletionChoice,
        crate::model::EmbeddingRequest,
        crate::model::EmbeddingResponse,
        crate::model::EmbeddingData,
        crate::model::EmbeddingUsage,
        crate::model::EmbeddingInput,
        crate::model::QuotaStatus,
        crate::model::HealthResponse,
        crate::model::ApiError,
        crate::model::ApiErrorDetail,
        DocsSummarizeRequest,
        RepoSummaryRequest,
        RagIngestRequest,
        RagIngestResponse,
        RagQueryRequest,
    )),
    tags(
        (name = "OpenAI Compatible", description = "OpenAI-style API endpoints for chat and text completion"),
        (name = "Quota", description = "Usage tracking and quota management"),
        (name = "Documents", description = "PDF and book summarization"),
        (name = "Repository", description = "Codebase analysis and summarization"),
        (name = "RAG", description = "Retrieval-Augmented Generation"),
        (name = "Health", description = "Server health and status")
    )
)]
pub struct ApiDoc;

/// Shared application state
pub struct AppState {
    pub gemini: GeminiClient,
    pub quota: QuotaManager,
    pub config: Arc<RwLock<Config>>,
    /// Cache of local embeddings models - lazily initialized per model
    embeddings_cache: RwLock<HashMap<String, Arc<LocalEmbeddings>>>,
}

impl AppState {
    pub fn new(gemini: GeminiClient, quota: QuotaManager, config: Config) -> Self {
        Self {
            gemini,
            quota,
            config: Arc::new(RwLock::new(config)),
            embeddings_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get or initialize local embeddings for a specific model
    /// Models are cached after first initialization
    pub async fn get_or_init_embeddings(
        &self,
        model_name: &str,
    ) -> Result<Arc<LocalEmbeddings>, EmbeddingsError> {
        // Normalize model name for cache key
        let cache_key = model_name.to_lowercase();

        // Check cache first (read lock)
        {
            let cache = self.embeddings_cache.read().await;
            if let Some(embeddings) = cache.get(&cache_key) {
                return Ok(Arc::clone(embeddings));
            }
        }

        // Not in cache, need to initialize (write lock)
        let mut cache = self.embeddings_cache.write().await;

        // Double-check after acquiring write lock (another request might have initialized)
        if let Some(embeddings) = cache.get(&cache_key) {
            return Ok(Arc::clone(embeddings));
        }

        // Initialize the model
        info!(
            "Lazy-initializing local embeddings model: {} ...",
            model_name
        );
        let embeddings = LocalEmbeddings::from_openai_model(model_name)?;
        let embeddings = Arc::new(embeddings);
        cache.insert(cache_key, Arc::clone(&embeddings));

        Ok(embeddings)
    }

    /// Get a cached embeddings model if available (for RAG operations)
    pub async fn get_cached_embeddings(&self) -> Option<Arc<LocalEmbeddings>> {
        let cache = self.embeddings_cache.read().await;
        // Return any cached model (prefer the first one found)
        cache.values().next().cloned()
    }
}

/// Create the Axum router with all routes
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // OpenAPI documentation (JSON spec)
        .route("/openapi.json", get(openapi_json))
        // OpenAI-compatible endpoints
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/completions", post(text_completions))
        .route("/v1/embeddings", post(create_embeddings))
        .route("/v1/models", get(list_models))
        // Genie-specific endpoints
        .route("/v1/quota", get(get_quota))
        .route("/v1/quota/logs", get(get_usage_logs))
        .route("/v1/json", post(json_completion))
        // Document summarization endpoints
        .route("/v1/docs/summarize", post(summarize_docs))
        // Repo summarization endpoint
        .route("/v1/repo/summary", post(summarize_repo))
        // RAG endpoints
        .route("/v1/rag/collections", get(rag_list_collections))
        .route("/v1/rag/ingest", post(rag_ingest))
        .route("/v1/rag/query", post(rag_query))
        // Health & status
        .route("/health", get(health_check))
        .route("/", get(root))
        // Web dashboard (Tilt-style)
        .route("/dashboard", get(dashboard))
        // Swagger UI for API testing
        .route("/swagger", get(swagger_ui))
        // Markdown documentation
        .route("/docs/markdown", get(docs_markdown))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// OpenAPI JSON specification endpoint
async fn openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

/// Root endpoint - Discovery information for SDK integration
async fn root(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let base_url = format!("http://{}:{}", config.server.host, config.server.port);
    let embeddings_enabled = config.embeddings.enabled;
    drop(config);

    // Build capabilities list dynamically
    let mut capabilities = vec![
        "chat",
        "completions",
        "json",
        "docs_summarization",
        "repo_summarization",
        "rag",
        "prompt_templates",
    ];
    if embeddings_enabled {
        capabilities.insert(3, "embeddings"); // Insert after "json"
    }

    Json(serde_json::json!({
        "name": "Genie",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Local Gemini-as-a-service backend with OpenAI-compatible API",
        
        // SDK integration info
        "openai_compatible": true,
        "base_url": base_url,
        "api_version": "v1",
        
        // Available capabilities
        "capabilities": capabilities,
        
        // Supported models
        "default_model": "gemini-3-pro-preview",
        "models": [
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "gemini-2.5-pro",
            "gemini-2.5-flash"
        ],
        
        // Documentation
        "documentation": {
            "openapi": "/openapi.json",
            "swagger_ui": "/swagger",
            "dashboard": "/dashboard",
            "readme": "https://github.com/celilygt/genie"
        },
        
        // Primary endpoints (OpenAI-compatible)
        "endpoints": {
            "openai_compatible": {
                "chat_completions": {
                    "path": "/v1/chat/completions",
                    "method": "POST",
                    "description": "OpenAI-compatible chat completions"
                },
                "completions": {
                    "path": "/v1/completions", 
                    "method": "POST",
                    "description": "OpenAI-compatible text completions"
                },
                "embeddings": {
                    "path": "/v1/embeddings",
                    "method": "POST",
                    "description": "Generate text embeddings (local, free)"
                },
                "models": {
                    "path": "/v1/models",
                    "method": "GET",
                    "description": "List available models"
                }
            },
            "genie_specific": {
                "json": {
                    "path": "/v1/json",
                    "method": "POST",
                    "description": "Guaranteed JSON output"
                },
                "quota": {
                    "path": "/v1/quota",
                    "method": "GET",
                    "description": "Usage quota status"
                },
                "docs_summarize": {
                    "path": "/v1/docs/summarize",
                    "method": "POST",
                    "description": "PDF/book summarization"
                },
                "repo_summary": {
                    "path": "/v1/repo/summary",
                    "method": "POST",
                    "description": "Repository code summarization"
                }
            },
            "rag": {
                "collections": {
                    "path": "/v1/rag/collections",
                    "method": "GET",
                    "description": "List RAG collections"
                },
                "ingest": {
                    "path": "/v1/rag/ingest",
                    "method": "POST",
                    "description": "Ingest documents into RAG"
                },
                "query": {
                    "path": "/v1/rag/query",
                    "method": "POST",
                    "description": "Query RAG collection"
                }
            },
            "system": {
                "health": {
                    "path": "/health",
                    "method": "GET",
                    "description": "Health check"
                },
                "openapi": {
                    "path": "/openapi.json",
                    "method": "GET",
                    "description": "OpenAPI specification"
                }
            }
        },
        
        // Example usage for quick start
        "quickstart": {
            "curl_example": "curl -X POST http://127.0.0.1:11435/v1/chat/completions -H 'Content-Type: application/json' -d '{\"model\":\"gemini-2.5-pro\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}'",
            "python_example": "from openai import OpenAI\nclient = OpenAI(base_url='http://127.0.0.1:11435/v1', api_key='not-needed')\nresponse = client.chat.completions.create(model='gemini-2.5-pro', messages=[{'role': 'user', 'content': 'Hello'}])"
        }
    }))
}

/// Web dashboard - Tilt-style UI
async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// Markdown documentation endpoint - downloadable API docs
async fn docs_markdown() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"genie-api-docs.md\"")
        .body(Body::from(DOCS_MARKDOWN))
        .unwrap()
}

/// Markdown documentation content
const DOCS_MARKDOWN: &str = r#"# Genie API Documentation

Genie is a local Gemini-as-a-service backend that provides an OpenAI-compatible API. It wraps the official Gemini CLI to provide quota management, document summarization, repository analysis, and RAG capabilities.

## Quick Start

### Base URL
```
http://127.0.0.1:11435
```

### Using with OpenAI SDK (Python)
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:11435/v1",
    api_key="not-needed"  # Genie uses Gemini CLI auth
)

# Chat completions
response = client.chat.completions.create(
    model="gemini-3-pro-preview",  # or gemini-3-flash-preview, gemini-2.5-pro, gemini-2.5-flash
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Hello!"}
    ]
)
print(response.choices[0].message.content)

# Embeddings (local, free!)
embedding = client.embeddings.create(
    input="Hello world",
    model="text-embedding-ada-002"
)
print(f"Embedding dimensions: {len(embedding.data[0].embedding)}")  # 384
```

### Using with cURL
```bash
curl -X POST http://127.0.0.1:11435/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-3-pro-preview",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

---

## Available Models

| Model | Description |
|-------|-------------|
| `gemini-3-pro-preview` | Latest Gemini 3 Pro - best for complex tasks |
| `gemini-3-flash-preview` | Latest Gemini 3 Flash - fast and efficient |
| `gemini-2.5-pro` | Gemini 2.5 Pro - powerful reasoning |
| `gemini-2.5-flash` | Gemini 2.5 Flash - balanced performance |

---

## API Endpoints

### OpenAI-Compatible Endpoints

#### POST /v1/chat/completions
Chat completion (supports streaming with `stream: true`).

**Request:**
```json
{
  "model": "gemini-3-pro-preview",
  "messages": [
    {"role": "system", "content": "You are helpful."},
    {"role": "user", "content": "Explain quantum computing"}
  ],
  "max_tokens": 1024,
  "temperature": 0.7,
  "stream": false
}
```

**Response:**
```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gemini-3-pro-preview",
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "Quantum computing is..."
    },
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 20,
    "completion_tokens": 150,
    "total_tokens": 170
  }
}
```

#### POST /v1/completions
Simple text completion.

**Request:**
```json
{
  "model": "gemini-3-flash-preview",
  "prompt": "The meaning of life is",
  "max_tokens": 100,
  "temperature": 0.5
}
```

#### GET /v1/models
List available models.

#### POST /v1/embeddings
Generate text embeddings locally (100% free, no API costs).

**Request:**
```json
{
  "input": "Hello world",
  "model": "text-embedding-ada-002"
}
```

Or with multiple inputs:
```json
{
  "input": ["Hello world", "How are you?"],
  "model": "text-embedding-ada-002"
}
```

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "object": "embedding",
      "embedding": [0.123, -0.456, 0.789, ...],
      "index": 0
    }
  ],
  "model": "all-MiniLM-L6-v2",
  "usage": {
    "prompt_tokens": 2,
    "total_tokens": 2
  }
}
```

| Model Requested | Local Model Used | Dimensions |
|-----------------|------------------|------------|
| `text-embedding-ada-002` | all-MiniLM-L6-v2 | 384 |
| `text-embedding-3-small` | bge-small-en-v1.5 | 384 |
| `text-embedding-3-large` | bge-base-en-v1.5 | 768 |

**Note:** Embeddings run 100% locally using ONNX models - no API calls, no costs!

---

### Genie-Specific Endpoints

#### POST /v1/json
Guaranteed JSON output - use when you need structured data.

**Request:** Same as `/v1/chat/completions`

#### GET /v1/quota
Get current quota usage status.

**Response:**
```json
{
  "requests_today": 42,
  "requests_per_day_limit": 1000,
  "requests_last_minute": 3,
  "requests_per_minute_limit": 60,
  "approx_input_tokens_today": 15000,
  "approx_output_tokens_today": 8500
}
```

#### GET /v1/quota/logs
Get recent request history.

---

### Document Summarization

#### POST /v1/docs/summarize
Summarize PDF documents or books with chapter detection.

**Request:**
```json
{
  "path": "/path/to/document.pdf",
  "mode": "book",
  "style": "detailed",
  "language": "en"
}
```

| Parameter | Values |
|-----------|--------|
| `mode` | `"pdf"` (simple) or `"book"` (with chapters) |
| `style` | `"concise"`, `"detailed"`, `"exam-notes"`, `"bullet"` |

---

### Repository Analysis

#### POST /v1/repo/summary
Analyze and summarize a code repository.

**Request:**
```json
{
  "path": "/path/to/repo",
  "max_files": 100
}
```

---

### RAG (Retrieval-Augmented Generation)

#### GET /v1/rag/collections
List all RAG collections.

#### POST /v1/rag/ingest
Ingest documents into a collection.

**Request:**
```json
{
  "collection_id": "my-docs",
  "path": "/path/to/documents",
  "pattern": "*.txt",
  "chunk_size": 1000
}
```

#### POST /v1/rag/query
Query a collection with RAG.

**Request:**
```json
{
  "collection_id": "my-docs",
  "question": "What is the main topic?",
  "top_k": 5,
  "return_sources": true
}
```

---

## LangChain Integration

```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="http://127.0.0.1:11435/v1",
    api_key="not-needed",
    model="gemini-3-pro-preview"
)

response = llm.invoke("Explain machine learning")
print(response.content)
```

---

## Configuration

Genie reads config from `~/.genie/config.toml`:

```toml
[gemini]
binary = "gemini"
default_model = "gemini-3-pro-preview"

[server]
host = "127.0.0.1"
port = 11435

[quota]
per_minute = 60
per_day = 1000
```

---

## CLI Commands

```bash
# Start server with TUI
genie up

# Start headless daemon
genie up --daemon

# Install as macOS service (auto-start on login)
genie service install
genie service start

# Simple prompt
genie ask "Hello world"

# JSON output
genie json "List 3 colors as JSON array"

# Check quota
genie quota status
```

---

## Health Check

#### GET /health
```json
{
  "status": "ok",
  "ready": true,
  "version": "0.1.0",
  "gemini_available": true
}
```

---

## Error Handling

Errors follow OpenAI format:
```json
{
  "error": {
    "message": "Rate limit exceeded",
    "type": "quota_exceeded",
    "code": "rate_limit_exceeded"
  }
}
```

---

## Dashboard

Open `http://127.0.0.1:11435/dashboard` for a web UI showing:
- Real-time quota status
- Request history
- API endpoint reference

Or press **Space** in the TUI (`genie up`) to open the dashboard.
"#;

/// HTML for the web dashboard
const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Genie Dashboard</title>
    <style>
        :root {
            --bg-primary: #0d1117;
            --bg-secondary: #161b22;
            --bg-tertiary: #21262d;
            --border-color: #30363d;
            --text-primary: #e6edf3;
            --text-secondary: #8b949e;
            --accent-cyan: #58a6ff;
            --accent-green: #3fb950;
            --accent-yellow: #d29922;
            --accent-red: #f85149;
            --accent-purple: #a371f7;
        }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }
        .container { max-width: 1200px; margin: 0 auto; padding: 20px; }
        header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 16px 24px;
            background: var(--bg-secondary);
            border-bottom: 1px solid var(--border-color);
            position: sticky;
            top: 0;
            z-index: 100;
        }
        .logo {
            display: flex;
            align-items: center;
            gap: 12px;
            font-size: 1.5rem;
            font-weight: 600;
        }
        .logo-icon { font-size: 2rem; }
        .version { color: var(--text-secondary); font-size: 0.875rem; font-weight: normal; }
        .status-badge {
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.875rem;
            font-weight: 500;
        }
        .status-badge.ok { background: rgba(63, 185, 80, 0.15); color: var(--accent-green); }
        .status-badge.degraded { background: rgba(210, 153, 34, 0.15); color: var(--accent-yellow); }
        .status-badge.error { background: rgba(248, 81, 73, 0.15); color: var(--accent-red); }
        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            animation: pulse 2s infinite;
        }
        .status-badge.ok .status-dot { background: var(--accent-green); }
        .status-badge.degraded .status-dot { background: var(--accent-yellow); }
        .status-badge.error .status-dot { background: var(--accent-red); }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; margin-top: 20px; }
        .card {
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 20px;
        }
        .card-title {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 0.875rem;
            color: var(--text-secondary);
            text-transform: uppercase;
            letter-spacing: 0.05em;
            margin-bottom: 16px;
        }
        .quota-section { margin-bottom: 20px; }
        .quota-label {
            display: flex;
            justify-content: space-between;
            margin-bottom: 8px;
            font-size: 0.875rem;
        }
        .quota-value { color: var(--text-secondary); }
        .progress-bar {
            height: 8px;
            background: var(--bg-tertiary);
            border-radius: 4px;
            overflow: hidden;
        }
        .progress-fill {
            height: 100%;
            border-radius: 4px;
            transition: width 0.3s ease, background-color 0.3s ease;
        }
        .progress-fill.green { background: var(--accent-green); }
        .progress-fill.yellow { background: var(--accent-yellow); }
        .progress-fill.red { background: var(--accent-red); }
        .stat-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 16px; }
        .stat-item {
            background: var(--bg-tertiary);
            padding: 16px;
            border-radius: 8px;
            text-align: center;
        }
        .stat-value { font-size: 1.5rem; font-weight: 600; color: var(--accent-cyan); }
        .stat-label { font-size: 0.75rem; color: var(--text-secondary); margin-top: 4px; }
        .logs-container { max-height: 400px; overflow-y: auto; }
        .log-entry {
            display: flex;
            align-items: center;
            gap: 12px;
            padding: 12px;
            border-bottom: 1px solid var(--border-color);
            font-size: 0.875rem;
        }
        .log-entry:last-child { border-bottom: none; }
        .log-time { color: var(--text-secondary); font-family: monospace; white-space: nowrap; }
        .log-status { font-size: 1.1rem; }
        .log-status.success { color: var(--accent-green); }
        .log-status.error { color: var(--accent-red); }
        .log-kind {
            background: var(--bg-tertiary);
            padding: 2px 8px;
            border-radius: 4px;
            font-family: monospace;
            font-size: 0.75rem;
            color: var(--accent-cyan);
        }
        .log-model { color: var(--accent-yellow); font-family: monospace; }
        .log-tokens { color: var(--text-secondary); margin-left: auto; font-family: monospace; }
        .log-error { color: var(--accent-red); font-size: 0.75rem; margin-top: 4px; }
        .endpoint-grid { display: grid; gap: 8px; }
        .endpoint {
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 8px 12px;
            background: var(--bg-tertiary);
            border-radius: 6px;
            font-family: monospace;
            font-size: 0.8rem;
        }
        .endpoint-method {
            padding: 2px 6px;
            border-radius: 4px;
            font-size: 0.7rem;
            font-weight: 600;
        }
        .endpoint-method.get { background: var(--accent-green); color: var(--bg-primary); }
        .endpoint-method.post { background: var(--accent-cyan); color: var(--bg-primary); }
        .endpoint-path { color: var(--text-primary); }
        .refresh-indicator {
            position: fixed;
            bottom: 20px;
            right: 20px;
            padding: 8px 16px;
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 20px;
            font-size: 0.75rem;
            color: var(--text-secondary);
        }
        .empty-state {
            text-align: center;
            padding: 40px;
            color: var(--text-secondary);
        }
        .download-btn {
            display: flex;
            align-items: center;
            gap: 6px;
            padding: 8px 16px;
            background: var(--bg-tertiary);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            color: var(--text-primary);
            text-decoration: none;
            font-size: 0.875rem;
            transition: all 0.2s;
        }
        .download-btn:hover {
            background: var(--accent-cyan);
            color: var(--bg-primary);
            border-color: var(--accent-cyan);
        }
        .models-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 8px;
        }
        .model-item {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 10px 12px;
            background: var(--bg-tertiary);
            border-radius: 6px;
            font-family: monospace;
            font-size: 0.85rem;
        }
        .model-item.featured {
            border: 1px solid var(--accent-purple);
            background: rgba(163, 113, 247, 0.1);
        }
        .model-name { color: var(--text-primary); }
        .model-badge {
            font-size: 0.65rem;
            padding: 2px 6px;
            border-radius: 4px;
            font-weight: 600;
            text-transform: uppercase;
        }
        .model-badge.new {
            background: var(--accent-purple);
            color: white;
        }
    </style>
</head>
<body>
    <header>
        <div class="logo">
            <span class="logo-icon">🧞</span>
            <span>Genie <span class="version" id="version">v0.1.0</span></span>
        </div>
        <div style="display: flex; align-items: center; gap: 12px;">
            <a href="/swagger" class="download-btn" style="background: linear-gradient(135deg, #85ea2d 0%, #173647 100%); border-color: #85ea2d;">
                🔧 Swagger UI
            </a>
            <a href="/docs/markdown" download="genie-api-docs.md" class="download-btn">
                📄 Download Docs
            </a>
            <div class="status-badge ok" id="status-badge">
                <span class="status-dot"></span>
                <span id="status-text">Running</span>
            </div>
        </div>
    </header>

    <div class="container">
        <div class="grid">
            <!-- Quota Card -->
            <div class="card">
                <div class="card-title">📊 Quota Status</div>
                <div class="quota-section">
                    <div class="quota-label">
                        <span>Daily Requests</span>
                        <span class="quota-value"><span id="daily-used">0</span> / <span id="daily-limit">1000</span></span>
                    </div>
                    <div class="progress-bar">
                        <div class="progress-fill green" id="daily-progress" style="width: 0%"></div>
                    </div>
                </div>
                <div class="quota-section">
                    <div class="quota-label">
                        <span>Per Minute</span>
                        <span class="quota-value"><span id="minute-used">0</span> / <span id="minute-limit">60</span></span>
                    </div>
                    <div class="progress-bar">
                        <div class="progress-fill green" id="minute-progress" style="width: 0%"></div>
                    </div>
                </div>
                <div class="stat-grid">
                    <div class="stat-item">
                        <div class="stat-value" id="tokens-in">0</div>
                        <div class="stat-label">Input Tokens</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value" id="tokens-out">0</div>
                        <div class="stat-label">Output Tokens</div>
                    </div>
                </div>
            </div>

            <!-- Models Card -->
            <div class="card">
                <div class="card-title">🤖 Available Models</div>
                <div class="models-grid">
                    <div class="model-item featured">
                        <span class="model-name">gemini-3-pro-preview</span>
                        <span class="model-badge new">NEW</span>
                    </div>
                    <div class="model-item featured">
                        <span class="model-name">gemini-3-flash-preview</span>
                        <span class="model-badge new">NEW</span>
                    </div>
                    <div class="model-item">
                        <span class="model-name">gemini-2.5-pro</span>
                    </div>
                    <div class="model-item">
                        <span class="model-name">gemini-2.5-flash</span>
                    </div>
                </div>
                <div class="card-title" style="margin-top: 20px;">🧠 Embeddings (Local)</div>
                <div class="models-grid">
                    <div class="model-item featured">
                        <span class="model-name">all-MiniLM-L6-v2</span>
                        <span class="model-badge new">FREE</span>
                    </div>
                    <div class="model-item">
                        <span class="model-name">384 dimensions</span>
                    </div>
                </div>
                <div class="card-title" style="margin-top: 20px;">🔗 API Endpoints</div>
                <div class="endpoint-grid">
                    <div class="endpoint">
                        <span class="endpoint-method post">POST</span>
                        <span class="endpoint-path">/v1/chat/completions</span>
                    </div>
                    <div class="endpoint">
                        <span class="endpoint-method post">POST</span>
                        <span class="endpoint-path">/v1/completions</span>
                    </div>
                    <div class="endpoint">
                        <span class="endpoint-method post">POST</span>
                        <span class="endpoint-path">/v1/embeddings</span>
                    </div>
                    <div class="endpoint">
                        <span class="endpoint-method get">GET</span>
                        <span class="endpoint-path">/v1/models</span>
                    </div>
                    <div class="endpoint">
                        <span class="endpoint-method post">POST</span>
                        <span class="endpoint-path">/v1/docs/summarize</span>
                    </div>
                    <div class="endpoint">
                        <span class="endpoint-method post">POST</span>
                        <span class="endpoint-path">/v1/rag/query</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- Logs Card -->
        <div class="card" style="margin-top: 20px;">
            <div class="card-title">📝 Recent Requests</div>
            <div class="logs-container" id="logs-container">
                <div class="empty-state">No requests yet...</div>
            </div>
        </div>
    </div>

    <div class="refresh-indicator">
        Auto-refreshing every 2s
    </div>

    <script>
        async function fetchQuota() {
            try {
                const res = await fetch('/v1/quota');
                const data = await res.json();
                
                document.getElementById('daily-used').textContent = data.requests_today;
                document.getElementById('daily-limit').textContent = data.requests_per_day_limit;
                document.getElementById('minute-used').textContent = data.requests_last_minute;
                document.getElementById('minute-limit').textContent = data.requests_per_minute_limit;
                document.getElementById('tokens-in').textContent = data.approx_input_tokens_today.toLocaleString();
                document.getElementById('tokens-out').textContent = data.approx_output_tokens_today.toLocaleString();
                
                const dailyPct = (data.requests_today / data.requests_per_day_limit) * 100;
                const minutePct = (data.requests_last_minute / data.requests_per_minute_limit) * 100;
                
                updateProgress('daily-progress', dailyPct);
                updateProgress('minute-progress', minutePct);
            } catch (e) {
                console.error('Failed to fetch quota:', e);
            }
        }

        function updateProgress(id, pct) {
            const el = document.getElementById(id);
            el.style.width = Math.min(pct, 100) + '%';
            el.classList.remove('green', 'yellow', 'red');
            if (pct > 90) el.classList.add('red');
            else if (pct > 70) el.classList.add('yellow');
            else el.classList.add('green');
        }

        async function fetchHealth() {
            try {
                const res = await fetch('/health');
                const data = await res.json();
                
                document.getElementById('version').textContent = 'v' + data.version;
                
                const badge = document.getElementById('status-badge');
                const statusText = document.getElementById('status-text');
                
                badge.classList.remove('ok', 'degraded', 'error');
                if (data.status === 'ok' && data.gemini_available) {
                    badge.classList.add('ok');
                    statusText.textContent = 'Running';
                } else if (data.gemini_available) {
                    badge.classList.add('degraded');
                    statusText.textContent = 'Degraded';
                } else {
                    badge.classList.add('error');
                    statusText.textContent = 'Gemini Unavailable';
                }
            } catch (e) {
                const badge = document.getElementById('status-badge');
                badge.classList.remove('ok', 'degraded');
                badge.classList.add('error');
                document.getElementById('status-text').textContent = 'Connection Error';
            }
        }

        async function fetchLogs() {
            try {
                const res = await fetch('/v1/quota/logs');
                const logs = await res.json();
                renderLogs(logs);
            } catch (e) {
                console.error('Failed to fetch logs:', e);
            }
        }
        
        function formatTime(timestamp) {
            try {
                const date = new Date(timestamp);
                return date.toLocaleTimeString('en-US', { hour12: false });
            } catch {
                return '???';
            }
        }
        
        function renderLogs(logs) {
            const container = document.getElementById('logs-container');
            if (!logs || logs.length === 0) {
                container.innerHTML = '<div class="empty-state">No requests yet... Make an API call to see activity here.</div>';
                return;
            }
            
            container.innerHTML = logs.map(log => `
                <div class="log-entry">
                    <span class="log-time">${formatTime(log.timestamp)}</span>
                    <span class="log-status ${log.success ? 'success' : 'error'}">${log.success ? '✓' : '✗'}</span>
                    <span class="log-kind">${log.kind}</span>
                    <span class="log-model">${log.model}</span>
                    <span class="log-tokens">${log.approx_input_tokens}→${log.approx_output_tokens}</span>
                    ${log.error_code ? `<div class="log-error">${log.error_code}</div>` : ''}
                </div>
            `).join('');
        }

        // Initial load
        fetchHealth();
        fetchQuota();
        fetchLogs();
        
        // Auto-refresh every 2 seconds
        setInterval(() => {
            fetchHealth();
            fetchQuota();
            fetchLogs();
        }, 2000);
    </script>
</body>
</html>
"#;

/// HTML for Swagger UI - Interactive API documentation
const SWAGGER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Genie API - Swagger UI</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        body { margin: 0; background: #1a1a2e; }
        .swagger-ui { background: #1a1a2e; }
        .swagger-ui .topbar { display: none; }
        .swagger-ui .info .title { color: #e6edf3; }
        .swagger-ui .info .description, .swagger-ui .info li, .swagger-ui .info p { color: #8b949e; }
        .swagger-ui .scheme-container { background: #161b22; box-shadow: none; }
        .swagger-ui .opblock-tag { color: #e6edf3; border-bottom-color: #30363d; }
        .swagger-ui .opblock { background: #21262d; border-color: #30363d; }
        .swagger-ui .opblock .opblock-summary { border-color: #30363d; }
        .swagger-ui .opblock .opblock-summary-description { color: #8b949e; }
        .swagger-ui .opblock.opblock-post { background: rgba(73, 204, 144, 0.1); border-color: #3fb950; }
        .swagger-ui .opblock.opblock-post .opblock-summary { border-color: #3fb950; }
        .swagger-ui .opblock.opblock-get { background: rgba(88, 166, 255, 0.1); border-color: #58a6ff; }
        .swagger-ui .opblock.opblock-get .opblock-summary { border-color: #58a6ff; }
        .swagger-ui .btn { background: #21262d; color: #e6edf3; border-color: #30363d; }
        .swagger-ui .btn:hover { background: #30363d; }
        .swagger-ui .btn.execute { background: #238636; border-color: #238636; }
        .swagger-ui .btn.execute:hover { background: #2ea043; }
        .swagger-ui select { background: #21262d; color: #e6edf3; border-color: #30363d; }
        .swagger-ui input[type=text], .swagger-ui textarea { background: #0d1117; color: #e6edf3; border-color: #30363d; }
        .swagger-ui .model-box { background: #161b22; }
        .swagger-ui .model { color: #8b949e; }
        .swagger-ui .prop-type { color: #58a6ff; }
        .swagger-ui .response-col_status { color: #e6edf3; }
        .swagger-ui table thead tr th { color: #8b949e; border-color: #30363d; }
        .swagger-ui table tbody tr td { color: #e6edf3; border-color: #30363d; }
        .swagger-ui .response-col_description { color: #8b949e; }
        .swagger-ui .responses-inner { background: #161b22; }
        .swagger-ui .opblock-body pre { background: #0d1117; color: #e6edf3; }
        .swagger-ui .highlight-code { background: #0d1117; }
        .swagger-ui .microlight { background: #0d1117 !important; color: #e6edf3 !important; }
        .header-links {
            display: flex;
            gap: 16px;
            padding: 16px 24px;
            background: #161b22;
            border-bottom: 1px solid #30363d;
        }
        .header-links a {
            color: #58a6ff;
            text-decoration: none;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            font-size: 0.875rem;
        }
        .header-links a:hover { text-decoration: underline; }
        .header-links .logo {
            color: #e6edf3;
            font-weight: 600;
            font-size: 1.1rem;
            margin-right: auto;
        }
    </style>
</head>
<body>
    <div class="header-links">
        <span class="logo">🧞 Genie API</span>
        <a href="/dashboard">Dashboard</a>
        <a href="/openapi.json">OpenAPI JSON</a>
        <a href="/docs/markdown">Download Docs</a>
    </div>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
        window.onload = function() {
            SwaggerUIBundle({
                url: "/openapi.json",
                dom_id: '#swagger-ui',
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIBundle.SwaggerUIStandalonePreset
                ],
                layout: "BaseLayout",
                deepLinking: true,
                defaultModelsExpandDepth: 1,
                defaultModelExpandDepth: 1,
                docExpansion: "list",
                filter: true,
                showExtensions: true,
                showCommonExtensions: true,
                tryItOutEnabled: true
            });
        };
    </script>
</body>
</html>
"#;

/// Swagger UI endpoint - Interactive API documentation
async fn swagger_ui() -> Html<&'static str> {
    Html(SWAGGER_HTML)
}

/// Health check endpoint with extended status
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Server health status", body = HealthResponse)
    )
)]
#[instrument(skip(state))]
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let gemini_available = state.gemini.check_available().await;
    let config = state.config.read().await;
    
    // Get quota stats for health info
    let quota_info = match state.quota.get_stats().await {
        Ok(stats) => {
            let minute_remaining = config.quota.per_minute.saturating_sub(stats.requests_last_minute);
            let day_remaining = config.quota.per_day.saturating_sub(stats.requests_today);
            serde_json::json!({
                "minute_remaining": minute_remaining,
                "day_remaining": day_remaining,
                "requests_today": stats.requests_today,
                "requests_last_minute": stats.requests_last_minute
            })
        }
        Err(_) => serde_json::json!(null)
    };

    let ready = gemini_available && quota_info != serde_json::json!(null);

    Json(serde_json::json!({
        "status": if gemini_available { "ok" } else { "degraded" },
        "ready": ready,
        "version": env!("CARGO_PKG_VERSION"),
        "gemini_available": gemini_available,
        "quota": quota_info,
        "server": {
            "host": config.server.host,
            "port": config.server.port
        }
    }))
}

/// List available models
#[utoipa::path(
    get,
    path = "/v1/models",
    tag = "OpenAI Compatible",
    responses(
        (status = 200, description = "List of available models")
    )
)]
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
            // Gemini 3 models (preview)
            {
                "id": "gemini-3-pro-preview",
                "object": "model",
                "owned_by": "google",
                "permission": []
            },
            {
                "id": "gemini-3-flash-preview",
                "object": "model",
                "owned_by": "google",
                "permission": []
            },
            // Gemini 2.5 models
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
#[utoipa::path(
    get,
    path = "/v1/quota",
    tag = "Quota",
    responses(
        (status = 200, description = "Current quota usage status", body = QuotaStatus),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
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

/// Get recent usage logs for the dashboard
#[instrument(skip(state))]
async fn get_usage_logs(State(state): State<Arc<AppState>>) -> Result<Json<Vec<UsageEvent>>, AppError> {
    let events = state.quota.get_recent_events(50).await
        .map_err(|e| AppError::InternalError(e.to_string()))?;
    Ok(Json(events))
}

/// OpenAI-compatible chat completions endpoint (supports streaming)
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    tag = "OpenAI Compatible",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Chat completion response (or SSE stream if stream=true)", body = ChatCompletionResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Quota exceeded", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(model = %request.model, messages = request.messages.len()))]
async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, AppError> {
    debug!("Received chat completion request (stream={})", request.stream);

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

    // Handle streaming vs non-streaming
    if request.stream {
        // Streaming response using SSE
        let model = request.model.clone();
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        
        // Call Gemini (we'll simulate streaming by chunking the response)
        let result = state.gemini.call_text(&gemini_req).await;

        match result {
            Ok(response) => {
                let response_chars = response.text.len();

                // Record successful usage
                let event = UsageEvent::new(
                    &model,
                    RequestKind::Chat,
                    prompt_chars,
                    response_chars,
                    true,
                );
                if let Err(e) = state.quota.record_event(event).await {
                    error!("Failed to record usage event: {}", e);
                }

                // Create streaming response by simulating chunks
                // In a real implementation, this would stream from Gemini directly
                let chunks = simulate_streaming_chunks(&completion_id, &response.model, &response.text);
                
                let stream = stream::iter(chunks.into_iter().map(Ok::<_, std::convert::Infallible>));
                let body = Body::from_stream(stream);

                info!("Chat completion streaming started");
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "keep-alive")
                    .body(body)
                    .unwrap())
            }
            Err(e) => {
                // Record failed usage
                let event = UsageEvent::new(&model, RequestKind::Chat, prompt_chars, 0, false)
                    .with_error(e.to_string());
                if let Err(re) = state.quota.record_event(event).await {
                    error!("Failed to record usage event: {}", re);
                }

                Err(e.into())
            }
        }
    } else {
        // Non-streaming response
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
                Ok(Json(completion).into_response())
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
}

/// Simulate streaming chunks from a complete response
/// In a real implementation, this would stream directly from the Gemini CLI
fn simulate_streaming_chunks(id: &str, model: &str, text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    
    // Initial chunk with role
    let initial = ChatCompletionChunk::new_initial(id, model);
    chunks.push(initial.to_sse());
    
    // Split text into smaller chunks (simulating word-by-word streaming)
    let words: Vec<&str> = text.split_whitespace().collect();
    let chunk_size = 3; // words per chunk
    
    for chunk_words in words.chunks(chunk_size) {
        let chunk_text = chunk_words.join(" ") + " ";
        let chunk = ChatCompletionChunk::new(id, model, &chunk_text);
        chunks.push(chunk.to_sse());
    }
    
    // Final chunk with finish_reason
    let final_chunk = ChatCompletionChunk::new_final(id, model);
    chunks.push(final_chunk.to_sse());
    
    // [DONE] marker
    chunks.push("data: [DONE]\n\n".to_string());
    
    chunks
}

/// JSON completion endpoint (guaranteed JSON output)
#[utoipa::path(
    post,
    path = "/v1/json",
    tag = "OpenAI Compatible",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "JSON completion response", body = ChatCompletionResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Quota exceeded", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
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

/// OpenAI-compatible text completions endpoint (simpler than chat)
#[utoipa::path(
    post,
    path = "/v1/completions",
    tag = "OpenAI Compatible",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Text completion response", body = CompletionResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Quota exceeded", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(model = %request.model))]
async fn text_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, AppError> {
    debug!("Received text completion request");

    // Validate request
    if request.prompt.is_empty() {
        return Err(AppError::InvalidRequest(
            "prompt cannot be empty".to_string(),
        ));
    }

    // Check quota
    state
        .quota
        .check_before_request(&RequestKind::Ask, &request.model)
        .await?;

    let prompt_chars = request.prompt.len() + request.system.as_ref().map(|s| s.len()).unwrap_or(0);

    // Build Gemini request
    let mut gemini_req = GeminiRequest::new(&request.model, &request.prompt);
    if let Some(sys) = &request.system {
        gemini_req = gemini_req.with_system_prompt(sys.clone());
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
                RequestKind::Ask,
                prompt_chars,
                response_chars,
                true,
            );
            if let Err(e) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", e);
            }

            // Build OpenAI-compatible response
            let completion = CompletionResponse {
                id: format!("cmpl-{}", Uuid::new_v4()),
                object: "text_completion".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: response.model,
                choices: vec![CompletionChoice {
                    index: 0,
                    text: response.text,
                    finish_reason: "stop".to_string(),
                }],
                usage: Some(Usage {
                    prompt_tokens: (prompt_chars / 4) as u32,
                    completion_tokens: (response_chars / 4) as u32,
                    total_tokens: ((prompt_chars + response_chars) / 4) as u32,
                }),
            };

            info!("Text completion successful");
            Ok(Json(completion))
        }
        Err(e) => {
            // Record failed usage
            let event = UsageEvent::new(&request.model, RequestKind::Ask, prompt_chars, 0, false)
                .with_error(e.to_string());
            if let Err(re) = state.quota.record_event(event).await {
                error!("Failed to record usage event: {}", re);
            }

            Err(e.into())
        }
    }
}

/// OpenAI-compatible embeddings endpoint (local, free)
#[utoipa::path(
    post,
    path = "/v1/embeddings",
    tag = "OpenAI Compatible",
    request_body = EmbeddingRequest,
    responses(
        (status = 200, description = "Embedding vectors", body = EmbeddingResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(model = %request.model, inputs = request.input.len()))]
async fn create_embeddings(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppError> {
    debug!("Received embeddings request for {} inputs", request.input.len());

    // Validate request
    if request.input.is_empty() {
        return Err(AppError::InvalidRequest(
            "input cannot be empty".to_string(),
        ));
    }

    // Check if embeddings are enabled in config
    let config = state.config.read().await;
    if !config.embeddings.enabled {
        return Err(AppError::InvalidRequest(
            "Embeddings are disabled. Enable them in config with [embeddings] enabled = true".to_string(),
        ));
    }
    drop(config);

    // Get or lazily initialize embeddings for the requested model
    let embeddings = state.get_or_init_embeddings(&request.model).await.map_err(|e| {
        error!("Failed to initialize embeddings model '{}': {}", request.model, e);
        AppError::InternalError(format!("Failed to initialize embeddings: {}", e))
    })?;

    // Convert input to vector of strings
    let texts = request.input.into_vec();

    // Estimate tokens for usage tracking
    let prompt_tokens = LocalEmbeddings::estimate_tokens(&texts);

    // Generate embeddings
    let embedding_vectors = embeddings.embed(texts.clone()).map_err(|e| {
        error!("Embeddings generation failed: {}", e);
        AppError::InternalError(format!("Embeddings generation failed: {}", e))
    })?;

    // Build OpenAI-compatible response
    let response = EmbeddingResponse::new(
        embedding_vectors,
        embeddings.model_name().to_string(),
        prompt_tokens,
    );

    info!(
        "Embeddings generated: {} vectors with {} dimensions",
        response.data.len(),
        embeddings.dimensions()
    );

    Ok(Json(response))
}

// === Document Summarization Types ===

/// Request for document summarization
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DocsSummarizeRequest {
    /// Path to the PDF file
    pub path: String,
    /// Mode: "pdf" for simple document, "book" for chapter detection
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Summary style
    #[serde(default)]
    pub style: Option<String>,
    /// Output language
    #[serde(default)]
    pub language: Option<String>,
}

fn default_mode() -> String {
    "pdf".to_string()
}

/// Response for document summarization
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DocsSummarizeResponse {
    Document(DocumentSummary),
    Book(BookSummary),
}

/// Document summarization endpoint
#[utoipa::path(
    post,
    path = "/v1/docs/summarize",
    tag = "Documents",
    request_body = DocsSummarizeRequest,
    responses(
        (status = 200, description = "Document summary"),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Quota exceeded", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(path = %request.path, mode = %request.mode))]
async fn summarize_docs(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DocsSummarizeRequest>,
) -> Result<Json<DocsSummarizeResponse>, AppError> {
    info!("Received docs summarization request");

    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(AppError::InvalidRequest(format!(
            "File not found: {}",
            request.path
        )));
    }

    // Build options
    let mut options = SummarizeOptions::new();
    if let Some(style) = &request.style {
        options = options.with_style(match style.as_str() {
            "detailed" => SummaryStyle::Detailed,
            "exam-notes" => SummaryStyle::ExamNotes,
            "bullet" => SummaryStyle::Bullet,
            _ => SummaryStyle::Concise,
        });
    }
    if let Some(lang) = &request.language {
        options = options.with_language(lang);
    }

    // Check quota
    let kind = if request.mode == "book" {
        RequestKind::SummarizeBook
    } else {
        RequestKind::SummarizePdf
    };
    state
        .quota
        .check_before_request(&kind, "gemini-2.5-pro")
        .await?;

    let response = if request.mode == "book" {
        let summary = docs::summarize_book(&state.gemini, &path, &options)
            .await
            .map_err(|e| AppError::InternalError(e.to_string()))?;

        // Record usage
        let event = UsageEvent::new("gemini-2.5-pro", kind, 0, 0, true);
        if let Err(e) = state.quota.record_event(event).await {
            error!("Failed to record usage event: {}", e);
        }

        DocsSummarizeResponse::Book(summary)
    } else {
        let summary = docs::summarize_pdf(&state.gemini, &path, &options)
            .await
            .map_err(|e| AppError::InternalError(e.to_string()))?;

        // Record usage
        let event = UsageEvent::new("gemini-2.5-pro", kind, 0, 0, true);
        if let Err(e) = state.quota.record_event(event).await {
            error!("Failed to record usage event: {}", e);
        }

        DocsSummarizeResponse::Document(summary)
    };

    info!("Document summarization successful");
    Ok(Json(response))
}

// === Repo Summarization Types ===

/// Request for repo summarization
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RepoSummaryRequest {
    /// Path to the repository
    pub path: String,
    /// Maximum files to process (optional)
    #[serde(default)]
    pub max_files: Option<u32>,
}

/// Repo summarization endpoint
#[utoipa::path(
    post,
    path = "/v1/repo/summary",
    tag = "Repository",
    request_body = RepoSummaryRequest,
    responses(
        (status = 200, description = "Repository summary"),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 429, description = "Quota exceeded", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(path = %request.path))]
async fn summarize_repo(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RepoSummaryRequest>,
) -> Result<Json<RepoSummary>, AppError> {
    info!("Received repo summarization request");

    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(AppError::InvalidRequest(format!(
            "Path not found: {}",
            request.path
        )));
    }

    if !path.is_dir() {
        return Err(AppError::InvalidRequest(format!(
            "Path is not a directory: {}",
            request.path
        )));
    }

    // Build options
    let mut options = RepoOptions::default();
    if let Some(max_files) = request.max_files {
        options = options.with_max_files(max_files);
    }

    // Check quota
    state
        .quota
        .check_before_request(&RequestKind::RepoSummary, "gemini-2.5-pro")
        .await?;

    let summary = repo::summarize_repo(&state.gemini, &path, &options)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    // Record usage
    let event = UsageEvent::new("gemini-2.5-pro", RequestKind::RepoSummary, 0, 0, true);
    if let Err(e) = state.quota.record_event(event).await {
        error!("Failed to record usage event: {}", e);
    }

    info!("Repo summarization successful");
    Ok(Json(summary))
}

// === RAG Endpoints ===

/// Request for RAG ingest
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RagIngestRequest {
    pub collection_id: String,
    pub path: String,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub chunk_size: Option<usize>,
}

/// Response for RAG ingest
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RagIngestResponse {
    pub documents_ingested: u32,
    pub chunks_created: u32,
    pub errors: Vec<String>,
}

/// Request for RAG query
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct RagQueryRequest {
    pub collection_id: String,
    pub question: String,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub return_sources: Option<bool>,
}

/// List RAG collections
#[utoipa::path(
    get,
    path = "/v1/rag/collections",
    tag = "RAG",
    responses(
        (status = 200, description = "List of RAG collections"),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(_state))]
async fn rag_list_collections(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<RagCollection>>, AppError> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| AppError::InternalError("Could not determine RAG database path".to_string()))?;

    let manager = RagManager::new(&db_path)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    let collections = manager
        .list_collections()
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    Ok(Json(collections))
}

/// Ingest documents into RAG collection
#[utoipa::path(
    post,
    path = "/v1/rag/ingest",
    tag = "RAG",
    request_body = RagIngestRequest,
    responses(
        (status = 200, description = "Ingest statistics", body = RagIngestResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(collection = %request.collection_id, path = %request.path))]
async fn rag_ingest(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RagIngestRequest>,
) -> Result<Json<RagIngestResponse>, AppError> {
    info!("Received RAG ingest request");

    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| AppError::InternalError("Could not determine RAG database path".to_string()))?;

    let manager = RagManager::new(&db_path)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    let path = PathBuf::from(&request.path);
    if !path.exists() {
        return Err(AppError::InvalidRequest(format!("Path not found: {}", request.path)));
    }

    let mut options = IngestOptions::default();
    if let Some(p) = &request.pattern {
        options.pattern = Some(p.clone());
    }
    if let Some(size) = request.chunk_size {
        options.chunk_size = size;
    }

    // Get cached embeddings if available (for semantic search during RAG)
    let cached_embeddings = state.get_cached_embeddings().await;
    let embeddings_ref = cached_embeddings.as_deref();

    let stats = manager
        .ingest(
            &request.collection_id,
            &path,
            &options,
            &state.gemini,
            embeddings_ref,
        )
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    info!("RAG ingest complete: {} docs, {} chunks", stats.documents_ingested, stats.chunks_created);

    Ok(Json(RagIngestResponse {
        documents_ingested: stats.documents_ingested,
        chunks_created: stats.chunks_created,
        errors: stats.errors,
    }))
}

/// Query RAG collection
#[utoipa::path(
    post,
    path = "/v1/rag/query",
    tag = "RAG",
    request_body = RagQueryRequest,
    responses(
        (status = 200, description = "Query response with answer and sources"),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    )
)]
#[instrument(skip(state, request), fields(collection = %request.collection_id))]
async fn rag_query(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>, AppError> {
    info!("Received RAG query request");

    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| AppError::InternalError("Could not determine RAG database path".to_string()))?;

    let manager = RagManager::new(&db_path)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    let mut options = QueryOptions::default();
    if let Some(k) = request.top_k {
        options.top_k = k;
    }
    if let Some(return_sources) = request.return_sources {
        options.return_sources = return_sources;
    }

    // Get cached embeddings if available (for semantic search during RAG)
    let cached_embeddings = state.get_cached_embeddings().await;
    let embeddings_ref = cached_embeddings.as_deref();

    let response = manager
        .query(
            &request.collection_id,
            &request.question,
            &options,
            &state.gemini,
            embeddings_ref,
        )
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    info!("RAG query complete");
    Ok(Json(response))
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
    EmbeddingsError(EmbeddingsError),
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

impl From<EmbeddingsError> for AppError {
    fn from(e: EmbeddingsError) -> Self {
        AppError::EmbeddingsError(e)
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
            AppError::EmbeddingsError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::internal_error(e.to_string()),
            ),
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
