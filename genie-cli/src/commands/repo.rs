//! Repository summarization command

use anyhow::{bail, Result};
use genie_core::repo::{repo_summary_to_markdown, RepoOptions};
use genie_core::{Config, GeminiClient, QuotaManager, RequestKind, UsageEvent};
use std::path::PathBuf;
use tracing::info;

pub async fn run(
    config: Config,
    path: PathBuf,
    format: String,
    out: Option<PathBuf>,
    max_files: Option<u32>,
    ignore_quota: bool,
) -> Result<()> {
    if !path.exists() {
        bail!("Path not found: {}", path.display());
    }

    if !path.is_dir() {
        bail!("Path is not a directory: {}", path.display());
    }

    println!("📁 Summarizing repository: {}", path.display());

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::RepoSummary, &config.gemini.default_model)
            .await?;
    }

    // Create client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build options
    let mut options = RepoOptions::default();
    if let Some(max) = max_files {
        options = options.with_max_files(max);
    }

    // Summarize
    info!("Starting repository summarization...");
    println!("🔄 Scanning files and generating summary...");

    let summary = genie_core::repo::summarize_repo(&client, &path, &options).await?;

    // Record usage
    let event = UsageEvent::new(
        &config.gemini.default_model,
        RequestKind::RepoSummary,
        0,
        0,
        true,
    );
    quota_manager.record_event(event).await?;

    // Output
    let _repo_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("repo");

    if format == "json" {
        let json = serde_json::to_string_pretty(&summary)?;
        if let Some(out_path) = out {
            std::fs::write(&out_path, &json)?;
            println!("✅ JSON saved to: {}", out_path.display());
        } else {
            println!("{}", json);
        }
    } else {
        let md = repo_summary_to_markdown(&summary);
        if let Some(out_path) = out {
            std::fs::write(&out_path, &md)?;
            println!("✅ Markdown saved to: {}", out_path.display());
        } else {
            // Print to console
            println!("\n{}", "─".repeat(60));
            println!("{}", md);
        }
    }

    // Print statistics
    println!("\n📊 Statistics:");
    println!("   Files analyzed: {}", summary.file_count);
    println!("   Lines of code:  ~{}", summary.total_lines);
    println!("   Languages:      {}", summary.languages.join(", "));
    println!("   Modules:        {}", summary.modules.len());

    Ok(())
}
