//! Application state management

use anyhow::Result;
use genie_core::{server, Config, GeminiClient, QuotaManager};
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::info;

/// Shared application state
pub struct AppState {
    pub config: Config,
    pub gemini: GeminiClient,
    pub quota: QuotaManager,
    /// Handle to the HTTP server's shared state
    pub server_state: Arc<server::AppState>,
    /// Shutdown signal sender for the HTTP server
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl AppState {
    /// Create a new AppState with initialized components
    pub fn new() -> Result<Self> {
        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Ensure directories exist
        Config::ensure_dirs()?;

        // Create Gemini client
        let gemini = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);

        // Create quota manager (blocking initialization for setup)
        let db_path = Config::default_db_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine database path"))?;

        let quota = tokio::runtime::Runtime::new()?.block_on(async {
            QuotaManager::new(&db_path, config.quota.clone()).await
        })?;

        // Create another set of components for the HTTP server
        let server_gemini = GeminiClient::new(&config.gemini.binary, &config.gemini.default_model);
        let server_quota = tokio::runtime::Runtime::new()?.block_on(async {
            QuotaManager::new(&db_path, config.quota.clone()).await
        })?;

        // Create shared server state
        let server_state = Arc::new(server::AppState::new(
            server_gemini,
            server_quota,
            config.clone(),
        ));

        Ok(Self {
            config,
            gemini,
            quota,
            server_state,
            shutdown_tx: None,
        })
    }

    /// Start the HTTP server in the background
    pub fn start_server(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let server_state = Arc::clone(&self.server_state);
        let addr = self.config.server_addr();

        // Spawn the server in a background task
        tokio::spawn(async move {
            let router = server::create_router(server_state);

            info!("Starting Genie HTTP server on {}", addr);

            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind HTTP server to {}: {}", addr, e);
                    return;
                }
            };

            // Run server until shutdown signal
            let server = axum::serve(listener, router);

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        tracing::error!("HTTP server error: {}", e);
                    }
                }
                _ = shutdown_rx => {
                    info!("HTTP server shutdown requested");
                }
            }

            info!("HTTP server stopped");
        });

        info!(
            "Genie API server started at http://{}",
            self.config.server_addr()
        );

        Ok(())
    }

    /// Stop the HTTP server
    pub fn stop_server(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        self.stop_server();
    }
}

