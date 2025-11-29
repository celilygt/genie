//! Document summarization pipeline.
//!
//! This module provides functionality for summarizing PDFs and books,
//! including chapter detection and map-reduce style summarization.

use crate::gemini::{GeminiClient, GeminiError, GeminiRequest};
use crate::pdf::{
    candidates_to_chapters, detect_chapter_candidates, Chapter, PdfDocument, PdfError,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tracing::{debug, info};

/// Errors that can occur during document summarization
#[derive(Debug, Error)]
pub enum DocsError {
    #[error("PDF error: {0}")]
    PdfError(#[from] PdfError),

    #[error("Gemini error: {0}")]
    GeminiError(#[from] GeminiError),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("No content to summarize")]
    EmptyContent,

    #[error("Summarization failed: {0}")]
    SummarizationFailed(String),
}

/// Summary style options
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStyle {
    #[default]
    Concise,
    Detailed,
    ExamNotes,
    Bullet,
}

impl std::fmt::Display for SummaryStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SummaryStyle::Concise => write!(f, "concise"),
            SummaryStyle::Detailed => write!(f, "detailed"),
            SummaryStyle::ExamNotes => write!(f, "exam-notes"),
            SummaryStyle::Bullet => write!(f, "bullet"),
        }
    }
}

/// Options for document summarization
#[derive(Debug, Clone, Default)]
pub struct SummarizeOptions {
    /// Summary style
    pub style: SummaryStyle,
    /// Output language
    pub language: String,
    /// Maximum chunk size in tokens
    pub max_chunk_tokens: u32,
}

impl SummarizeOptions {
    pub fn new() -> Self {
        Self {
            style: SummaryStyle::Concise,
            language: "en".to_string(),
            max_chunk_tokens: 4000,
        }
    }

    pub fn with_style(mut self, style: SummaryStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = lang.into();
        self
    }

    pub fn with_max_chunk_tokens(mut self, tokens: u32) -> Self {
        self.max_chunk_tokens = tokens;
        self
    }
}

/// Summary of a single document (non-book)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub title: Option<String>,
    pub summary: String,
    pub key_points: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

/// Summary of a chapter in a book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummary {
    pub chapter_id: u32,
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub important_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions_for_reflection: Vec<String>,
}

/// Summary of an entire book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSummary {
    pub title: Option<String>,
    pub author: Option<String>,
    pub chapters: Vec<ChapterSummary>,
    pub global_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reading_roadmap: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

/// Summarize a PDF document (single document, not book-style)
pub async fn summarize_pdf(
    client: &GeminiClient,
    path: &Path,
    options: &SummarizeOptions,
) -> Result<DocumentSummary, DocsError> {
    info!("Summarizing PDF: {}", path.display());

    // Load the PDF
    let doc = PdfDocument::load(path)?;
    let full_text = doc.full_text();

    if full_text.trim().is_empty() {
        return Err(DocsError::EmptyContent);
    }

    debug!(
        "PDF loaded: {} pages, ~{} tokens",
        doc.page_count(),
        doc.estimate_tokens()
    );

    // Check if we need to chunk
    let estimated_tokens = doc.estimate_tokens();
    let summary = if estimated_tokens > options.max_chunk_tokens {
        // Use chunked summarization
        summarize_large_text(client, &full_text, options).await?
    } else {
        // Direct summarization
        summarize_text_direct(client, &full_text, options).await?
    };

    let page_count = doc.page_count();
    Ok(DocumentSummary {
        title: doc
            .metadata
            .title
            .clone()
            .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string())),
        summary: summary.summary,
        key_points: summary.key_points,
        word_count: Some(full_text.split_whitespace().count() as u32),
        page_count: Some(page_count),
    })
}

/// Summarize a book with chapter detection
pub async fn summarize_book(
    client: &GeminiClient,
    path: &Path,
    options: &SummarizeOptions,
) -> Result<BookSummary, DocsError> {
    info!("Summarizing book: {}", path.display());

    // Load the PDF
    let doc = PdfDocument::load(path)?;

    if doc.pages.is_empty() {
        return Err(DocsError::EmptyContent);
    }

    debug!(
        "Book loaded: {} pages, ~{} tokens",
        doc.page_count(),
        doc.estimate_tokens()
    );

    // Detect chapters
    let candidates = detect_chapter_candidates(&doc);
    let chapters = candidates_to_chapters(&candidates, doc.page_count());

    info!("Detected {} chapters", chapters.len());

    // Summarize each chapter
    let mut chapter_summaries = Vec::new();

    for chapter in &chapters {
        debug!(
            "Summarizing chapter {}: {} (pages {}-{})",
            chapter.id, chapter.title, chapter.start_page, chapter.end_page
        );

        let chapter_text = chapter.text(&doc)?;
        let chapter_summary = summarize_chapter(client, chapter, &chapter_text, options).await?;
        chapter_summaries.push(chapter_summary);
    }

    // Generate global summary from chapter summaries
    let global_summary = generate_global_summary(client, &chapter_summaries, options).await?;

    let page_count = doc.page_count();
    Ok(BookSummary {
        title: doc
            .metadata
            .title
            .clone()
            .or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string())),
        author: doc.metadata.author.clone(),
        chapters: chapter_summaries,
        global_summary: global_summary.summary,
        reading_roadmap: global_summary.roadmap,
        page_count: Some(page_count),
    })
}

/// Summarize text directly (for small documents)
async fn summarize_text_direct(
    client: &GeminiClient,
    text: &str,
    options: &SummarizeOptions,
) -> Result<SummaryResult, DocsError> {
    let prompt = format!(
        r#"Summarize the following text in {} style.
Language: {}

Respond with valid JSON only, in this exact format:
{{
  "summary": "A comprehensive summary of the document",
  "key_points": ["Key point 1", "Key point 2", "Key point 3"]
}}

Text to summarize:
{}
"#,
        options.style, options.language, text
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &prompt).with_json_output();
    let response = client.call_json(&request).await?;

    parse_summary_response(&response.text)
}

/// Summarize large text using chunking
async fn summarize_large_text(
    client: &GeminiClient,
    text: &str,
    options: &SummarizeOptions,
) -> Result<SummaryResult, DocsError> {
    // Split into chunks
    let chunks = split_into_chunks(text, options.max_chunk_tokens);
    debug!("Split text into {} chunks", chunks.len());

    // Summarize each chunk
    let mut chunk_summaries = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        debug!("Summarizing chunk {}/{}", idx + 1, chunks.len());
        let summary = summarize_text_direct(client, chunk, options).await?;
        chunk_summaries.push(summary.summary);
    }

    // Combine chunk summaries
    let combined = chunk_summaries.join("\n\n");
    let final_prompt = format!(
        r#"Combine these section summaries into a single coherent summary in {} style.
Language: {}

Respond with valid JSON only, in this exact format:
{{
  "summary": "A comprehensive summary combining all sections",
  "key_points": ["Key point 1", "Key point 2", "Key point 3"]
}}

Section summaries:
{}
"#,
        options.style, options.language, combined
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &final_prompt).with_json_output();
    let response = client.call_json(&request).await?;

    parse_summary_response(&response.text)
}

/// Summarize a single chapter
async fn summarize_chapter(
    client: &GeminiClient,
    chapter: &Chapter,
    text: &str,
    options: &SummarizeOptions,
) -> Result<ChapterSummary, DocsError> {
    let prompt = format!(
        r#"Summarize this book chapter in {} style.
Language: {}
Chapter: {} - {}

Respond with valid JSON only, in this exact format:
{{
  "summary": "A comprehensive summary of the chapter",
  "key_points": ["Key point 1", "Key point 2"],
  "important_terms": ["Term 1", "Term 2"],
  "questions_for_reflection": ["Question 1", "Question 2"]
}}

Chapter text:
{}
"#,
        options.style, options.language, chapter.id, chapter.title, text
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &prompt).with_json_output();
    let response = client.call_json(&request).await?;

    let parsed = parse_chapter_response(&response.text)?;

    Ok(ChapterSummary {
        chapter_id: chapter.id,
        title: chapter.title.clone(),
        summary: parsed.summary,
        key_points: parsed.key_points,
        important_terms: parsed.important_terms.unwrap_or_default(),
        questions_for_reflection: parsed.questions.unwrap_or_default(),
    })
}

/// Generate global summary from chapter summaries
async fn generate_global_summary(
    client: &GeminiClient,
    chapters: &[ChapterSummary],
    options: &SummarizeOptions,
) -> Result<GlobalSummaryResult, DocsError> {
    let chapter_summaries: Vec<String> = chapters
        .iter()
        .map(|c| format!("## {}\n{}", c.title, c.summary))
        .collect();

    let prompt = format!(
        r#"Based on these chapter summaries, create an overall book summary and reading roadmap.
Language: {}

Respond with valid JSON only, in this exact format:
{{
  "summary": "A comprehensive summary of the entire book",
  "roadmap": ["Start with chapter X because...", "Then read chapter Y...", "Finally..."]
}}

Chapter summaries:
{}
"#,
        options.language,
        chapter_summaries.join("\n\n")
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &prompt).with_json_output();
    let response = client.call_json(&request).await?;

    parse_global_summary_response(&response.text)
}

/// Split text into chunks of approximately max_tokens each
fn split_into_chunks(text: &str, max_tokens: u32) -> Vec<String> {
    let max_chars = (max_tokens * 4) as usize; // Approximate
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for paragraph in text.split("\n\n") {
        if current_chunk.len() + paragraph.len() > max_chars && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = String::new();
        }
        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(paragraph);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

// Internal result types for parsing
struct SummaryResult {
    summary: String,
    key_points: Vec<String>,
}

struct ChapterResult {
    summary: String,
    key_points: Vec<String>,
    important_terms: Option<Vec<String>>,
    questions: Option<Vec<String>>,
}

struct GlobalSummaryResult {
    summary: String,
    roadmap: Vec<String>,
}

fn parse_summary_response(text: &str) -> Result<SummaryResult, DocsError> {
    // Try to extract JSON from the response
    let json_text = extract_json(text);

    let value: serde_json::Value = serde_json::from_str(&json_text)?;

    Ok(SummaryResult {
        summary: value["summary"]
            .as_str()
            .unwrap_or("Summary not available")
            .to_string(),
        key_points: value["key_points"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_chapter_response(text: &str) -> Result<ChapterResult, DocsError> {
    let json_text = extract_json(text);
    let value: serde_json::Value = serde_json::from_str(&json_text)?;

    Ok(ChapterResult {
        summary: value["summary"]
            .as_str()
            .unwrap_or("Summary not available")
            .to_string(),
        key_points: value["key_points"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        important_terms: value["important_terms"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        }),
        questions: value["questions_for_reflection"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        }),
    })
}

fn parse_global_summary_response(text: &str) -> Result<GlobalSummaryResult, DocsError> {
    let json_text = extract_json(text);
    let value: serde_json::Value = serde_json::from_str(&json_text)?;

    Ok(GlobalSummaryResult {
        summary: value["summary"]
            .as_str()
            .unwrap_or("Summary not available")
            .to_string(),
        roadmap: value["roadmap"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Extract JSON from text that might contain markdown fences or other content
fn extract_json(text: &str) -> String {
    let trimmed = text.trim();

    // Try direct parse first
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed.to_string();
    }

    // Try to extract from markdown code fence
    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim().to_string();
        }
    }

    // Try to extract from plain code fence
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let content = after_fence[..end].trim();
            // Skip language identifier if present
            if let Some(newline) = content.find('\n') {
                return content[newline + 1..].trim().to_string();
            }
            return content.to_string();
        }
    }

    // Try to find JSON object or array
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

/// Convert document summary to markdown
pub fn document_summary_to_markdown(summary: &DocumentSummary) -> String {
    let mut md = String::new();

    if let Some(title) = &summary.title {
        md.push_str(&format!("# {}\n\n", title));
    }

    md.push_str("## Summary\n\n");
    md.push_str(&summary.summary);
    md.push_str("\n\n");

    if !summary.key_points.is_empty() {
        md.push_str("## Key Points\n\n");
        for point in &summary.key_points {
            md.push_str(&format!("- {}\n", point));
        }
        md.push('\n');
    }

    if let Some(pages) = summary.page_count {
        md.push_str(&format!("---\n*{} pages*\n", pages));
    }

    md
}

/// Convert book summary to markdown
pub fn book_summary_to_markdown(summary: &BookSummary) -> String {
    let mut md = String::new();

    if let Some(title) = &summary.title {
        md.push_str(&format!("# {}\n\n", title));
    }

    if let Some(author) = &summary.author {
        md.push_str(&format!("*By {}*\n\n", author));
    }

    md.push_str("## Overview\n\n");
    md.push_str(&summary.global_summary);
    md.push_str("\n\n");

    if !summary.reading_roadmap.is_empty() {
        md.push_str("## Reading Roadmap\n\n");
        for (i, step) in summary.reading_roadmap.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, step));
        }
        md.push('\n');
    }

    md.push_str("---\n\n## Chapters\n\n");

    for chapter in &summary.chapters {
        md.push_str(&format!(
            "### {}. {}\n\n",
            chapter.chapter_id, chapter.title
        ));
        md.push_str(&chapter.summary);
        md.push_str("\n\n");

        if !chapter.key_points.is_empty() {
            md.push_str("**Key Points:**\n");
            for point in &chapter.key_points {
                md.push_str(&format!("- {}\n", point));
            }
            md.push('\n');
        }

        if !chapter.important_terms.is_empty() {
            md.push_str(&format!(
                "**Terms:** {}\n\n",
                chapter.important_terms.join(", ")
            ));
        }
    }

    if let Some(pages) = summary.page_count {
        md.push_str(&format!("---\n*{} pages*\n", pages));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_chunks() {
        let text = "Paragraph one.\n\nParagraph two.\n\nParagraph three.";
        let chunks = split_into_chunks(text, 10); // Very small for testing

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_extract_json() {
        // Direct JSON
        let json = extract_json(r#"{"key": "value"}"#);
        assert_eq!(json, r#"{"key": "value"}"#);

        // With markdown fence
        let json = extract_json("```json\n{\"key\": \"value\"}\n```");
        assert_eq!(json, r#"{"key": "value"}"#);

        // With text before
        let json = extract_json("Here is the result: {\"key\": \"value\"}");
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_document_summary_to_markdown() {
        let summary = DocumentSummary {
            title: Some("Test Document".to_string()),
            summary: "This is a test summary.".to_string(),
            key_points: vec!["Point 1".to_string(), "Point 2".to_string()],
            word_count: Some(100),
            page_count: Some(5),
        };

        let md = document_summary_to_markdown(&summary);
        assert!(md.contains("# Test Document"));
        assert!(md.contains("This is a test summary"));
        assert!(md.contains("- Point 1"));
    }
}
