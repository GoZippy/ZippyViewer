use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::PathBuf,
    time::Duration,
};
use crate::auth::AuthMode;
use crate::rate_limit::RateLimitConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    
    // Mailbox settings
    pub max_message_size: usize,
    pub max_queue_length: usize,
    pub message_ttl_secs: u64,
    pub eviction_interval_secs: u64,
    pub idle_mailbox_timeout_secs: u64,
    pub memory_limit_mb: usize,
    
    // Rate limiting
    pub rate_limit: RateLimitConfig,
    
    // Authentication
    pub auth_mode: String, // "disabled", "server_wide", "per_mailbox"
    pub server_tokens: Vec<String>,
    
    // Allowlist/Blocklist
    pub allowlist: Vec<String>,
    pub blocklist: Vec<String>,
    
    // Graceful shutdown
    pub shutdown_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
            tls_cert_path: None,
            tls_key_path: None,
            max_message_size: 64 * 1024, // 64KB
            max_queue_length: 100,
            message_ttl_secs: 300, // 5 minutes
            eviction_interval_secs: 30,
            idle_mailbox_timeout_secs: 3600, // 1 hour
            memory_limit_mb: 50,
            rate_limit: RateLimitConfig::default(),
            auth_mode: "disabled".to_string(),
            server_tokens: Vec::new(),
            allowlist: Vec::new(),
            blocklist: Vec::new(),
            shutdown_timeout_secs: 30,
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        if let Ok(addr) = std::env::var("ZRC_BIND_ADDR") {
            config.bind_addr = addr.parse()?;
        }
        
        if let Ok(size) = std::env::var("ZRC_MAX_MESSAGE_SIZE") {
            config.max_message_size = size.parse()?;
        }
        
        if let Ok(len) = std::env::var("ZRC_MAX_QUEUE_LENGTH") {
            config.max_queue_length = len.parse()?;
        }
        
        if let Ok(ttl) = std::env::var("ZRC_MESSAGE_TTL_SECS") {
            config.message_ttl_secs = ttl.parse()?;
        }
        
        if let Ok(mode) = std::env::var("ZRC_AUTH_MODE") {
            config.auth_mode = mode;
        }
        
        if let Ok(tokens) = std::env::var("ZRC_SERVER_TOKENS") {
            config.server_tokens = tokens.split(',').map(|s| s.trim().to_string()).collect();
        }
        
        Ok(config)
    }

    pub fn from_toml(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.max_message_size == 0 {
            anyhow::bail!("max_message_size must be > 0");
        }
        
        if self.max_queue_length == 0 {
            anyhow::bail!("max_queue_length must be > 0");
        }
        
        if self.message_ttl_secs == 0 {
            anyhow::bail!("message_ttl_secs must be > 0");
        }
        
        if let (Some(_), None) | (None, Some(_)) = (&self.tls_cert_path, &self.tls_key_path) {
            anyhow::bail!("both tls_cert_path and tls_key_path must be set or both unset");
        }
        
        Ok(())
    }

    pub fn auth_mode_enum(&self) -> AuthMode {
        match self.auth_mode.as_str() {
            "server_wide" => AuthMode::ServerWide,
            "per_mailbox" => AuthMode::PerMailbox,
            _ => AuthMode::Disabled,
        }
    }

    pub fn message_ttl(&self) -> Duration {
        Duration::from_secs(self.message_ttl_secs)
    }

    pub fn eviction_interval(&self) -> Duration {
        Duration::from_secs(self.eviction_interval_secs)
    }

    pub fn idle_mailbox_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_mailbox_timeout_secs)
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_secs)
    }
}
