//! Prompt template gallery.
//!
//! This module provides functionality for managing reusable prompt templates
//! with YAML frontmatter and Tera-based variable interpolation.

use crate::config::Config;
use crate::gemini::{GeminiClient, GeminiError, GeminiRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur during template operations
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),

    #[error("Failed to read template file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse template frontmatter: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Template rendering error: {0}")]
    RenderError(#[from] tera::Error),

    #[error("Missing required variable: {0}")]
    MissingVariable(String),

    #[error("Invalid template format: {0}")]
    InvalidFormat(String),

    #[error("File variable error: {0}")]
    FileError(String),

    #[error("Gemini error: {0}")]
    GeminiError(#[from] GeminiError),
}

/// Type of input variable
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputVarType {
    #[default]
    String,
    File,
    Number,
    Boolean,
    #[serde(rename = "enum")]
    Enum(Vec<String>),
}

/// Definition of an input variable for a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputVar {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "type")]
    pub var_type: InputVarType,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// A prompt template with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Template name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Model to use (optional, uses default if not specified)
    #[serde(default)]
    pub model: Option<String>,
    /// Input variables
    #[serde(default)]
    pub input_variables: Vec<InputVar>,
    /// Whether to expect JSON output
    #[serde(default)]
    pub json_output: bool,
    /// The template body (Tera syntax)
    #[serde(skip)]
    pub body: String,
    /// Source file path
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
}

impl PromptTemplate {
    /// Parse a template from a .prompt.md file
    pub fn from_file(path: &Path) -> Result<Self, TemplateError> {
        let content = std::fs::read_to_string(path)?;
        let mut template = Self::parse(&content)?;
        template.source_path = Some(path.to_path_buf());
        Ok(template)
    }

    /// Parse a template from string content
    pub fn parse(content: &str) -> Result<Self, TemplateError> {
        // Split frontmatter and body
        let (frontmatter, body) = split_frontmatter(content)?;

        // Parse YAML frontmatter
        let mut template: PromptTemplate = serde_yaml::from_str(&frontmatter)?;
        template.body = body;

        // Validate template name
        if template.name.is_empty() {
            return Err(TemplateError::InvalidFormat(
                "Template must have a 'name' field".to_string(),
            ));
        }

        Ok(template)
    }

    /// Get all required variables that don't have defaults
    pub fn required_variables(&self) -> Vec<&InputVar> {
        self.input_variables
            .iter()
            .filter(|v| v.required || (v.default.is_none() && v.var_type != InputVarType::Boolean))
            .collect()
    }

    /// Validate that all required variables are provided
    pub fn validate_variables(
        &self,
        provided: &HashMap<String, String>,
    ) -> Result<(), TemplateError> {
        for var in &self.input_variables {
            if var.required && var.default.is_none() && !provided.contains_key(&var.name) {
                return Err(TemplateError::MissingVariable(var.name.clone()));
            }
        }
        Ok(())
    }

    /// Render the template with the given variables
    pub fn render(
        &self,
        variables: &HashMap<String, String>,
        file_contents: &HashMap<String, String>,
    ) -> Result<String, TemplateError> {
        let mut tera = Tera::default();
        tera.add_raw_template("prompt", &self.body)?;

        let mut context = Context::new();

        // Add regular variables
        for var in &self.input_variables {
            let value = variables
                .get(&var.name)
                .or(var.default.as_ref())
                .cloned()
                .unwrap_or_default();

            context.insert(&var.name, &value);

            // For file variables, also add {name}_content
            if var.var_type == InputVarType::File {
                let content_key = format!("{}_content", var.name);
                if let Some(content) = file_contents.get(&var.name) {
                    context.insert(&content_key, content);
                } else {
                    context.insert(&content_key, "");
                }
                // Also insert as file_content for compatibility
                if var.name == "file" {
                    if let Some(content) = file_contents.get(&var.name) {
                        context.insert("file_content", content);
                    }
                }
            }
        }

        // Render template
        let rendered = tera.render("prompt", &context)?;
        Ok(rendered)
    }
}

/// Split content into frontmatter and body
fn split_frontmatter(content: &str) -> Result<(String, String), TemplateError> {
    let content = content.trim();

    // Check for YAML frontmatter delimiter
    if !content.starts_with("---") {
        return Err(TemplateError::InvalidFormat(
            "Template must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the end of frontmatter
    let rest = &content[3..];
    let end_pos = rest.find("\n---").ok_or_else(|| {
        TemplateError::InvalidFormat("Could not find end of frontmatter (---)".to_string())
    })?;

    let frontmatter = rest[..end_pos].trim().to_string();
    let body = rest[end_pos + 4..].trim().to_string();

    Ok((frontmatter, body))
}

/// Load all templates from the prompts directory
pub fn load_templates() -> Result<Vec<PromptTemplate>, TemplateError> {
    let prompts_dir = Config::prompts_dir().ok_or_else(|| {
        TemplateError::InvalidFormat("Could not determine prompts directory".to_string())
    })?;

    load_templates_from_dir(&prompts_dir)
}

/// Load templates from a specific directory
pub fn load_templates_from_dir(dir: &Path) -> Result<Vec<PromptTemplate>, TemplateError> {
    if !dir.exists() {
        debug!("Prompts directory does not exist: {}", dir.display());
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .prompt.md files
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".prompt.md") {
                match PromptTemplate::from_file(&path) {
                    Ok(template) => {
                        debug!("Loaded template: {}", template.name);
                        templates.push(template);
                    }
                    Err(e) => {
                        warn!("Failed to load template from {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    info!("Loaded {} templates", templates.len());

    Ok(templates)
}

/// Find a template by name
pub fn find_template(name: &str) -> Result<PromptTemplate, TemplateError> {
    let templates = load_templates()?;
    templates
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| TemplateError::NotFound(name.to_string()))
}

/// Run a template with the given variables
pub async fn run_template(
    client: &GeminiClient,
    template: &PromptTemplate,
    variables: HashMap<String, String>,
    file_paths: HashMap<String, PathBuf>,
    model_override: Option<&str>,
) -> Result<String, TemplateError> {
    // Validate variables
    template.validate_variables(&variables)?;

    // Read file contents
    let mut file_contents = HashMap::new();
    for var in &template.input_variables {
        if var.var_type == InputVarType::File {
            if let Some(path) = file_paths.get(&var.name) {
                let content = std::fs::read_to_string(path).map_err(|e| {
                    TemplateError::FileError(format!(
                        "Failed to read file '{}': {}",
                        path.display(),
                        e
                    ))
                })?;
                file_contents.insert(var.name.clone(), content);
            }
        }
    }

    // Render template
    let rendered = template.render(&variables, &file_contents)?;
    debug!("Rendered prompt: {} chars", rendered.len());

    // Determine model to use
    let model = model_override
        .map(|s| s.to_string())
        .or_else(|| template.model.clone())
        .unwrap_or_else(|| "gemini-2.5-pro".to_string());

    // Build request
    let mut request = GeminiRequest::new(&model, &rendered);
    if template.json_output {
        request = request.with_json_output();
    }

    // Call Gemini
    let response = if template.json_output {
        client.call_json(&request).await?
    } else {
        client.call_text(&request).await?
    };

    Ok(response.text)
}

/// Create a default example template
pub fn create_example_template() -> String {
    r#"---
name: "example"
description: "An example prompt template"
model: "gemini-2.5-pro"
input_variables:
  - name: "topic"
    description: "The topic to write about"
    default: "Rust programming"
  - name: "style"
    description: "Writing style"
    default: "concise"
json_output: false
---
Write a brief explanation about {{ topic }} in a {{ style }} style.

Make it informative and engaging.
"#
    .to_string()
}

/// Create a book summary template
pub fn create_book_summary_template() -> String {
    r#"---
name: "book-summary"
description: "Summarize a book chapter-by-chapter"
model: "gemini-2.5-pro"
input_variables:
  - name: "style"
    description: "Summary style"
    default: "concise"
  - name: "language"
    description: "Output language"
    default: "en"
  - name: "file"
    type: "file"
    description: "Book file path"
    required: true
json_output: true
---
You are an expert book summarizer.
Style: {{ style }}
Language: {{ language }}

Analyze the following book content and produce a structured JSON summary:

{
  "title": "Book title",
  "summary": "Overall summary",
  "chapters": [
    {
      "title": "Chapter title",
      "summary": "Chapter summary",
      "key_points": ["point 1", "point 2"]
    }
  ]
}

Book content:
{{ file_content }}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter() {
        let content = r#"---
name: test
description: A test template
---
Hello, {{ name }}!"#;

        let (fm, body) = split_frontmatter(content).unwrap();
        assert!(fm.contains("name: test"));
        assert!(body.contains("Hello"));
    }

    #[test]
    fn test_parse_template() {
        let content = r#"---
name: "greeting"
description: "A greeting template"
input_variables:
  - name: "who"
    description: "Who to greet"
    default: "world"
---
Hello, {{ who }}!"#;

        let template = PromptTemplate::parse(content).unwrap();
        assert_eq!(template.name, "greeting");
        assert_eq!(template.input_variables.len(), 1);
        assert_eq!(template.input_variables[0].name, "who");
        assert_eq!(
            template.input_variables[0].default,
            Some("world".to_string())
        );
    }

    #[test]
    fn test_render_template() {
        let content = r#"---
name: "test"
description: "Test"
input_variables:
  - name: "name"
    default: "World"
---
Hello, {{ name }}!"#;

        let template = PromptTemplate::parse(content).unwrap();

        // Test with default
        let result = template.render(&HashMap::new(), &HashMap::new()).unwrap();
        assert_eq!(result.trim(), "Hello, World!");

        // Test with override
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Genie".to_string());
        let result = template.render(&vars, &HashMap::new()).unwrap();
        assert_eq!(result.trim(), "Hello, Genie!");
    }

    #[test]
    fn test_validate_variables() {
        let content = r#"---
name: "test"
description: "Test"
input_variables:
  - name: "required_var"
    required: true
  - name: "optional_var"
    default: "default_value"
---
{{ required_var }} {{ optional_var }}"#;

        let template = PromptTemplate::parse(content).unwrap();

        // Should fail without required variable
        let result = template.validate_variables(&HashMap::new());
        assert!(result.is_err());

        // Should pass with required variable
        let mut vars = HashMap::new();
        vars.insert("required_var".to_string(), "value".to_string());
        let result = template.validate_variables(&vars);
        assert!(result.is_ok());
    }

    #[test]
    fn test_example_template() {
        let content = create_example_template();
        let template = PromptTemplate::parse(&content).unwrap();
        assert_eq!(template.name, "example");
    }
}
