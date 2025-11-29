//! Template gallery commands

use anyhow::{bail, Result};
use genie_core::templates::{find_template, load_templates, run_template};
use genie_core::{Config, GeminiClient, QuotaManager, RequestKind, UsageEvent};
use std::collections::HashMap;
use std::path::PathBuf;

pub async fn list() -> Result<()> {
    let templates = load_templates()?;

    if templates.is_empty() {
        println!("📭 No templates found.");
        println!("\n   Templates are stored in: ~/.genie/prompts/*.prompt.md");
        println!("   Create a new template with: genie templates new <name>");
        return Ok(());
    }

    println!("📋 Available Templates:\n");
    println!("{:<20} DESCRIPTION", "NAME");
    println!("{}", "─".repeat(60));

    for template in templates {
        println!("{:<20} {}", template.name, template.description);
    }

    println!("\n💡 Use 'genie templates show <name>' to see details");
    println!("💡 Use 'genie templates run <name>' to execute a template");

    Ok(())
}

pub async fn show(name: &str) -> Result<()> {
    let template = find_template(name)?;

    println!("📝 Template: {}\n", template.name);
    println!("Description: {}", template.description);

    if let Some(model) = &template.model {
        println!("Model: {}", model);
    }

    println!("JSON Output: {}", template.json_output);

    if !template.input_variables.is_empty() {
        println!("\n📥 Input Variables:");
        for var in &template.input_variables {
            let type_str = match &var.var_type {
                genie_core::templates::InputVarType::String => "string",
                genie_core::templates::InputVarType::File => "file",
                genie_core::templates::InputVarType::Number => "number",
                genie_core::templates::InputVarType::Boolean => "boolean",
                genie_core::templates::InputVarType::Enum(opts) => {
                    &format!("enum({})", opts.join("|"))
                }
            };

            let required = if var.required { " [required]" } else { "" };
            let default = var
                .default
                .as_ref()
                .map(|d| format!(" (default: {})", d))
                .unwrap_or_default();

            println!("  • {} ({}){}{}", var.name, type_str, required, default);
            if !var.description.is_empty() {
                println!("    {}", var.description);
            }
        }
    }

    println!("\n📄 Template Body:");
    println!("{}", "─".repeat(40));
    println!("{}", template.body);
    println!("{}", "─".repeat(40));

    if let Some(path) = &template.source_path {
        println!("\n📁 Source: {}", path.display());
    }

    Ok(())
}

pub async fn run(
    config: Config,
    name: &str,
    vars: Vec<(String, String)>,
    files: Vec<(String, String)>,
    ignore_quota: bool,
) -> Result<()> {
    let template = find_template(name)?;

    println!("🚀 Running template: {}\n", template.name);

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::Template, &config.gemini.default_model)
            .await?;
    }

    // Create client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build variable maps
    let variables: HashMap<String, String> = vars.into_iter().collect();
    let file_paths: HashMap<String, PathBuf> = files
        .into_iter()
        .map(|(k, v)| (k, PathBuf::from(v)))
        .collect();

    // Show what we're using
    if !variables.is_empty() {
        println!("Variables:");
        for (k, v) in &variables {
            println!("  {} = {}", k, v);
        }
    }
    if !file_paths.is_empty() {
        println!("Files:");
        for (k, v) in &file_paths {
            println!("  {} = {}", k, v.display());
        }
    }

    println!("\n🔄 Executing...\n");

    // Run template
    let result = run_template(&client, &template, variables, file_paths, None).await?;

    // Record usage
    let event = UsageEvent::new(
        &config.gemini.default_model,
        RequestKind::Template,
        0,
        result.len(),
        true,
    );
    quota_manager.record_event(event).await?;

    // Output result
    println!("{}", "─".repeat(60));
    println!("{}", result);
    println!("{}", "─".repeat(60));

    Ok(())
}

pub fn new_template(name: &str) -> Result<()> {
    let prompts_dir = Config::prompts_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine prompts directory"))?;

    // Ensure directory exists
    std::fs::create_dir_all(&prompts_dir)?;

    let filename = format!("{}.prompt.md", name);
    let path = prompts_dir.join(&filename);

    if path.exists() {
        bail!("Template already exists: {}", path.display());
    }

    // Create template content
    let content = format!(
        r#"---
name: "{}"
description: "Description of what this template does"
model: "gemini-2.5-pro"
input_variables:
  - name: "input"
    description: "Main input for the template"
    default: ""
json_output: false
---
You are a helpful assistant.

User request: {{{{ input }}}}

Please provide a helpful response.
"#,
        name
    );

    std::fs::write(&path, &content)?;

    println!("✅ Created new template: {}", path.display());
    println!("\n📝 Edit the file to customize your template.");
    println!("💡 Use 'genie templates show {}' to preview", name);
    println!(
        "💡 Use 'genie templates run {} --var input=\"...\"' to execute",
        name
    );

    Ok(())
}
