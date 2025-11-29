//! Genie CLI - Local Gemini-as-a-service
//!
//! A command-line interface for interacting with the Gemini CLI
//! through Genie's quota tracking and power tools.

mod commands;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use genie_core::Config;
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Genie - Local Gemini-as-a-service
#[derive(Parser)]
#[command(name = "genie")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to config file
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Model to use (overrides config)
    #[arg(short, long, global = true, env = "GENIE_MODEL")]
    model: Option<String>,

    /// Server port (overrides config)
    #[arg(long, global = true, env = "GENIE_PORT")]
    port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, global = true, default_value = "info", env = "GENIE_LOG_LEVEL")]
    log_level: String,

    /// Ignore quota limits
    #[arg(long, global = true)]
    ignore_quota: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Send a one-shot prompt to Gemini
    Ask {
        /// The prompt to send (or read from stdin if not provided)
        prompt: Option<String>,
    },

    /// Send a prompt and get JSON response
    Json {
        /// The prompt to send
        prompt: Option<String>,

        /// Path to JSON schema file for validation
        #[arg(long)]
        schema: Option<PathBuf>,
    },

    /// Start Genie daemon with TUI
    Up {
        /// Run in background without TUI
        #[arg(long)]
        daemon: bool,
    },

    /// Check daemon status
    Status,

    /// Stop the running daemon
    Stop,

    /// Quota management commands
    #[command(subcommand)]
    Quota(QuotaCommands),

    /// Configuration commands
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
pub enum QuotaCommands {
    /// Show current quota status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show recent usage log
    Log {
        /// Number of recent entries to show
        #[arg(long, default_value = "10")]
        last: u32,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Initialize default configuration
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level);

    // Load configuration
    let mut config = if let Some(path) = &cli.config {
        Config::load_from_file(path)?
    } else {
        Config::load().unwrap_or_default()
    };

    // Apply CLI overrides
    if let Some(model) = &cli.model {
        config.gemini.default_model = model.clone();
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }

    // Ensure genie directory exists
    Config::ensure_dirs()?;

    // Run the appropriate command
    match cli.command {
        Commands::Ask { prompt } => commands::ask::run(config, prompt, cli.ignore_quota).await,
        Commands::Json { prompt, schema } => {
            commands::json::run(config, prompt, schema, cli.ignore_quota).await
        }
        Commands::Up { daemon } => commands::up::run(config, daemon).await,
        Commands::Status => commands::status::run(config).await,
        Commands::Stop => commands::stop::run(config).await,
        Commands::Quota(cmd) => match cmd {
            QuotaCommands::Status { json } => commands::quota::status(config, json).await,
            QuotaCommands::Log { last } => commands::quota::log(config, last).await,
        },
        Commands::Config(cmd) => match cmd {
            ConfigCommands::Show => commands::config::show(config),
            ConfigCommands::Init { force } => commands::config::init(force),
        },
    }
}
