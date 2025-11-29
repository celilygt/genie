//! `genie up` command - Start daemon with TUI

use anyhow::Result;
use genie_core::{server, Config, GeminiClient, QuotaManager};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use crate::tui;

pub async fn run(config: Config, daemon: bool) -> Result<()> {
    info!("Starting Genie daemon...");

    // Initialize quota manager
    let db_path = Config::default_db_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;
    let quota_manager = QuotaManager::new(&db_path, config.quota.clone()).await?;

    // Create Gemini client
    let gemini_client = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

    // Check if Gemini CLI is available
    if !gemini_client.check_available().await {
        eprintln!(
            "⚠️  Warning: Gemini CLI not found at '{}'",
            config.gemini.binary
        );
        eprintln!("   Make sure it's installed and in your PATH.");
        eprintln!("   Install: npm install -g @google/gemini-cli");
    }

    // Create app state
    let state = Arc::new(server::AppState::new(
        gemini_client,
        quota_manager,
        config.clone(),
    ));

    if daemon {
        // Run server only (no TUI)
        println!("🚀 Genie server starting on {}", config.server_url());
        println!(
            "   OpenAI-compatible endpoint: {}/v1/chat/completions",
            config.server_url()
        );
        println!("   Quota status: {}/v1/quota", config.server_url());
        println!("   Press Ctrl+C to stop");

        server::start_server(state).await?;
    } else {
        // Run with TUI
        // Create a channel to communicate between server and TUI
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        // Clone state for the server task
        let server_state = Arc::clone(&state);
        let server_config = config.clone();

        // Spawn HTTP server in background
        let server_handle = tokio::spawn(async move {
            let router = server::create_router(server_state);
            let addr = server_config.server_addr();

            info!("Starting HTTP server on {}", addr);

            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, router).await?;

            Ok::<_, std::io::Error>(())
        });

        // Run TUI
        let tui_result = tui::run(state, config, shutdown_rx).await;

        // Cleanup
        drop(shutdown_tx);
        server_handle.abort();

        tui_result?;
    }

    Ok(())
}
