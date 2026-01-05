//! Configuration management

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::access::AccessMode;
use crate::records::RecordConfig;
use crate::discovery::DiscoveryConfig;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    pub database_path: PathBuf,
    pub web_ui_enabled: bool,
    pub access_mode: String, // "invite_only", "discovery_enabled", "open"
    pub max_record_ttl_seconds: u32,
    pub max_discovery_ttl_seconds: u32,
    pub rate_limit_per_minute: u32,
    pub admin_tokens: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".parse().unwrap(),
            database_path: PathBuf::from("dirnode.db"),
            web_ui_enabled: false,
            access_mode: "invite_only".to_string(),
            max_record_ttl_seconds: 86400,  // 24 hours
            max_discovery_ttl_seconds: 3600, // 1 hour
            rate_limit_per_minute: 60,
            admin_tokens: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables and TOML file
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Load from environment variables
        if let Ok(addr) = std::env::var("ZRC_DIRNODE_LISTEN_ADDR") {
            config.listen_addr = addr.parse()
                .map_err(|e| ConfigError::Invalid(format!("Invalid listen_addr: {}", e)))?;
        }

        if let Ok(path) = std::env::var("ZRC_DIRNODE_DATABASE_PATH") {
            config.database_path = PathBuf::from(path);
        }

        if let Ok(enabled) = std::env::var("ZRC_DIRNODE_WEB_UI_ENABLED") {
            config.web_ui_enabled = enabled.parse().unwrap_or(false);
        }

        if let Ok(mode) = std::env::var("ZRC_DIRNODE_ACCESS_MODE") {
            config.access_mode = mode;
        }

        // Load from TOML config file (if specified)
        if let Ok(config_path) = std::env::var("ZRC_DIRNODE_CONFIG") {
            config.load_from_toml(&config_path)?;
        }

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Load from TOML config file
    fn load_from_toml(&mut self, path: &str) -> Result<(), ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let toml_config: toml::Value = toml::from_str(&content)?;

        if let Some(addr) = toml_config.get("listen_addr").and_then(|v| v.as_str()) {
            self.listen_addr = addr.parse()
                .map_err(|e| ConfigError::Invalid(format!("Invalid listen_addr in TOML: {}", e)))?;
        }

        if let Some(path) = toml_config.get("database_path").and_then(|v| v.as_str()) {
            self.database_path = PathBuf::from(path);
        }

        if let Some(enabled) = toml_config.get("web_ui_enabled").and_then(|v| v.as_bool()) {
            self.web_ui_enabled = enabled;
        }

        if let Some(mode) = toml_config.get("access_mode").and_then(|v| v.as_str()) {
            self.access_mode = mode.to_string();
        }

        if let Some(ttl) = toml_config.get("max_record_ttl_seconds").and_then(|v| v.as_integer()) {
            self.max_record_ttl_seconds = ttl as u32;
        }

        if let Some(ttl) = toml_config.get("max_discovery_ttl_seconds").and_then(|v| v.as_integer()) {
            self.max_discovery_ttl_seconds = ttl as u32;
        }

        if let Some(limit) = toml_config.get("rate_limit_per_minute").and_then(|v| v.as_integer()) {
            self.rate_limit_per_minute = limit as u32;
        }

        if let Some(tokens) = toml_config.get("admin_tokens").and_then(|v| v.as_array()) {
            self.admin_tokens = tokens.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_record_ttl_seconds == 0 {
            return Err(ConfigError::Invalid("max_record_ttl_seconds must be > 0".to_string()));
        }

        if self.max_discovery_ttl_seconds == 0 {
            return Err(ConfigError::Invalid("max_discovery_ttl_seconds must be > 0".to_string()));
        }

        if !matches!(self.access_mode.as_str(), "invite_only" | "discovery_enabled" | "open") {
            return Err(ConfigError::Invalid(
                "access_mode must be one of: invite_only, discovery_enabled, open".to_string()
            ));
        }

        Ok(())
    }

    /// Get access mode enum
    pub fn access_mode(&self) -> AccessMode {
        match self.access_mode.as_str() {
            "discovery_enabled" => AccessMode::DiscoveryEnabled,
            "open" => AccessMode::Open,
            _ => AccessMode::InviteOnly,
        }
    }

    /// Get record config
    pub fn record_config(&self) -> RecordConfig {
        RecordConfig {
            max_record_size: 4 * 1024,
            max_ttl_seconds: self.max_record_ttl_seconds,
            max_records: 100_000,
            cleanup_interval: Duration::from_secs(3600),
        }
    }

    /// Get discovery config
    pub fn discovery_config(&self) -> DiscoveryConfig {
        DiscoveryConfig {
            max_ttl: Duration::from_secs(self.max_discovery_ttl_seconds as u64),
            default_ttl: Duration::from_secs(600), // 10 minutes
            max_tokens_per_subject: 3,
        }
    }
}
