//! Configuration management

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::allocation::AllocationConfig;

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
    pub quic_cert_path: PathBuf,
    pub quic_key_path: PathBuf,
    pub max_allocations: usize,
    pub default_bandwidth_limit: u32,
    pub default_quota: u64,
    pub allocation_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub keepalive_interval_secs: u64,
    pub admin_addr: Option<SocketAddr>,
    pub admin_token: Option<String>,
    pub global_bandwidth_limit: Option<u64>,
    // High Availability
    pub instance_id: Option<String>,
    pub region: Option<String>,
    pub redis_url: Option<String>,
    pub enable_state_sharing: bool,
    pub state_sync_interval_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:4433".parse().unwrap(),
            quic_cert_path: PathBuf::from("cert.pem"),
            quic_key_path: PathBuf::from("key.pem"),
            max_allocations: 1000,
            default_bandwidth_limit: 10 * 1024 * 1024, // 10 Mbps
            default_quota: 1024 * 1024 * 1024,        // 1 GB
            allocation_timeout_secs: 8 * 3600,         // 8 hours
            idle_timeout_secs: 30,
            keepalive_interval_secs: 15,
            admin_addr: None,
            admin_token: None,
            global_bandwidth_limit: None,
            instance_id: None,
            region: None,
            redis_url: None,
            enable_state_sharing: false,
            state_sync_interval_secs: 30,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables, command line, and TOML file
    pub fn load() -> Result<Self, ConfigError> {
        // Start with defaults
        let mut config = Self::default();

        // Load from environment variables
        if let Ok(addr) = std::env::var("ZRC_RELAY_LISTEN_ADDR") {
            config.listen_addr = addr.parse()
                .map_err(|e| ConfigError::Invalid(format!("Invalid listen_addr: {}", e)))?;
        }

        if let Ok(path) = std::env::var("ZRC_RELAY_CERT_PATH") {
            config.quic_cert_path = PathBuf::from(path);
        }

        if let Ok(path) = std::env::var("ZRC_RELAY_KEY_PATH") {
            config.quic_key_path = PathBuf::from(path);
        }

        // Load from command line arguments
        config.load_from_args()?;

        // Load from TOML config file (if specified)
        if let Ok(config_path) = std::env::var("ZRC_RELAY_CONFIG") {
            config.load_from_toml(&config_path)?;
        }

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_allocations == 0 {
            return Err(ConfigError::Invalid("max_allocations must be > 0".to_string()));
        }

        if self.default_bandwidth_limit == 0 {
            return Err(ConfigError::Invalid("default_bandwidth_limit must be > 0".to_string()));
        }

        if self.default_quota == 0 {
            return Err(ConfigError::Invalid("default_quota must be > 0".to_string()));
        }

        if !self.quic_cert_path.exists() {
            return Err(ConfigError::Invalid(format!(
                "Certificate file not found: {:?}",
                self.quic_cert_path
            )));
        }

        if !self.quic_key_path.exists() {
            return Err(ConfigError::Invalid(format!(
                "Key file not found: {:?}",
                self.quic_key_path
            )));
        }

        Ok(())
    }

    /// Load from command line arguments
    fn load_from_args(&mut self) -> Result<(), ConfigError> {
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--listen-addr" | "-l" => {
                    if i + 1 < args.len() {
                        self.listen_addr = args[i + 1].parse()
                            .map_err(|e| ConfigError::Invalid(format!("Invalid listen_addr: {}", e)))?;
                        i += 2;
                    } else {
                        return Err(ConfigError::Invalid("--listen-addr requires a value".to_string()));
                    }
                }
                "--cert" | "-c" => {
                    if i + 1 < args.len() {
                        self.quic_cert_path = PathBuf::from(&args[i + 1]);
                        i += 2;
                    } else {
                        return Err(ConfigError::Invalid("--cert requires a value".to_string()));
                    }
                }
                "--key" | "-k" => {
                    if i + 1 < args.len() {
                        self.quic_key_path = PathBuf::from(&args[i + 1]);
                        i += 2;
                    } else {
                        return Err(ConfigError::Invalid("--key requires a value".to_string()));
                    }
                }
                "--admin-addr" => {
                    if i + 1 < args.len() {
                        self.admin_addr = Some(args[i + 1].parse()
                            .map_err(|e| ConfigError::Invalid(format!("Invalid admin_addr: {}", e)))?);
                        i += 2;
                    } else {
                        return Err(ConfigError::Invalid("--admin-addr requires a value".to_string()));
                    }
                }
                "--admin-token" => {
                    if i + 1 < args.len() {
                        self.admin_token = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err(ConfigError::Invalid("--admin-token requires a value".to_string()));
                    }
                }
                "--config" | "-f" => {
                    // Config file path - will be loaded separately
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }
        Ok(())
    }

    /// Load from TOML config file
    fn load_from_toml(&mut self, path: &str) -> Result<(), ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(e))?;
        
        let toml_config: toml::Value = toml::from_str(&content)
            .map_err(|e| ConfigError::Toml(e))?;

        // Parse TOML values
        if let Some(addr) = toml_config.get("listen_addr").and_then(|v| v.as_str()) {
            self.listen_addr = addr.parse()
                .map_err(|e| ConfigError::Invalid(format!("Invalid listen_addr in TOML: {}", e)))?;
        }

        if let Some(path) = toml_config.get("quic_cert_path").and_then(|v| v.as_str()) {
            self.quic_cert_path = PathBuf::from(path);
        }

        if let Some(path) = toml_config.get("quic_key_path").and_then(|v| v.as_str()) {
            self.quic_key_path = PathBuf::from(path);
        }

        if let Some(addr) = toml_config.get("admin_addr").and_then(|v| v.as_str()) {
            self.admin_addr = Some(addr.parse()
                .map_err(|e| ConfigError::Invalid(format!("Invalid admin_addr in TOML: {}", e)))?);
        }

        if let Some(token) = toml_config.get("admin_token").and_then(|v| v.as_str()) {
            self.admin_token = Some(token.to_string());
        }

        if let Some(max) = toml_config.get("max_allocations").and_then(|v| v.as_integer()) {
            self.max_allocations = max as usize;
        }

        if let Some(bw) = toml_config.get("default_bandwidth_limit").and_then(|v| v.as_integer()) {
            self.default_bandwidth_limit = bw as u32;
        }

        if let Some(quota) = toml_config.get("default_quota").and_then(|v| v.as_integer()) {
            self.default_quota = quota as u64;
        }

        if let Some(timeout) = toml_config.get("allocation_timeout_secs").and_then(|v| v.as_integer()) {
            self.allocation_timeout_secs = timeout as u64;
        }

        if let Some(idle) = toml_config.get("idle_timeout_secs").and_then(|v| v.as_integer()) {
            self.idle_timeout_secs = idle as u64;
        }

        if let Some(keepalive) = toml_config.get("keepalive_interval_secs").and_then(|v| v.as_integer()) {
            self.keepalive_interval_secs = keepalive as u64;
        }

        if let Some(global) = toml_config.get("global_bandwidth_limit").and_then(|v| v.as_integer()) {
            self.global_bandwidth_limit = Some(global as u64);
        }

        Ok(())
    }

    /// Convert to AllocationConfig
    pub fn to_allocation_config(&self) -> AllocationConfig {
        AllocationConfig {
            max_allocations: self.max_allocations,
            default_bandwidth: self.default_bandwidth_limit,
            default_quota: self.default_quota,
            allocation_timeout: Duration::from_secs(self.allocation_timeout_secs),
            idle_timeout: Duration::from_secs(self.idle_timeout_secs),
            keepalive_interval: Duration::from_secs(self.keepalive_interval_secs),
        }
    }
}
