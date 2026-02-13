//! RAG (Retrieval-Augmented Generation) module.
//!
//! This module provides a simple RAG subsystem for ingesting documents,
//! storing embeddings, and querying for relevant context.
//!
//! Uses fastembed for local, free embeddings generation.

use crate::embeddings::LocalEmbeddings;
use crate::gemini::GeminiClient;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur in RAG operations
#[derive(Error, Debug)]
pub enum RagError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Collection not found: {0}")]
    CollectionNotFound(String),

    #[error("Document not found: {0}")]
    DocumentNotFound(String),

    #[error("File error: {0}")]
    FileError(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A RAG collection containing documents and their chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub document_count: u32,
    pub chunk_count: u32,
}

/// Metadata about an ingested document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub collection_id: String,
    pub path: String,
    pub title: Option<String>,
    pub chunk_count: u32,
    pub created_at: String,
}

/// A chunk of text from a document with its embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub collection_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub embedding: Vec<f32>,
    pub order: i32,
}

/// Query result with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub chunk: Chunk,
    pub score: f32,
    pub document_path: String,
}

/// RAG query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryResponse {
    pub answer: String,
    pub sources: Vec<QueryResult>,
}

/// Options for ingesting documents
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// File pattern to match (e.g., "*.txt", "*.pdf")
    pub pattern: Option<String>,
    /// Approximate chunk size in characters
    pub chunk_size: usize,
    /// Whether to process PDFs
    pub include_pdfs: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            pattern: None,
            chunk_size: 1000,
            include_pdfs: true,
        }
    }
}

/// Options for querying
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// Number of top results to retrieve
    pub top_k: usize,
    /// Whether to include sources in response
    pub return_sources: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            top_k: 5,
            return_sources: true,
        }
    }
}

/// RAG Manager for handling collections, documents, and queries
pub struct RagManager {
    pool: SqlitePool,
}

impl RagManager {
    /// Create a new RAG manager with the given database pool
    pub async fn new(db_path: &Path) -> Result<Self, RagError> {
        // Ensure parent directory exists (SQLite won't create it)
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    RagError::Io(std::io::Error::other(format!(
                        "Failed to create RAG database directory {}: {}",
                        parent.display(),
                        e
                    )))
                })?;
            }
        }

        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&url).await?;

        // Initialize tables
        Self::init_tables(&pool).await?;

        Ok(Self { pool })
    }

    /// Initialize database tables
    async fn init_tables(pool: &SqlitePool) -> Result<(), RagError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rag_collections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rag_documents (
                id TEXT PRIMARY KEY,
                collection_id TEXT NOT NULL,
                path TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (collection_id) REFERENCES rag_collections(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS rag_chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                collection_id TEXT NOT NULL,
                text TEXT NOT NULL,
                embedding BLOB,
                chunk_order INTEGER NOT NULL,
                FOREIGN KEY (document_id) REFERENCES rag_documents(id),
                FOREIGN KEY (collection_id) REFERENCES rag_collections(id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create indexes for faster querying
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_chunks_collection ON rag_chunks(collection_id)
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_chunks_document ON rag_chunks(document_id)
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Create a new collection
    pub async fn create_collection(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<RagCollection, RagError> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO rag_collections (id, name, description, created_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        Ok(RagCollection {
            id,
            name: name.to_string(),
            description: description.map(String::from),
            created_at,
            document_count: 0,
            chunk_count: 0,
        })
    }

    /// List all collections
    pub async fn list_collections(&self) -> Result<Vec<RagCollection>, RagError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                c.id, c.name, c.description, c.created_at,
                (SELECT COUNT(*) FROM rag_documents WHERE collection_id = c.id) as doc_count,
                (SELECT COUNT(*) FROM rag_chunks WHERE collection_id = c.id) as chunk_count
            FROM rag_collections c
            ORDER BY c.created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let collections = rows
            .iter()
            .map(|row| RagCollection {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                document_count: row.get::<i64, _>("doc_count") as u32,
                chunk_count: row.get::<i64, _>("chunk_count") as u32,
            })
            .collect();

        Ok(collections)
    }

    /// Get a collection by ID or name
    pub async fn get_collection(&self, id_or_name: &str) -> Result<RagCollection, RagError> {
        let row = sqlx::query(
            r#"
            SELECT 
                c.id, c.name, c.description, c.created_at,
                (SELECT COUNT(*) FROM rag_documents WHERE collection_id = c.id) as doc_count,
                (SELECT COUNT(*) FROM rag_chunks WHERE collection_id = c.id) as chunk_count
            FROM rag_collections c
            WHERE c.id = ? OR c.name = ?
            "#,
        )
        .bind(id_or_name)
        .bind(id_or_name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| RagError::CollectionNotFound(id_or_name.to_string()))?;

        Ok(RagCollection {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            created_at: row.get("created_at"),
            document_count: row.get::<i64, _>("doc_count") as u32,
            chunk_count: row.get::<i64, _>("chunk_count") as u32,
        })
    }

    /// Delete a collection and all its documents/chunks
    pub async fn delete_collection(&self, id_or_name: &str) -> Result<(), RagError> {
        let collection = self.get_collection(id_or_name).await?;

        // Delete chunks first
        sqlx::query("DELETE FROM rag_chunks WHERE collection_id = ?")
            .bind(&collection.id)
            .execute(&self.pool)
            .await?;

        // Delete documents
        sqlx::query("DELETE FROM rag_documents WHERE collection_id = ?")
            .bind(&collection.id)
            .execute(&self.pool)
            .await?;

        // Delete collection
        sqlx::query("DELETE FROM rag_collections WHERE id = ?")
            .bind(&collection.id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Ingest documents from a path into a collection
    /// 
    /// If `embeddings` is provided, real vector embeddings will be generated.
    /// Otherwise, text-based search will be used.
    pub async fn ingest(
        &self,
        collection_id: &str,
        path: &Path,
        options: &IngestOptions,
        _gemini: &GeminiClient,
        embeddings: Option<&LocalEmbeddings>,
    ) -> Result<IngestStats, RagError> {
        let collection = self.get_collection(collection_id).await?;
        let mut stats = IngestStats::default();

        // Walk the path and collect files
        let files = collect_files(path, options)?;

        for file_path in files {
            match self.ingest_file(&collection.id, &file_path, options, embeddings).await {
                Ok((doc_id, chunk_count)) => {
                    stats.documents_ingested += 1;
                    stats.chunks_created += chunk_count;
                    info!("Ingested {} ({} chunks)", file_path.display(), chunk_count);

                    // Store document reference
                    stats.document_ids.push(doc_id);
                }
                Err(e) => {
                    warn!("Failed to ingest {}: {}", file_path.display(), e);
                    stats.errors.push(format!("{}: {}", file_path.display(), e));
                }
            }
        }

        Ok(stats)
    }

    /// Ingest a single file
    async fn ingest_file(
        &self,
        collection_id: &str,
        path: &Path,
        options: &IngestOptions,
        embeddings: Option<&LocalEmbeddings>,
    ) -> Result<(String, u32), RagError> {
        let doc_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        // Read file content
        let content = if path.extension().map(|e| e == "pdf").unwrap_or(false) {
            // Use PDF extraction for PDF files
            extract_pdf_text(path)?
        } else {
            std::fs::read_to_string(path).map_err(|e| RagError::FileError(e.to_string()))?
        };

        // Get title from filename
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from);

        // Insert document
        sqlx::query(
            r#"
            INSERT INTO rag_documents (id, collection_id, path, title, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&doc_id)
        .bind(collection_id)
        .bind(path.to_string_lossy().as_ref())
        .bind(&title)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        // Chunk the content
        let chunks = chunk_text(&content, options.chunk_size);

        // Generate embeddings if available
        let chunk_embeddings: Option<Vec<Vec<f32>>> = if let Some(emb) = embeddings {
            debug!("Generating embeddings for {} chunks", chunks.len());
            match emb.embed(chunks.clone()) {
                Ok(embs) => Some(embs),
                Err(e) => {
                    warn!("Failed to generate embeddings: {}. Continuing without embeddings.", e);
                    None
                }
            }
        } else {
            None
        };

        // Insert chunks with embeddings
        for (order, chunk_text) in chunks.iter().enumerate() {
            let chunk_id = uuid::Uuid::new_v4().to_string();
            
            // Get embedding for this chunk if available
            let embedding_blob: Option<Vec<u8>> = chunk_embeddings
                .as_ref()
                .and_then(|embs| embs.get(order))
                .map(|emb| embedding_to_blob(emb));

            sqlx::query(
                r#"
                INSERT INTO rag_chunks (id, document_id, collection_id, text, embedding, chunk_order)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&chunk_id)
            .bind(&doc_id)
            .bind(collection_id)
            .bind(chunk_text)
            .bind(&embedding_blob)
            .bind(order as i32)
            .execute(&self.pool)
            .await?;
        }

        Ok((doc_id, chunks.len() as u32))
    }

    /// Query a collection with a question
    /// 
    /// If `embeddings` is provided, semantic similarity search will be used.
    /// Otherwise, falls back to text-based search.
    pub async fn query(
        &self,
        collection_id: &str,
        question: &str,
        options: &QueryOptions,
        gemini: &GeminiClient,
        embeddings: Option<&LocalEmbeddings>,
    ) -> Result<RagQueryResponse, RagError> {
        let collection = self.get_collection(collection_id).await?;

        // Get all chunks from the collection (with embeddings if available)
        let chunks = self.get_chunks_with_embeddings(&collection.id, options.top_k).await?;

        if chunks.is_empty() {
            return Ok(RagQueryResponse {
                answer: "No documents found in this collection.".to_string(),
                sources: vec![],
            });
        }

        // Use embedding-based search if available, otherwise fall back to text search
        let relevant_chunks = if let Some(emb) = embeddings {
            // Check if chunks have embeddings
            let has_embeddings = chunks.iter().any(|(c, _)| !c.embedding.is_empty());
            
            if has_embeddings {
                debug!("Using embedding-based similarity search");
                // Generate query embedding
                match emb.embed_one(question) {
                    Ok(query_emb) => embedding_search(&chunks, &query_emb, options.top_k),
                    Err(e) => {
                        warn!("Failed to generate query embedding: {}. Falling back to text search.", e);
                        simple_text_search(&chunks, question, options.top_k)
                    }
                }
            } else {
                debug!("No embeddings in chunks, using text-based search");
                simple_text_search(&chunks, question, options.top_k)
            }
        } else {
            debug!("No embeddings model, using text-based search");
            simple_text_search(&chunks, question, options.top_k)
        };

        // Build context from relevant chunks
        let context: String = relevant_chunks
            .iter()
            .map(|r| format!("--- Source: {} (score: {:.3}) ---\n{}", r.document_path, r.score, r.chunk.text))
            .collect::<Vec<_>>()
            .join("\n\n");

        // Build RAG prompt
        let prompt = format!(
            r#"You are a helpful assistant. Use the following context to answer the question. 
If the answer is not in the context, say so clearly.

Context:
{}

Question: {}

Answer:"#,
            context, question
        );

        // Call Gemini
        let request = crate::gemini::GeminiRequest::new("gemini-2.5-pro", &prompt);
        let response = gemini
            .call_text(&request)
            .await
            .map_err(|e| RagError::EmbeddingError(e.to_string()))?;

        Ok(RagQueryResponse {
            answer: response.text,
            sources: if options.return_sources {
                relevant_chunks
            } else {
                vec![]
            },
        })
    }

    /// Get chunks from a collection (without embeddings)
    #[allow(dead_code)]
    async fn get_chunks(
        &self,
        collection_id: &str,
        limit: usize,
    ) -> Result<Vec<(Chunk, String)>, RagError> {
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.document_id, c.collection_id, c.text, c.chunk_order, d.path
            FROM rag_chunks c
            JOIN rag_documents d ON c.document_id = d.id
            WHERE c.collection_id = ?
            ORDER BY c.chunk_order
            LIMIT ?
            "#,
        )
        .bind(collection_id)
        .bind(limit as i32 * 10) // Get more chunks for search
        .fetch_all(&self.pool)
        .await?;

        let chunks = rows
            .iter()
            .map(|row| {
                let chunk = Chunk {
                    id: row.get("id"),
                    document_id: row.get("document_id"),
                    collection_id: row.get("collection_id"),
                    text: row.get("text"),
                    embedding: vec![],
                    order: row.get("chunk_order"),
                };
                let path: String = row.get("path");
                (chunk, path)
            })
            .collect();

        Ok(chunks)
    }

    /// Get chunks from a collection with embeddings
    async fn get_chunks_with_embeddings(
        &self,
        collection_id: &str,
        limit: usize,
    ) -> Result<Vec<(Chunk, String)>, RagError> {
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.document_id, c.collection_id, c.text, c.embedding, c.chunk_order, d.path
            FROM rag_chunks c
            JOIN rag_documents d ON c.document_id = d.id
            WHERE c.collection_id = ?
            ORDER BY c.chunk_order
            LIMIT ?
            "#,
        )
        .bind(collection_id)
        .bind(limit as i32 * 10) // Get more chunks for search
        .fetch_all(&self.pool)
        .await?;

        let chunks = rows
            .iter()
            .map(|row| {
                // Parse embedding from blob if present
                let embedding_blob: Option<Vec<u8>> = row.get("embedding");
                let embedding = embedding_blob
                    .map(|blob| blob_to_embedding(&blob))
                    .unwrap_or_default();

                let chunk = Chunk {
                    id: row.get("id"),
                    document_id: row.get("document_id"),
                    collection_id: row.get("collection_id"),
                    text: row.get("text"),
                    embedding,
                    order: row.get("chunk_order"),
                };
                let path: String = row.get("path");
                (chunk, path)
            })
            .collect();

        Ok(chunks)
    }
}

/// Statistics from an ingest operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestStats {
    pub documents_ingested: u32,
    pub chunks_created: u32,
    pub document_ids: Vec<String>,
    pub errors: Vec<String>,
}

// === Helper Functions ===

/// Collect files from a path based on options
fn collect_files(path: &Path, options: &IngestOptions) -> Result<Vec<PathBuf>, RagError> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if entry_path.is_file() {
                // Check extension
                let ext = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                let should_include = match ext {
                    "txt" | "md" | "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp"
                    | "h" | "hpp" | "json" | "yaml" | "yml" | "toml" => true,
                    "pdf" => options.include_pdfs,
                    _ => false,
                };

                // Check pattern if specified
                if let Some(pattern) = &options.pattern {
                    if let Ok(glob) = glob::Pattern::new(pattern) {
                        if !glob.matches_path(entry_path) {
                            continue;
                        }
                    }
                }

                if should_include {
                    files.push(entry_path.to_path_buf());
                }
            }
        }
    }

    Ok(files)
}

/// Extract text from a PDF file
fn extract_pdf_text(path: &Path) -> Result<String, RagError> {
    let doc = crate::pdf::PdfDocument::load(path).map_err(|e| RagError::FileError(e.to_string()))?;
    Ok(doc.full_text())
}

/// Chunk text into smaller pieces
fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for paragraph in text.split("\n\n") {
        if current_chunk.len() + paragraph.len() > chunk_size {
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
            }

            // If paragraph itself is larger than chunk_size, split it
            if paragraph.len() > chunk_size {
                let words: Vec<&str> = paragraph.split_whitespace().collect();
                let mut word_chunk = String::new();

                for word in words {
                    if word_chunk.len() + word.len() + 1 > chunk_size {
                        if !word_chunk.is_empty() {
                            chunks.push(word_chunk.trim().to_string());
                        }
                        word_chunk = word.to_string();
                    } else {
                        if !word_chunk.is_empty() {
                            word_chunk.push(' ');
                        }
                        word_chunk.push_str(word);
                    }
                }
                if !word_chunk.is_empty() {
                    current_chunk = word_chunk;
                }
            } else {
                current_chunk = paragraph.to_string();
            }
        } else {
            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(paragraph);
        }
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    chunks
}

/// Simple text-based search (no embeddings)
fn simple_text_search(
    chunks: &[(Chunk, String)],
    query: &str,
    top_k: usize,
) -> Vec<QueryResult> {
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    let mut scored: Vec<(f32, &Chunk, &String)> = chunks
        .iter()
        .map(|(chunk, path)| {
            let text_lower = chunk.text.to_lowercase();

            // Simple scoring: count matching words
            let score: f32 = query_words
                .iter()
                .filter(|word| text_lower.contains(*word))
                .count() as f32
                / query_words.len().max(1) as f32;

            (score, chunk, path)
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Take top_k and filter out zero scores
    scored
        .into_iter()
        .filter(|(score, _, _)| *score > 0.0)
        .take(top_k)
        .map(|(score, chunk, path)| QueryResult {
            chunk: chunk.clone(),
            score,
            document_path: path.clone(),
        })
        .collect()
}

/// Compute cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Embedding-based similarity search using cosine similarity
fn embedding_search(
    chunks: &[(Chunk, String)],
    query_embedding: &[f32],
    top_k: usize,
) -> Vec<QueryResult> {
    let mut scored: Vec<(f32, &Chunk, &String)> = chunks
        .iter()
        .filter(|(chunk, _)| !chunk.embedding.is_empty())
        .map(|(chunk, path)| {
            let score = cosine_similarity(query_embedding, &chunk.embedding);
            (score, chunk, path)
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Take top_k
    scored
        .into_iter()
        .take(top_k)
        .map(|(score, chunk, path)| QueryResult {
            chunk: chunk.clone(),
            score,
            document_path: path.clone(),
        })
        .collect()
}

/// Convert embedding vector to bytes for storage
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// Convert bytes back to embedding vector
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunk_text(text, 50);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 100); // Allow some flexibility
        }
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_simple_text_search() {
        let chunks = vec![
            (
                Chunk {
                    id: "1".to_string(),
                    document_id: "doc1".to_string(),
                    collection_id: "col1".to_string(),
                    text: "Rust is a systems programming language.".to_string(),
                    embedding: vec![],
                    order: 0,
                },
                "/path/to/rust.txt".to_string(),
            ),
            (
                Chunk {
                    id: "2".to_string(),
                    document_id: "doc2".to_string(),
                    collection_id: "col1".to_string(),
                    text: "Bananas are yellow fruits.".to_string(),
                    embedding: vec![],
                    order: 0,
                },
                "/path/to/fruits.txt".to_string(),
            ),
        ];

        let results = simple_text_search(&chunks, "Rust programming", 5);
        assert!(!results.is_empty());
        assert!(results[0].chunk.text.contains("Rust"));
    }

    #[test]
    fn test_embedding_blob_roundtrip() {
        let embedding = vec![0.1, 0.2, 0.3, -0.4, 0.5];
        let blob = embedding_to_blob(&embedding);
        let recovered = blob_to_embedding(&blob);
        
        assert_eq!(embedding.len(), recovered.len());
        for (a, b) in embedding.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_embedding_search() {
        let chunks = vec![
            (
                Chunk {
                    id: "1".to_string(),
                    document_id: "doc1".to_string(),
                    collection_id: "col1".to_string(),
                    text: "Rust programming".to_string(),
                    embedding: vec![1.0, 0.0, 0.0], // Most similar to query
                    order: 0,
                },
                "/path/to/rust.txt".to_string(),
            ),
            (
                Chunk {
                    id: "2".to_string(),
                    document_id: "doc2".to_string(),
                    collection_id: "col1".to_string(),
                    text: "Bananas".to_string(),
                    embedding: vec![0.0, 1.0, 0.0], // Orthogonal to query
                    order: 0,
                },
                "/path/to/fruits.txt".to_string(),
            ),
        ];

        let query_emb = vec![1.0, 0.0, 0.0];
        let results = embedding_search(&chunks, &query_emb, 5);
        
        assert!(!results.is_empty());
        assert!(results[0].chunk.text.contains("Rust"));
        assert!((results[0].score - 1.0).abs() < 0.001); // Perfect match
    }
}

