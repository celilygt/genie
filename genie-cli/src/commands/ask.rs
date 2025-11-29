//! `genie ask` command - Send a one-shot prompt to Gemini

use anyhow::{bail, Result};
use genie_core::{Config, GeminiClient, GeminiRequest, QuotaManager, RequestKind, UsageEvent};
use std::io::{self, BufRead};
use tracing::{debug, info};

/// Read prompt from stdin if available
fn read_stdin() -> Option<String> {
    if atty::is(atty::Stream::Stdin) {
        // Stdin is a terminal, not piped
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

pub async fn run(config: Config, prompt: Option<String>, ignore_quota: bool) -> Result<()> {
    // Get prompt from argument or stdin
    let prompt = match prompt {
        Some(p) => p,
        None => match read_stdin() {
            Some(p) => p,
            None => {
                bail!("No prompt provided. Usage: genie ask \"your prompt\" or echo \"prompt\" | genie ask");
            }
        },
    };

    debug!("Prompt: {} chars", prompt.len());

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Check quota unless ignored
    if !ignore_quota {
        quota_manager
            .check_before_request(&RequestKind::Ask, &config.gemini.default_model)
            .await?;
    }

    // Create Gemini client
    let client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Build request
    let mut request = GeminiRequest::new(&config.gemini.default_model, &prompt);
    if let Some(system) = &config.gemini.system_prompt {
        request = request.with_system_prompt(system);
    }

    info!("Sending request to Gemini...");

    // Call Gemini
    match client.call_text(&request).await {
        Ok(response) => {
            // Record usage
            let event = UsageEvent::new(
                &config.gemini.default_model,
                RequestKind::Ask,
                prompt.len(),
                response.text.len(),
                true,
            );
            quota_manager.record_event(event).await?;

            // Print response
            println!("{}", response.text);
            Ok(())
        }
        Err(e) => {
            // Record failed usage
            let event = UsageEvent::new(
                &config.gemini.default_model,
                RequestKind::Ask,
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
