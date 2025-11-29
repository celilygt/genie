//! `genie json` command - Send a prompt and get JSON response

use anyhow::{bail, Result};
use genie_core::{Config, GeminiClient, GeminiRequest, QuotaManager, RequestKind, UsageEvent};
use std::io::{self, BufRead};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Read prompt from stdin if available
fn read_stdin() -> Option<String> {
    if atty::is(atty::Stream::Stdin) {
        None
    } else {
        let stdin = io::stdin();
        let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }
}

pub async fn run(
    config: Config,
    prompt: Option<String>,
    schema_path: Option<PathBuf>,
    ignore_quota: bool,
) -> Result<()> {
    // Get prompt from argument or stdin
    let prompt = match prompt {
        Some(p) => p,
        None => match read_stdin() {
            Some(p) => p,
            None => {
                bail!(
                    "No prompt provided. Usage: genie json \"your prompt\" or echo \"prompt\" | genie json"
                );
            }
        },
    };

    debug!("Prompt: {} chars", prompt.len());

    // Load schema if provided
    let schema: Option<serde_json::Value> = if let Some(path) = schema_path {
        let content = std::fs::read_to_string(&path)?;
        Some(serde_json::from_str(&content)?)
    } else {
        None
    };

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota unless ignored
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::Json, &config.gemini.default_model)
            .await?;
    }

    // Create Gemini client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build request with JSON output flag
    let mut request = GeminiRequest::new(&config.gemini.default_model, &prompt).with_json_output();
    if let Some(system) = &config.gemini.system_prompt {
        request = request.with_system_prompt(system);
    }

    info!("Sending JSON request to Gemini...");

    // First attempt
    let response = client.call_json(&request).await;

    match response {
        Ok(resp) => {
            // Check if we got valid JSON
            if let Some(json) = &resp.json {
                // Validate against schema if provided
                if let Some(_schema) = &schema {
                    // TODO: Implement proper JSON Schema validation
                    // For now, just check it's valid JSON
                    warn!("JSON Schema validation not yet implemented");
                }

                // Record usage
                let event = UsageEvent::new(
                    &config.gemini.default_model,
                    RequestKind::Json,
                    prompt.len(),
                    resp.text.len(),
                    true,
                );
                quota_manager.record_event(event).await?;

                // Print pretty JSON
                println!("{}", serde_json::to_string_pretty(json)?);
                Ok(())
            } else {
                // No valid JSON found, try to re-prompt
                warn!("First response was not valid JSON, retrying...");

                let retry_prompt = format!(
                    "{}\n\nIMPORTANT: Your previous response was not valid JSON. \
                     Please respond with ONLY valid JSON, no markdown code fences or other text.",
                    prompt
                );

                let retry_request = GeminiRequest::new(&config.gemini.default_model, &retry_prompt)
                    .with_json_output();

                match client.call_json(&retry_request).await {
                    Ok(retry_resp) => {
                        if let Some(json) = &retry_resp.json {
                            // Record usage
                            let event = UsageEvent::new(
                                &config.gemini.default_model,
                                RequestKind::Json,
                                retry_prompt.len(),
                                retry_resp.text.len(),
                                true,
                            );
                            quota_manager.record_event(event).await?;

                            println!("{}", serde_json::to_string_pretty(json)?);
                            Ok(())
                        } else {
                            // Still no JSON after retry
                            let event = UsageEvent::new(
                                &config.gemini.default_model,
                                RequestKind::Json,
                                retry_prompt.len(),
                                retry_resp.text.len(),
                                false,
                            )
                            .with_error("invalid_json_response");
                            quota_manager.record_event(event).await?;

                            bail!(
                                "Could not get valid JSON response from Gemini. Raw response:\n{}",
                                retry_resp.text
                            )
                        }
                    }
                    Err(e) => {
                        let event = UsageEvent::new(
                            &config.gemini.default_model,
                            RequestKind::Json,
                            retry_prompt.len(),
                            0,
                            false,
                        )
                        .with_error(e.to_string());
                        quota_manager.record_event(event).await?;

                        bail!("Gemini error on retry: {}", e)
                    }
                }
            }
        }
        Err(e) => {
            // Record failed usage
            let event = UsageEvent::new(
                &config.gemini.default_model,
                RequestKind::Json,
                prompt.len(),
                0,
                false,
            )
            .with_error(e.to_string());
            quota_manager.record_event(event).await?;

            bail!("Gemini error: {}", e)
        }
    }
}
