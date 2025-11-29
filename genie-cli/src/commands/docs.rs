//! Document summarization commands

use anyhow::{bail, Result};
use genie_core::docs::{
    book_summary_to_markdown, document_summary_to_markdown, SummarizeOptions, SummaryStyle,
};
use genie_core::{Config, GeminiClient, QuotaManager, RequestKind, UsageEvent};
use std::path::PathBuf;
use tracing::info;

fn parse_style(style: &str) -> SummaryStyle {
    match style.to_lowercase().as_str() {
        "detailed" => SummaryStyle::Detailed,
        "exam-notes" | "exam_notes" => SummaryStyle::ExamNotes,
        "bullet" => SummaryStyle::Bullet,
        _ => SummaryStyle::Concise,
    }
}

pub async fn summarize_pdf(
    config: Config,
    file: PathBuf,
    style: String,
    language: String,
    format: String,
    out: Option<PathBuf>,
    ignore_quota: bool,
) -> Result<()> {
    if !file.exists() {
        bail!("File not found: {}", file.display());
    }

    println!("📄 Summarizing PDF: {}", file.display());

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::SummarizePdf, &config.gemini.default_model)
            .await?;
    }

    // Create client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build options
    let options = SummarizeOptions::new()
        .with_style(parse_style(&style))
        .with_language(&language);

    // Summarize
    info!("Starting PDF summarization...");
    let summary = genie_core::docs::summarize_pdf(&client, &file, &options).await?;

    // Record usage
    let event = UsageEvent::new(
        &config.gemini.default_model,
        RequestKind::SummarizePdf,
        0,
        0,
        true,
    );
    quota_manager.record_event(event).await?;

    // Output
    let base_name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("summary");

    let output_json = format == "json" || format == "both";
    let output_md = format == "markdown" || format == "both";

    if output_json {
        let json = serde_json::to_string_pretty(&summary)?;
        let json_path = out
            .as_ref()
            .map(|p| p.with_extension("json"))
            .unwrap_or_else(|| PathBuf::from(format!("{}_summary.json", base_name)));
        std::fs::write(&json_path, &json)?;
        println!("✅ JSON saved to: {}", json_path.display());
    }

    if output_md {
        let md = document_summary_to_markdown(&summary);
        let md_path = out
            .as_ref()
            .map(|p| p.with_extension("md"))
            .unwrap_or_else(|| PathBuf::from(format!("{}_summary.md", base_name)));
        std::fs::write(&md_path, &md)?;
        println!("✅ Markdown saved to: {}", md_path.display());
    }

    // Print summary
    println!("\n{}", "─".repeat(60));
    if let Some(title) = &summary.title {
        println!("📖 {}", title);
    }
    println!("\n{}\n", summary.summary);

    if !summary.key_points.is_empty() {
        println!("Key Points:");
        for point in &summary.key_points {
            println!("  • {}", point);
        }
    }

    Ok(())
}

pub async fn summarize_book(
    config: Config,
    file: PathBuf,
    style: String,
    language: String,
    format: String,
    out: Option<PathBuf>,
    ignore_quota: bool,
) -> Result<()> {
    if !file.exists() {
        bail!("File not found: {}", file.display());
    }

    println!("📚 Summarizing book: {}", file.display());

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::SummarizeBook, &config.gemini.default_model)
            .await?;
    }

    // Create client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build options
    let options = SummarizeOptions::new()
        .with_style(parse_style(&style))
        .with_language(&language);

    // Summarize
    info!("Starting book summarization (this may take a while)...");
    println!("🔄 Detecting chapters and summarizing...");

    let summary = genie_core::docs::summarize_book(&client, &file, &options).await?;

    // Record usage
    let event = UsageEvent::new(
        &config.gemini.default_model,
        RequestKind::SummarizeBook,
        0,
        0,
        true,
    );
    quota_manager.record_event(event).await?;

    // Output
    let base_name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("summary");

    let output_json = format == "json" || format == "both";
    let output_md = format == "markdown" || format == "both";

    if output_json {
        let json = serde_json::to_string_pretty(&summary)?;
        let json_path = out
            .as_ref()
            .map(|p| p.with_extension("json"))
            .unwrap_or_else(|| PathBuf::from(format!("{}_book_summary.json", base_name)));
        std::fs::write(&json_path, &json)?;
        println!("✅ JSON saved to: {}", json_path.display());
    }

    if output_md {
        let md = book_summary_to_markdown(&summary);
        let md_path = out
            .as_ref()
            .map(|p| p.with_extension("md"))
            .unwrap_or_else(|| PathBuf::from(format!("{}_book_summary.md", base_name)));
        std::fs::write(&md_path, &md)?;
        println!("✅ Markdown saved to: {}", md_path.display());
    }

    // Print summary
    println!("\n{}", "─".repeat(60));
    if let Some(title) = &summary.title {
        println!("📖 {}", title);
    }
    if let Some(author) = &summary.author {
        println!("✍️  By {}", author);
    }
    println!("\n📝 Overview:\n{}\n", summary.global_summary);
    println!("📚 {} chapters detected\n", summary.chapters.len());

    for chapter in &summary.chapters {
        println!("  {}. {}", chapter.chapter_id, chapter.title);
    }

    Ok(())
}
