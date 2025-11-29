//! # Genie Core
//!
//! Core library for Genie - a local Gemini-as-a-service backend.
//!
//! This crate provides:
//! - Configuration management
//! - Gemini CLI process wrapper
//! - Quota tracking and enforcement
//! - HTTP API server (OpenAI-compatible)
//! - Shared data models

pub mod config;
pub mod gemini;
pub mod model;
pub mod quota;
pub mod server;

pub use config::Config;
pub use gemini::{GeminiClient, GeminiError, GeminiRequest, GeminiResponse};
pub use model::*;
pub use quota::{QuotaError, QuotaManager, UsageEvent};
