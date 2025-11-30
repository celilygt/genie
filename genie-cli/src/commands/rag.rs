//! RAG CLI commands

use anyhow::Result;
use genie_core::rag::{IngestOptions, QueryOptions, RagManager};
use genie_core::{Config, GeminiClient};
use std::path::Path;

/// Initialize a new RAG collection
pub async fn init(name: &str, description: Option<&str>) -> Result<()> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine RAG database path"))?;

    let manager = RagManager::new(&db_path).await?;
    let collection = manager.create_collection(name, description).await?;

    println!("Created collection: {}", collection.name);
    println!("  ID: {}", collection.id);
    if let Some(desc) = &collection.description {
        println!("  Description: {}", desc);
    }

    Ok(())
}

/// List all RAG collections
pub async fn list() -> Result<()> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine RAG database path"))?;

    let manager = RagManager::new(&db_path).await?;
    let collections = manager.list_collections().await?;

    if collections.is_empty() {
        println!("No collections found.");
        println!("Create one with: genie rag init <name>");
        return Ok(());
    }

    println!("{:<36} {:<20} {:>8} {:>8}", "ID", "NAME", "DOCS", "CHUNKS");
    println!("{}", "-".repeat(76));

    for collection in collections {
        println!(
            "{:<36} {:<20} {:>8} {:>8}",
            collection.id, collection.name, collection.document_count, collection.chunk_count
        );
    }

    Ok(())
}

/// Ingest documents into a collection
pub async fn ingest(
    collection_id: &str,
    path: &Path,
    pattern: Option<&str>,
    chunk_size: Option<usize>,
) -> Result<()> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine RAG database path"))?;

    let manager = RagManager::new(&db_path).await?;

    // Verify collection exists
    let collection = manager.get_collection(collection_id).await?;
    println!("Ingesting into collection: {} ({})", collection.name, collection.id);

    let mut options = IngestOptions::default();
    if let Some(p) = pattern {
        options.pattern = Some(p.to_string());
    }
    if let Some(size) = chunk_size {
        options.chunk_size = size;
    }

    // Load config and create Gemini client (for future embedding support)
    let config = Config::load().unwrap_or_default();
    let gemini = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    println!("Processing path: {}", path.display());

    let stats = manager.ingest(collection_id, path, &options, &gemini).await?;

    println!("\nIngest complete:");
    println!("  Documents: {}", stats.documents_ingested);
    println!("  Chunks: {}", stats.chunks_created);

    if !stats.errors.is_empty() {
        println!("\nWarnings:");
        for err in &stats.errors {
            println!("  - {}", err);
        }
    }

    Ok(())
}

/// Query a collection
pub async fn query(
    collection_id: &str,
    question: &str,
    top_k: Option<usize>,
    show_sources: bool,
) -> Result<()> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine RAG database path"))?;

    let manager = RagManager::new(&db_path).await?;

    // Verify collection exists
    let collection = manager.get_collection(collection_id).await?;
    println!("Querying collection: {} ({})\n", collection.name, collection.id);

    let config = Config::load().unwrap_or_default();
    let gemini = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    let mut options = QueryOptions::default();
    if let Some(k) = top_k {
        options.top_k = k;
    }
    options.return_sources = show_sources;

    let response = manager.query(collection_id, question, &options, &gemini).await?;

    println!("Answer:\n{}\n", response.answer);

    if show_sources && !response.sources.is_empty() {
        println!("Sources:");
        for (i, source) in response.sources.iter().enumerate() {
            println!("  {}. {} (score: {:.2})", i + 1, source.document_path, source.score);
            let preview: String = source.chunk.text.chars().take(100).collect();
            println!("     \"{}...\"", preview.replace('\n', " "));
        }
    }

    Ok(())
}

/// Remove a collection
pub async fn remove(collection_id: &str, force: bool) -> Result<()> {
    let db_path = Config::default_rag_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine RAG database path"))?;

    let manager = RagManager::new(&db_path).await?;

    // Get collection info first
    let collection = manager.get_collection(collection_id).await?;

    if !force {
        println!(
            "This will delete collection '{}' with {} documents and {} chunks.",
            collection.name, collection.document_count, collection.chunk_count
        );
        println!("Use --force to confirm deletion.");
        return Ok(());
    }

    manager.delete_collection(collection_id).await?;
    println!("Deleted collection: {} ({})", collection.name, collection.id);

    Ok(())
}

