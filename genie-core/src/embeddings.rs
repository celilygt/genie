//! Local embeddings module using fastembed.
//!
//! This module provides a completely free, local embeddings solution
//! using ONNX-based models via fastembed-rs. No API calls or costs.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Mutex;
use thiserror::Error;
use tracing::{debug, info};

/// Errors that can occur in embeddings operations
#[derive(Error, Debug)]
pub enum EmbeddingsError {
    #[error("Failed to initialize embedding model: {0}")]
    InitError(String),

    #[error("Failed to generate embeddings: {0}")]
    EmbedError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl From<anyhow::Error> for EmbeddingsError {
    fn from(err: anyhow::Error) -> Self {
        EmbeddingsError::EmbedError(err.to_string())
    }
}

/// Information about an embedding model
#[derive(Debug, Clone)]
pub struct EmbeddingModelInfo {
    /// Model name (e.g., "all-MiniLM-L6-v2")
    pub name: String,
    /// Output embedding dimensions
    pub dimensions: usize,
    /// Model description
    pub description: String,
}

/// Local embeddings generator using fastembed
pub struct LocalEmbeddings {
    model: Mutex<TextEmbedding>,
    model_info: EmbeddingModelInfo,
}

impl LocalEmbeddings {
    /// Create a new LocalEmbeddings with the default model (AllMiniLML6V2)
    pub fn new() -> Result<Self, EmbeddingsError> {
        Self::with_model(EmbeddingModel::AllMiniLML6V2)
    }

    /// Create LocalEmbeddings with a specific fastembed model
    pub fn with_model(model: EmbeddingModel) -> Result<Self, EmbeddingsError> {
        info!("Initializing local embedding model: {:?}", model);

        let model_info = get_model_info(&model);

        let text_embedding = TextEmbedding::try_new(
            InitOptions::new(model).with_show_download_progress(true),
        )
        .map_err(|e| EmbeddingsError::InitError(e.to_string()))?;

        info!(
            "Embedding model initialized: {} ({} dimensions)",
            model_info.name, model_info.dimensions
        );

        Ok(Self {
            model: Mutex::new(text_embedding),
            model_info,
        })
    }

    /// Create LocalEmbeddings from an OpenAI model name
    /// Maps OpenAI model names to fastembed equivalents
    pub fn from_openai_model(model_name: &str) -> Result<Self, EmbeddingsError> {
        let fastembed_model = map_openai_to_fastembed(model_name);
        Self::with_model(fastembed_model)
    }

    /// Get information about the current model
    pub fn model_info(&self) -> &EmbeddingModelInfo {
        &self.model_info
    }

    /// Get the model name
    pub fn model_name(&self) -> &str {
        &self.model_info.name
    }

    /// Get the embedding dimensions
    pub fn dimensions(&self) -> usize {
        self.model_info.dimensions
    }

    /// Generate embeddings for a list of texts
    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingsError> {
        if texts.is_empty() {
            return Err(EmbeddingsError::InvalidInput(
                "Input texts cannot be empty".to_string(),
            ));
        }

        debug!("Generating embeddings for {} texts", texts.len());

        // Convert to &str references for fastembed
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        // Acquire lock and generate embeddings
        let mut model = self
            .model
            .lock()
            .map_err(|e| EmbeddingsError::EmbedError(format!("Failed to acquire lock: {}", e)))?;

        let embeddings = model
            .embed(refs, None)
            .map_err(|e| EmbeddingsError::EmbedError(e.to_string()))?;

        debug!(
            "Generated {} embeddings with {} dimensions each",
            embeddings.len(),
            embeddings.first().map(|e| e.len()).unwrap_or(0)
        );

        Ok(embeddings)
    }

    /// Generate embedding for a single text
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbeddingsError> {
        let embeddings = self.embed(vec![text.to_string()])?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingsError::EmbedError("No embedding generated".to_string()))
    }

    /// Estimate token count (rough approximation: chars / 4)
    pub fn estimate_tokens(texts: &[String]) -> u32 {
        let total_chars: usize = texts.iter().map(|t| t.len()).sum();
        (total_chars / 4) as u32
    }
}

/// Map OpenAI model names to fastembed equivalents
fn map_openai_to_fastembed(model_name: &str) -> EmbeddingModel {
    match model_name.to_lowercase().as_str() {
        // OpenAI model mappings
        "text-embedding-ada-002" => EmbeddingModel::AllMiniLML6V2,
        "text-embedding-3-small" => EmbeddingModel::BGESmallENV15,
        "text-embedding-3-large" => EmbeddingModel::BGEBaseENV15,

        // Direct fastembed model names (allow users to specify)
        "all-minilm-l6-v2" | "allminiml6v2" => EmbeddingModel::AllMiniLML6V2,
        "bge-small-en-v1.5" | "bgesmallenv15" => EmbeddingModel::BGESmallENV15,
        "bge-base-en-v1.5" | "bgebaseenv15" => EmbeddingModel::BGEBaseENV15,
        "bge-large-en-v1.5" | "bgelargeenv15" => EmbeddingModel::BGELargeENV15,
        "multilingual-e5-small" | "multilinguale5small" => EmbeddingModel::MultilingualE5Small,
        "multilingual-e5-base" | "multilinguale5base" => EmbeddingModel::MultilingualE5Base,

        // Default fallback
        _ => EmbeddingModel::AllMiniLML6V2,
    }
}

/// Get model information for a fastembed model
fn get_model_info(model: &EmbeddingModel) -> EmbeddingModelInfo {
    match model {
        EmbeddingModel::AllMiniLML6V2 | EmbeddingModel::AllMiniLML6V2Q => EmbeddingModelInfo {
            name: "all-MiniLM-L6-v2".to_string(),
            dimensions: 384,
            description: "Fast, good quality general-purpose embeddings".to_string(),
        },
        EmbeddingModel::AllMiniLML12V2 | EmbeddingModel::AllMiniLML12V2Q => EmbeddingModelInfo {
            name: "all-MiniLM-L12-v2".to_string(),
            dimensions: 384,
            description: "Higher quality MiniLM variant".to_string(),
        },
        EmbeddingModel::BGESmallENV15 | EmbeddingModel::BGESmallENV15Q => EmbeddingModelInfo {
            name: "bge-small-en-v1.5".to_string(),
            dimensions: 384,
            description: "BGE small model, good quality".to_string(),
        },
        EmbeddingModel::BGEBaseENV15 | EmbeddingModel::BGEBaseENV15Q => EmbeddingModelInfo {
            name: "bge-base-en-v1.5".to_string(),
            dimensions: 768,
            description: "BGE base model, high quality".to_string(),
        },
        EmbeddingModel::BGELargeENV15 | EmbeddingModel::BGELargeENV15Q => EmbeddingModelInfo {
            name: "bge-large-en-v1.5".to_string(),
            dimensions: 1024,
            description: "BGE large model, highest quality".to_string(),
        },
        EmbeddingModel::MultilingualE5Small => EmbeddingModelInfo {
            name: "multilingual-e5-small".to_string(),
            dimensions: 384,
            description: "Multilingual support, 100+ languages".to_string(),
        },
        EmbeddingModel::MultilingualE5Base => EmbeddingModelInfo {
            name: "multilingual-e5-base".to_string(),
            dimensions: 768,
            description: "Multilingual base model".to_string(),
        },
        EmbeddingModel::MultilingualE5Large => EmbeddingModelInfo {
            name: "multilingual-e5-large".to_string(),
            dimensions: 1024,
            description: "Multilingual large model".to_string(),
        },
        // Default for other models
        _ => EmbeddingModelInfo {
            name: format!("{:?}", model),
            dimensions: 384, // Default assumption
            description: "Embedding model".to_string(),
        },
    }
}

/// List available embedding models
pub fn list_available_models() -> Vec<EmbeddingModelInfo> {
    vec![
        get_model_info(&EmbeddingModel::AllMiniLML6V2),
        get_model_info(&EmbeddingModel::BGESmallENV15),
        get_model_info(&EmbeddingModel::BGEBaseENV15),
        get_model_info(&EmbeddingModel::BGELargeENV15),
        get_model_info(&EmbeddingModel::MultilingualE5Small),
        get_model_info(&EmbeddingModel::MultilingualE5Base),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_model_mapping() {
        // These should map to valid fastembed models
        let model = map_openai_to_fastembed("text-embedding-ada-002");
        assert!(matches!(model, EmbeddingModel::AllMiniLML6V2));

        let model = map_openai_to_fastembed("text-embedding-3-small");
        assert!(matches!(model, EmbeddingModel::BGESmallENV15));

        let model = map_openai_to_fastembed("unknown-model");
        assert!(matches!(model, EmbeddingModel::AllMiniLML6V2)); // Default
    }

    #[test]
    fn test_model_info() {
        let info = get_model_info(&EmbeddingModel::AllMiniLML6V2);
        assert_eq!(info.dimensions, 384);
        assert_eq!(info.name, "all-MiniLM-L6-v2");
    }

    #[test]
    fn test_estimate_tokens() {
        let texts = vec!["Hello world".to_string(), "Test".to_string()];
        let tokens = LocalEmbeddings::estimate_tokens(&texts);
        assert!(tokens > 0);
    }
}
