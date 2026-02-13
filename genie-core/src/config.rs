//! Configuration management for Genie.
//!
//! Configuration is loaded in order of precedence:
//! 1. Defaults
//! 2. Config file (~/.genie/config.toml)
//! 3. Environment variables
//! 4. CLI flags (handled at CLI layer)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Gemini CLI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    /// Path to the gemini binary (default: "gemini")
    #[serde(default = "default_binary")]
    pub binary: String,

    /// Default model to use
    #[serde(default = "default_model")]
    pub default_model: String,

    /// Default system prompt (optional)
    #[serde(default)]
    pub system_prompt: Option<String>,
}

fn default_binary() -> String {
    "gemini".to_string()
}

fn default_model() -> String {
    "gemini-3-pro-preview".to_string()
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            binary: default_binary(),
            default_model: default_model(),
            system_prompt: None,
        }
    }
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11435
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

/// Quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// Maximum requests per minute
    #[serde(default = "default_per_minute")]
    pub per_minute: u32,

    /// Maximum requests per day
    #[serde(default = "default_per_day")]
    pub per_day: u32,

    /// Daily reset time (HH:MM format, local time)
    #[serde(default = "default_reset_time")]
    pub reset_time: String,
}

fn default_per_minute() -> u32 {
    60
}

fn default_per_day() -> u32 {
    1000
}

fn default_reset_time() -> String {
    "00:00".to_string()
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            per_minute: default_per_minute(),
            per_day: default_per_day(),
            reset_time: default_reset_time(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

/// Local embeddings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    /// Whether local embeddings are enabled (default: true)
    /// When enabled, embeddings are lazily initialized on first request
    #[serde(default = "default_embeddings_enabled")]
    pub enabled: bool,

    /// Embedding model to use
    /// Supported: "all-MiniLM-L6-v2", "bge-small-en-v1.5", "bge-base-en-v1.5",
    /// "bge-large-en-v1.5", "multilingual-e5-small", "multilingual-e5-base"
    /// Also accepts OpenAI names: "text-embedding-ada-002", "text-embedding-3-small", "text-embedding-3-large"
    #[serde(default = "default_embeddings_model")]
    pub model: String,
}

fn default_embeddings_enabled() -> bool {
    true
}

fn default_embeddings_model() -> String {
    "all-MiniLM-L6-v2".to_string()
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: default_embeddings_enabled(),
            model: default_embeddings_model(),
        }
    }
}

/// Main configuration struct
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gemini: GeminiConfig,

    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub quota: QuotaConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub embeddings: EmbeddingsConfig,
}

impl Config {
    /// Returns the default Genie configuration directory (~/.genie)
    pub fn genie_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".genie"))
    }

    /// Returns the default config file path
    pub fn default_config_path() -> Option<PathBuf> {
        Self::genie_dir().map(|d| d.join("config.toml"))
    }

    /// Returns the default database path
    pub fn default_db_path() -> Option<PathBuf> {
        Self::genie_dir().map(|d| d.join("usage.db"))
    }

    /// Returns the default prompts directory
    pub fn prompts_dir() -> Option<PathBuf> {
        Self::genie_dir().map(|d| d.join("prompts"))
    }

    /// Returns the default RAG database path
    pub fn default_rag_db_path() -> Option<PathBuf> {
        Self::genie_dir().map(|d| d.join("rag.db"))
    }

    /// Load configuration from the default path with environment overrides
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = if let Some(path) = Self::default_config_path() {
            if path.exists() {
                Self::load_from_file(&path)?
            } else {
                Config::default()
            }
        } else {
            Config::default()
        };

        // Apply environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // GENIE_MODEL overrides default model
        if let Ok(model) = std::env::var("GENIE_MODEL") {
            self.gemini.default_model = model;
        }

        // GENIE_PORT overrides server port
        if let Ok(port) = std::env::var("GENIE_PORT") {
            if let Ok(port) = port.parse() {
                self.server.port = port;
            }
        }

        // GENIE_HOST overrides server host
        if let Ok(host) = std::env::var("GENIE_HOST") {
            self.server.host = host;
        }

        // GENIE_LOG_LEVEL overrides log level
        if let Ok(level) = std::env::var("GENIE_LOG_LEVEL") {
            self.logging.level = level;
        }

        // GENIE_GEMINI_BINARY overrides gemini binary path
        if let Ok(binary) = std::env::var("GENIE_GEMINI_BINARY") {
            self.gemini.binary = binary;
        }

        // GENIE_EMBEDDINGS_ENABLED overrides embeddings enabled
        if let Ok(enabled) = std::env::var("GENIE_EMBEDDINGS_ENABLED") {
            self.embeddings.enabled = enabled.to_lowercase() == "true" || enabled == "1";
        }

        // GENIE_EMBEDDINGS_MODEL overrides embeddings model
        if let Ok(model) = std::env::var("GENIE_EMBEDDINGS_MODEL") {
            self.embeddings.model = model;
        }
    }

    /// Save configuration to the default path
    pub fn save(&self) -> Result<(), ConfigError> {
        if let Some(path) = Self::default_config_path() {
            self.save_to_file(&path)
        } else {
            Err(ConfigError::ValidationError(
                "Could not determine config path".to_string(),
            ))
        }
    }

    /// Save configuration to a specific file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Get the server address as a string
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    /// Get the server URL
    pub fn server_url(&self) -> String {
        format!("http://{}:{}", self.server.host, self.server.port)
    }

    /// Ensure the Genie directory and subdirectories exist
    pub fn ensure_dirs() -> std::io::Result<()> {
        if let Some(genie_dir) = Self::genie_dir() {
            std::fs::create_dir_all(&genie_dir)?;
            std::fs::create_dir_all(genie_dir.join("prompts"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.gemini.binary, "gemini");
        assert_eq!(config.gemini.default_model, "gemini-3-pro-preview");
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 11435);
        assert_eq!(config.quota.per_minute, 60);
        assert_eq!(config.quota.per_day, 1000);
        // Embeddings defaults
        assert!(config.embeddings.enabled);
        assert_eq!(config.embeddings.model, "all-MiniLM-L6-v2");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.server.port, parsed.server.port);
    }

    #[test]
    fn test_partial_config() {
        let toml_str = r#"
[server]
port = 9999
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // Custom value
        assert_eq!(config.server.port, 9999);
        // Defaults still applied
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.gemini.default_model, "gemini-3-pro-preview");
    }
}
