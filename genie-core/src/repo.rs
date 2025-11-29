//! Repository summarization module.
//!
//! This module provides functionality to scan and summarize code repositories,
//! respecting .gitignore and grouping files by language and directory.

use crate::gemini::{GeminiClient, GeminiError, GeminiRequest};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};

/// Errors that can occur during repository summarization
#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Path does not exist: {0}")]
    PathNotFound(String),

    #[error("Not a directory: {0}")]
    NotADirectory(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Gemini error: {0}")]
    GeminiError(#[from] GeminiError),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("No files found to summarize")]
    NoFilesFound,
}

/// A file snippet from the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnippet {
    /// Relative path from repo root
    pub path: PathBuf,
    /// Detected language
    pub language: String,
    /// File content (may be truncated)
    pub content: String,
    /// Whether content was truncated
    pub truncated: bool,
    /// Original file size in bytes
    pub size: u64,
}

/// A chunk of files for summarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoChunk {
    /// Chunk ID
    pub id: u32,
    /// Description of what's in this chunk
    pub description: String,
    /// Files in this chunk
    pub files: Vec<FileSnippet>,
}

/// Summary of a module/directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    /// Path relative to repo root
    pub path: String,
    /// Description of the module's purpose
    pub description: String,
    /// Key files in this module
    pub key_files: Vec<String>,
    /// Technologies/frameworks detected
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub technologies: Vec<String>,
}

/// Complete repository summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    /// Repository name (directory name)
    pub name: String,
    /// High-level overview
    pub overview: String,
    /// Module summaries
    pub modules: Vec<ModuleSummary>,
    /// Detected programming languages
    pub languages: Vec<String>,
    /// Total files analyzed
    pub file_count: u32,
    /// Total lines of code (approximate)
    pub total_lines: u32,
}

/// Options for repository scanning
#[derive(Debug, Clone)]
pub struct RepoOptions {
    /// Maximum file size to include (in bytes)
    pub max_file_size: u64,
    /// Maximum files to process
    pub max_files: Option<u32>,
    /// Maximum tokens per chunk
    pub max_chunk_tokens: u32,
    /// Maximum content per file (in chars)
    pub max_content_per_file: usize,
    /// File extensions to include (empty = all known code files)
    pub include_extensions: Vec<String>,
}

impl Default for RepoOptions {
    fn default() -> Self {
        Self {
            max_file_size: 100_000, // 100KB
            max_files: None,
            max_chunk_tokens: 8000,
            max_content_per_file: 10_000, // ~2500 tokens
            include_extensions: Vec::new(),
        }
    }
}

impl RepoOptions {
    pub fn with_max_files(mut self, max: u32) -> Self {
        self.max_files = Some(max);
        self
    }

    pub fn with_max_chunk_tokens(mut self, tokens: u32) -> Self {
        self.max_chunk_tokens = tokens;
        self
    }
}

/// Known code file extensions and their languages
fn get_language_for_extension(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "rs" => Some("Rust"),
        "py" => Some("Python"),
        "js" => Some("JavaScript"),
        "ts" => Some("TypeScript"),
        "tsx" | "jsx" => Some("TypeScript/React"),
        "go" => Some("Go"),
        "java" => Some("Java"),
        "kt" | "kts" => Some("Kotlin"),
        "swift" => Some("Swift"),
        "c" | "h" => Some("C"),
        "cpp" | "cxx" | "cc" | "hpp" => Some("C++"),
        "cs" => Some("C#"),
        "rb" => Some("Ruby"),
        "php" => Some("PHP"),
        "scala" => Some("Scala"),
        "ex" | "exs" => Some("Elixir"),
        "erl" | "hrl" => Some("Erlang"),
        "hs" => Some("Haskell"),
        "ml" | "mli" => Some("OCaml"),
        "clj" | "cljs" => Some("Clojure"),
        "lua" => Some("Lua"),
        "r" => Some("R"),
        "jl" => Some("Julia"),
        "sh" | "bash" | "zsh" => Some("Shell"),
        "sql" => Some("SQL"),
        "html" | "htm" => Some("HTML"),
        "css" | "scss" | "sass" | "less" => Some("CSS"),
        "json" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "toml" => Some("TOML"),
        "xml" => Some("XML"),
        "md" | "markdown" => Some("Markdown"),
        "dockerfile" => Some("Dockerfile"),
        "makefile" => Some("Makefile"),
        _ => None,
    }
}

/// Check if a file should be included based on extension
fn should_include_file(path: &Path, options: &RepoOptions) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Check filename for known config files
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let is_known_file = matches!(
        filename.to_lowercase().as_str(),
        "dockerfile" | "makefile" | "cargo.toml" | "package.json" | "readme.md"
    );

    if !options.include_extensions.is_empty() {
        options
            .include_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    } else {
        is_known_file || get_language_for_extension(ext).is_some()
    }
}

/// Scan a repository and collect file snippets
pub fn scan_repository(path: &Path, options: &RepoOptions) -> Result<Vec<FileSnippet>, RepoError> {
    if !path.exists() {
        return Err(RepoError::PathNotFound(path.display().to_string()));
    }

    if !path.is_dir() {
        return Err(RepoError::NotADirectory(path.display().to_string()));
    }

    info!("Scanning repository: {}", path.display());

    let mut files = Vec::new();
    let mut file_count = 0;

    // Use the ignore crate to respect .gitignore
    let walker = WalkBuilder::new(path)
        .hidden(true) // Skip hidden files
        .git_ignore(true) // Respect .gitignore
        .git_global(true) // Respect global gitignore
        .git_exclude(true) // Respect .git/info/exclude
        .build();

    for entry in walker.flatten() {
        if let Some(max) = options.max_files {
            if file_count >= max {
                debug!("Reached max files limit: {}", max);
                break;
            }
        }

        let entry_path = entry.path();

        // Skip directories
        if entry_path.is_dir() {
            continue;
        }

        // Skip files that are too large
        if let Ok(metadata) = entry_path.metadata() {
            if metadata.len() > options.max_file_size {
                debug!("Skipping large file: {}", entry_path.display());
                continue;
            }
        }

        // Check if we should include this file
        if !should_include_file(entry_path, options) {
            continue;
        }

        // Read file content
        match std::fs::read_to_string(entry_path) {
            Ok(content) => {
                let relative_path = entry_path
                    .strip_prefix(path)
                    .unwrap_or(entry_path)
                    .to_path_buf();

                let ext = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                let language = get_language_for_extension(ext)
                    .unwrap_or("Unknown")
                    .to_string();

                let size = content.len() as u64;
                let truncated = content.len() > options.max_content_per_file;
                let content = if truncated {
                    format!(
                        "{}...\n[Truncated - showing first {} chars of {} total]",
                        &content[..options.max_content_per_file],
                        options.max_content_per_file,
                        size
                    )
                } else {
                    content
                };

                files.push(FileSnippet {
                    path: relative_path,
                    language,
                    content,
                    truncated,
                    size,
                });

                file_count += 1;
            }
            Err(e) => {
                debug!("Failed to read file {}: {}", entry_path.display(), e);
            }
        }
    }

    if files.is_empty() {
        return Err(RepoError::NoFilesFound);
    }

    info!("Found {} files to analyze", files.len());
    Ok(files)
}

/// Group files into chunks for summarization
pub fn chunk_files(files: Vec<FileSnippet>, max_tokens: u32) -> Vec<RepoChunk> {
    let max_chars = (max_tokens * 4) as usize;
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_size = 0;
    let mut chunk_id = 0;

    // Group by directory first
    let mut by_dir: HashMap<String, Vec<FileSnippet>> = HashMap::new();
    for file in files {
        let dir = file
            .path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        by_dir.entry(dir).or_default().push(file);
    }

    for (dir, dir_files) in by_dir {
        for file in dir_files {
            let file_size = file.content.len() + file.path.display().to_string().len() + 50;

            if current_size + file_size > max_chars && !current_chunk.is_empty() {
                chunk_id += 1;
                chunks.push(RepoChunk {
                    id: chunk_id,
                    description: format!("Files from {}", dir),
                    files: std::mem::take(&mut current_chunk),
                });
                current_size = 0;
            }

            current_chunk.push(file);
            current_size += file_size;
        }
    }

    if !current_chunk.is_empty() {
        chunk_id += 1;
        chunks.push(RepoChunk {
            id: chunk_id,
            description: "Remaining files".to_string(),
            files: current_chunk,
        });
    }

    chunks
}

/// Summarize a repository
pub async fn summarize_repo(
    client: &GeminiClient,
    path: &Path,
    options: &RepoOptions,
) -> Result<RepoSummary, RepoError> {
    info!("Summarizing repository: {}", path.display());

    // Scan files
    let files = scan_repository(path, options)?;

    // Collect statistics
    let file_count = files.len() as u32;
    let total_lines: u32 = files.iter().map(|f| f.content.lines().count() as u32).sum();

    // Collect unique languages
    let mut languages: Vec<String> = files
        .iter()
        .map(|f| f.language.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    languages.sort();

    // Chunk files for processing
    let chunks = chunk_files(files, options.max_chunk_tokens);
    debug!("Split into {} chunks", chunks.len());

    // Summarize each chunk
    let mut chunk_summaries = Vec::new();
    for chunk in &chunks {
        debug!(
            "Summarizing chunk {}: {} files",
            chunk.id,
            chunk.files.len()
        );
        let summary = summarize_chunk(client, chunk).await?;
        chunk_summaries.push(summary);
    }

    // Generate overall summary
    let repo_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();

    let (overview, modules) = generate_repo_overview(client, &repo_name, &chunk_summaries).await?;

    Ok(RepoSummary {
        name: repo_name,
        overview,
        modules,
        languages,
        file_count,
        total_lines,
    })
}

/// Summarize a single chunk of files
async fn summarize_chunk(
    client: &GeminiClient,
    chunk: &RepoChunk,
) -> Result<ChunkSummary, RepoError> {
    let files_text: Vec<String> = chunk
        .files
        .iter()
        .map(|f| {
            format!(
                "=== {} ({}) ===\n{}",
                f.path.display(),
                f.language,
                f.content
            )
        })
        .collect();

    let prompt = format!(
        r#"Analyze these source code files and provide a summary.

Respond with valid JSON only, in this exact format:
{{
  "summary": "Brief summary of what these files do",
  "modules": [
    {{
      "path": "path/to/module",
      "description": "What this module does",
      "key_files": ["file1.rs", "file2.rs"],
      "technologies": ["framework1", "library2"]
    }}
  ]
}}

Files to analyze:
{}
"#,
        files_text.join("\n\n")
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &prompt).with_json_output();
    let response = client.call_json(&request).await?;

    parse_chunk_summary(&response.text)
}

/// Generate overall repository overview from chunk summaries
async fn generate_repo_overview(
    client: &GeminiClient,
    repo_name: &str,
    chunk_summaries: &[ChunkSummary],
) -> Result<(String, Vec<ModuleSummary>), RepoError> {
    let summaries_text: Vec<String> = chunk_summaries
        .iter()
        .map(|s| {
            let modules: Vec<String> = s
                .modules
                .iter()
                .map(|m| format!("- {}: {}", m.path, m.description))
                .collect();
            format!("{}\nModules:\n{}", s.summary, modules.join("\n"))
        })
        .collect();

    let prompt = format!(
        r#"Based on these partial summaries of the "{}" repository, create an overall summary.

Respond with valid JSON only, in this exact format:
{{
  "overview": "High-level description of what this repository does, its architecture, and main components",
  "modules": [
    {{
      "path": "src/main",
      "description": "Main entry point and application logic",
      "key_files": ["main.rs", "app.rs"],
      "technologies": ["tokio", "axum"]
    }}
  ]
}}

Partial summaries:
{}
"#,
        repo_name,
        summaries_text.join("\n---\n")
    );

    let request = GeminiRequest::new("gemini-2.5-pro", &prompt).with_json_output();
    let response = client.call_json(&request).await?;

    parse_overview_response(&response.text)
}

// Internal types for parsing
struct ChunkSummary {
    summary: String,
    modules: Vec<ModuleSummary>,
}

fn parse_chunk_summary(text: &str) -> Result<ChunkSummary, RepoError> {
    let json_text = extract_json(text);
    let value: serde_json::Value = serde_json::from_str(&json_text)?;

    let modules = value["modules"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(ModuleSummary {
                        path: m["path"].as_str()?.to_string(),
                        description: m["description"].as_str().unwrap_or("").to_string(),
                        key_files: m["key_files"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        technologies: m["technologies"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ChunkSummary {
        summary: value["summary"]
            .as_str()
            .unwrap_or("Summary not available")
            .to_string(),
        modules,
    })
}

fn parse_overview_response(text: &str) -> Result<(String, Vec<ModuleSummary>), RepoError> {
    let json_text = extract_json(text);
    let value: serde_json::Value = serde_json::from_str(&json_text)?;

    let overview = value["overview"]
        .as_str()
        .unwrap_or("Overview not available")
        .to_string();

    let modules = value["modules"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(ModuleSummary {
                        path: m["path"].as_str()?.to_string(),
                        description: m["description"].as_str().unwrap_or("").to_string(),
                        key_files: m["key_files"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        technologies: m["technologies"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok((overview, modules))
}

fn extract_json(text: &str) -> String {
    let trimmed = text.trim();

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed.to_string();
    }

    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim().to_string();
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let content = after_fence[..end].trim();
            if let Some(newline) = content.find('\n') {
                return content[newline + 1..].trim().to_string();
            }
            return content.to_string();
        }
    }

    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if start <= end {
                return trimmed[start..=end].to_string();
            }
        }
    }

    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if start <= end {
                return trimmed[start..=end].to_string();
            }
        }
    }

    trimmed.to_string()
}

/// Convert repository summary to markdown
pub fn repo_summary_to_markdown(summary: &RepoSummary) -> String {
    let mut md = String::new();

    md.push_str(&format!("# {} - Repository Summary\n\n", summary.name));

    md.push_str("## Overview\n\n");
    md.push_str(&summary.overview);
    md.push_str("\n\n");

    md.push_str("## Statistics\n\n");
    md.push_str(&format!("- **Files analyzed:** {}\n", summary.file_count));
    md.push_str(&format!("- **Lines of code:** ~{}\n", summary.total_lines));
    md.push_str(&format!(
        "- **Languages:** {}\n\n",
        summary.languages.join(", ")
    ));

    if !summary.modules.is_empty() {
        md.push_str("## Modules\n\n");
        for module in &summary.modules {
            md.push_str(&format!("### {}\n\n", module.path));
            md.push_str(&format!("{}\n\n", module.description));

            if !module.key_files.is_empty() {
                md.push_str(&format!(
                    "**Key files:** {}\n\n",
                    module.key_files.join(", ")
                ));
            }

            if !module.technologies.is_empty() {
                md.push_str(&format!(
                    "**Technologies:** {}\n\n",
                    module.technologies.join(", ")
                ));
            }
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_language_for_extension() {
        assert_eq!(get_language_for_extension("rs"), Some("Rust"));
        assert_eq!(get_language_for_extension("py"), Some("Python"));
        assert_eq!(get_language_for_extension("unknown"), None);
    }

    #[test]
    fn test_should_include_file() {
        let options = RepoOptions::default();

        assert!(should_include_file(Path::new("main.rs"), &options));
        assert!(should_include_file(Path::new("app.py"), &options));
        assert!(!should_include_file(Path::new("image.png"), &options));
    }

    #[test]
    fn test_chunk_files() {
        let files = vec![
            FileSnippet {
                path: PathBuf::from("file1.rs"),
                language: "Rust".to_string(),
                content: "fn main() {}".to_string(),
                truncated: false,
                size: 12,
            },
            FileSnippet {
                path: PathBuf::from("file2.rs"),
                language: "Rust".to_string(),
                content: "fn test() {}".to_string(),
                truncated: false,
                size: 12,
            },
        ];

        let chunks = chunk_files(files, 1000);
        assert!(!chunks.is_empty());
    }
}
