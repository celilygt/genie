//! # Genie Core
//!
//! Core library for Genie - a local Gemini-as-a-service backend.
//!
//! This crate provides:
//! - Configuration management
//! - Gemini CLI process wrapper
//! - Quota tracking and enforcement
//! - HTTP API server (OpenAI-compatible)
//! - PDF/Book summarization
//! - Repository summarization
//! - Prompt templates gallery
//! - RAG (Retrieval-Augmented Generation)
//! - Shared data models

pub mod config;
pub mod docs;
pub mod gemini;
pub mod model;
pub mod pdf;
pub mod quota;
pub mod rag;
pub mod repo;
pub mod server;
pub mod templates;

pub use config::{Config, QuotaConfig};
pub use gemini::{GeminiClient, GeminiError, GeminiRequest, GeminiResponse};
pub use model::*;
pub use quota::{QuotaError, QuotaManager, UsageEvent, UsageStats};
pub use rag::{RagError, RagManager, RagQueryResponse, IngestStats};
