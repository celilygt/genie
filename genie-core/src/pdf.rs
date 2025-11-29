//! PDF extraction and text model.
//!
//! This module provides functionality to read PDFs into structured Rust types
//! with page and formatting information for chapter detection and summarization.

use lopdf::Document;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

/// Errors that can occur during PDF processing
#[derive(Debug, Error)]
pub enum PdfError {
    #[error("Failed to open PDF file: {0}")]
    OpenError(String),

    #[error("Failed to read PDF: {0}")]
    ReadError(#[from] lopdf::Error),

    #[error("PDF has no pages")]
    EmptyDocument,

    #[error("Failed to extract text from page {0}: {1}")]
    ExtractionError(u32, String),

    #[error("Invalid page range: {0}")]
    InvalidPageRange(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A text block extracted from a PDF page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// The text content
    pub text: String,
    /// Estimated font size (may be approximate)
    pub font_size: f32,
    /// Whether the text appears to be bold
    pub bold: bool,
    /// Position on page (if available)
    pub position: Option<(f32, f32)>,
}

impl TextBlock {
    pub fn new(text: String) -> Self {
        Self {
            text,
            font_size: 12.0, // Default
            bold: false,
            position: None,
        }
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }
}

/// A single page from a PDF document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPage {
    /// Page index (0-based)
    pub index: u32,
    /// Text blocks on this page
    pub text_blocks: Vec<TextBlock>,
}

impl PdfPage {
    /// Get all text on this page as a single string
    pub fn text(&self) -> String {
        self.text_blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check if this page is likely empty or has minimal content
    pub fn is_empty(&self) -> bool {
        self.text_blocks.is_empty() || self.text_blocks.iter().all(|b| b.text.trim().is_empty())
    }
}

/// A PDF document with extracted text and structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfDocument {
    /// Path to the PDF file
    pub path: PathBuf,
    /// Pages in the document
    pub pages: Vec<PdfPage>,
    /// Document metadata
    pub metadata: PdfMetadata,
}

/// PDF metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub page_count: u32,
}

impl PdfDocument {
    /// Load a PDF document from a file path
    pub fn load(path: &Path) -> Result<Self, PdfError> {
        debug!("Loading PDF from: {}", path.display());

        let doc = Document::load(path).map_err(|e| PdfError::OpenError(e.to_string()))?;

        let page_count = doc.get_pages().len() as u32;
        if page_count == 0 {
            return Err(PdfError::EmptyDocument);
        }

        debug!("PDF has {} pages", page_count);

        // Extract metadata
        let metadata = Self::extract_metadata(&doc, page_count);

        // Extract pages
        let mut pages = Vec::new();
        for (page_num, _page_id) in doc.get_pages() {
            let page = Self::extract_page(&doc, page_num)?;
            pages.push(page);
        }

        // Sort pages by index
        pages.sort_by_key(|p| p.index);

        Ok(PdfDocument {
            path: path.to_path_buf(),
            pages,
            metadata,
        })
    }

    /// Extract metadata from the PDF
    fn extract_metadata(doc: &Document, page_count: u32) -> PdfMetadata {
        let mut metadata = PdfMetadata {
            page_count,
            ..Default::default()
        };

        // Try to extract from document info dictionary
        if let Ok(info) = doc.trailer.get(b"Info") {
            if let Ok(info_ref) = info.as_reference() {
                if let Ok(info_dict) = doc.get_dictionary(info_ref) {
                    if let Ok(title) = info_dict.get(b"Title") {
                        if let Ok(s) = title.as_string() {
                            metadata.title = Some(s.to_string());
                        }
                    }
                    if let Ok(author) = info_dict.get(b"Author") {
                        if let Ok(s) = author.as_string() {
                            metadata.author = Some(s.to_string());
                        }
                    }
                    if let Ok(subject) = info_dict.get(b"Subject") {
                        if let Ok(s) = subject.as_string() {
                            metadata.subject = Some(s.to_string());
                        }
                    }
                    if let Ok(creator) = info_dict.get(b"Creator") {
                        if let Ok(s) = creator.as_string() {
                            metadata.creator = Some(s.to_string());
                        }
                    }
                }
            }
        }

        metadata
    }

    /// Extract a single page from the document
    fn extract_page(doc: &Document, page_num: u32) -> Result<PdfPage, PdfError> {
        let text = doc
            .extract_text(&[page_num])
            .unwrap_or_else(|_| String::new());

        // Split text into blocks (paragraphs)
        let text_blocks = Self::parse_text_blocks(&text);

        Ok(PdfPage {
            index: page_num - 1, // Convert to 0-based
            text_blocks,
        })
    }

    /// Parse extracted text into text blocks
    fn parse_text_blocks(text: &str) -> Vec<TextBlock> {
        let mut blocks = Vec::new();

        // Heuristic: lines starting with "Chapter", "Section" etc might be headings
        let heading_pattern =
            Regex::new(r"^(Chapter|Section|Part|CHAPTER|SECTION|PART)\s+\d+").unwrap();

        // Split by double newlines to get paragraphs
        for paragraph in text.split("\n\n") {
            let trimmed = paragraph.trim();
            if !trimmed.is_empty() {
                let mut block = TextBlock::new(trimmed.to_string());

                // Heuristic: short lines in ALL CAPS might be headings
                if trimmed.len() < 100
                    && trimmed
                        .chars()
                        .filter(|c| c.is_alphabetic())
                        .all(|c| c.is_uppercase())
                {
                    block.font_size = 16.0;
                    block.bold = true;
                }

                if heading_pattern.is_match(trimmed) {
                    block.font_size = 18.0;
                    block.bold = true;
                }

                blocks.push(block);
            }
        }

        // If no paragraphs found, split by single newlines
        if blocks.is_empty() {
            for line in text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    blocks.push(TextBlock::new(trimmed.to_string()));
                }
            }
        }

        blocks
    }

    /// Get all text from the document as a single string
    pub fn full_text(&self) -> String {
        self.pages
            .iter()
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Get text from a range of pages (1-based, inclusive)
    pub fn pages_text_range(&self, start: u32, end: u32) -> Result<String, PdfError> {
        if start == 0 || end == 0 {
            return Err(PdfError::InvalidPageRange(
                "Page numbers must be 1-based".to_string(),
            ));
        }
        if start > end {
            return Err(PdfError::InvalidPageRange(format!(
                "Start page ({}) must be <= end page ({})",
                start, end
            )));
        }

        let text = self
            .pages
            .iter()
            .filter(|p| {
                let page_num = p.index + 1; // Convert back to 1-based
                page_num >= start && page_num <= end
            })
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(text)
    }

    /// Get the number of pages
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Compute a font size histogram
    pub fn font_size_histogram(&self) -> HashMap<u32, usize> {
        let mut histogram = HashMap::new();

        for page in &self.pages {
            for block in &page.text_blocks {
                // Round to nearest integer for histogram
                let size_key = block.font_size.round() as u32;
                *histogram.entry(size_key).or_insert(0) += 1;
            }
        }

        histogram
    }

    /// Estimate the body text font size (most common size)
    pub fn estimate_body_font_size(&self) -> f32 {
        let histogram = self.font_size_histogram();

        histogram
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(size, _)| size as f32)
            .unwrap_or(12.0)
    }

    /// Estimate total token count (chars / 4)
    pub fn estimate_tokens(&self) -> u32 {
        let total_chars: usize = self.pages.iter().map(|p| p.text().len()).sum();
        (total_chars / 4) as u32
    }
}

/// Chapter candidate detected by heuristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterCandidate {
    /// Page index (0-based)
    pub page_index: u32,
    /// Block index on the page
    pub block_index: usize,
    /// Title text
    pub title: String,
    /// Font size
    pub font_size: f32,
    /// Whether bold
    pub bold: bool,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// A confirmed chapter with page range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// Chapter ID (sequential)
    pub id: u32,
    /// Chapter title
    pub title: String,
    /// Start page (1-based, inclusive)
    pub start_page: u32,
    /// End page (1-based, inclusive)
    pub end_page: u32,
}

impl Chapter {
    /// Get text for this chapter from the document
    pub fn text(&self, doc: &PdfDocument) -> Result<String, PdfError> {
        doc.pages_text_range(self.start_page, self.end_page)
    }
}

/// Detect chapter candidates using heuristics
pub fn detect_chapter_candidates(doc: &PdfDocument) -> Vec<ChapterCandidate> {
    let body_size = doc.estimate_body_font_size();
    let mut candidates = Vec::new();

    // Patterns that suggest chapter headings
    let chapter_patterns = [
        Regex::new(r"(?i)^chapter\s+\d+").unwrap(),
        Regex::new(r"(?i)^chapter\s+[ivxlc]+").unwrap(), // Roman numerals
        Regex::new(r"(?i)^section\s+\d+").unwrap(),
        Regex::new(r"(?i)^part\s+\d+").unwrap(),
        Regex::new(r"^\d+\.\s+[A-Z]").unwrap(), // "1. Title"
    ];

    for page in &doc.pages {
        for (block_idx, block) in page.text_blocks.iter().enumerate() {
            let text = block.text.trim();

            // Skip very long text (unlikely to be a chapter heading)
            if text.len() > 150 {
                continue;
            }

            // Skip very short text
            if text.len() < 3 {
                continue;
            }

            let mut confidence: f32 = 0.0;

            // Check font size (larger than body = higher confidence)
            if block.font_size > body_size + 1.5 {
                confidence += 0.3;
            }

            // Check bold
            if block.bold {
                confidence += 0.2;
            }

            // Check against chapter patterns
            for pattern in &chapter_patterns {
                if pattern.is_match(text) {
                    confidence += 0.4;
                    break;
                }
            }

            // Check if text is mostly uppercase (common for headings)
            let alpha_chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
            if !alpha_chars.is_empty() {
                let upper_ratio = alpha_chars.iter().filter(|c| c.is_uppercase()).count() as f32
                    / alpha_chars.len() as f32;
                if upper_ratio > 0.7 {
                    confidence += 0.15;
                }
            }

            // Only include if we have some confidence
            if confidence >= 0.3 {
                candidates.push(ChapterCandidate {
                    page_index: page.index,
                    block_index: block_idx,
                    title: text.to_string(),
                    font_size: block.font_size,
                    bold: block.bold,
                    confidence,
                });
            }
        }
    }

    // Sort by page and confidence
    candidates.sort_by(|a, b| {
        a.page_index
            .cmp(&b.page_index)
            .then(b.confidence.partial_cmp(&a.confidence).unwrap())
    });

    // Remove duplicates on same page (keep highest confidence)
    let mut seen_pages = std::collections::HashSet::new();
    candidates.retain(|c| {
        if seen_pages.contains(&c.page_index) {
            false
        } else {
            seen_pages.insert(c.page_index);
            true
        }
    });

    debug!("Detected {} chapter candidates", candidates.len());
    candidates
}

/// Convert candidates to chapters with page ranges
pub fn candidates_to_chapters(candidates: &[ChapterCandidate], total_pages: u32) -> Vec<Chapter> {
    if candidates.is_empty() {
        // No chapters detected - treat whole document as one chapter
        return vec![Chapter {
            id: 1,
            title: "Document".to_string(),
            start_page: 1,
            end_page: total_pages,
        }];
    }

    let mut chapters = Vec::new();

    for (idx, candidate) in candidates.iter().enumerate() {
        let start_page = candidate.page_index + 1; // Convert to 1-based
        let end_page = if idx + 1 < candidates.len() {
            // End at page before next chapter
            candidates[idx + 1].page_index // This gives us the last page (0-based) which equals prev page 1-based
        } else {
            total_pages
        };

        chapters.push(Chapter {
            id: (idx + 1) as u32,
            title: candidate.title.clone(),
            start_page,
            end_page,
        });
    }

    chapters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_block_creation() {
        let block = TextBlock::new("Hello World".to_string())
            .with_font_size(14.0)
            .with_bold(true);

        assert_eq!(block.text, "Hello World");
        assert_eq!(block.font_size, 14.0);
        assert!(block.bold);
    }

    #[test]
    fn test_pdf_page_text() {
        let page = PdfPage {
            index: 0,
            text_blocks: vec![
                TextBlock::new("First paragraph".to_string()),
                TextBlock::new("Second paragraph".to_string()),
            ],
        };

        let text = page.text();
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_chapter_candidate_patterns() {
        let chapter_pattern = Regex::new(r"(?i)^chapter\s+\d+").unwrap();
        assert!(chapter_pattern.is_match("Chapter 1"));
        assert!(chapter_pattern.is_match("CHAPTER 2"));
        assert!(chapter_pattern.is_match("chapter 10"));
        assert!(!chapter_pattern.is_match("In this chapter"));
    }

    #[test]
    fn test_candidates_to_chapters() {
        let candidates = vec![
            ChapterCandidate {
                page_index: 0,
                block_index: 0,
                title: "Chapter 1".to_string(),
                font_size: 18.0,
                bold: true,
                confidence: 0.8,
            },
            ChapterCandidate {
                page_index: 5,
                block_index: 0,
                title: "Chapter 2".to_string(),
                font_size: 18.0,
                bold: true,
                confidence: 0.8,
            },
        ];

        let chapters = candidates_to_chapters(&candidates, 10);

        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].start_page, 1);
        assert_eq!(chapters[0].end_page, 5);
        assert_eq!(chapters[1].start_page, 6);
        assert_eq!(chapters[1].end_page, 10);
    }
}
