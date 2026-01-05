use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    FileNotFound(String),
    #[error("config parse error: {0}")]
    ParseError(String),
    #[error("config validation error: {0}")]
    ValidationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub bind_addr: String,
    pub rendezvous_url: Option<String>,
    
    // ICE/TURN configuration
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServerConfig>,
    
    // Capture settings
    pub capture_fps: u32,
    pub capture_quality: u8, // 0-100
    
    // Session settings
    pub max_concurrent_sessions: usize,
    pub session_timeout_secs: u64,
    
    // Policy settings
    pub consent_mode: String, // "always_require", "unattended_allowed", "trusted_only"
    pub allow_unattended: bool,
    
    // Logging
    pub log_level: String,
    pub log_file: Option<PathBuf>,
    pub audit_log: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServerConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".to_string(),
            rendezvous_url: None,
            stun_servers: vec!["stun:stun.l.google.com:19302".to_string()],
            turn_servers: Vec::new(),
            capture_fps: 30,
            capture_quality: 80,
            max_concurrent_sessions: 1,
            session_timeout_secs: 28800, // 8 hours
            consent_mode: "always_require".to_string(),
            allow_unattended: false,
            log_level: "info".to_string(),
            log_file: None,
            audit_log: None,
        }
    }
}

impl AgentConfig {
    pub fn load_from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileNotFound(e.to_string()))?;
        
        let config: AgentConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        config.validate()?;
        Ok(config)
    }

    pub fn load_from_env() -> Self {
        let mut config = Self::default();
        
        if let Ok(addr) = std::env::var("ZRC_BIND_ADDR") {
            config.bind_addr = addr;
        }
        if let Ok(url) = std::env::var("ZRC_RENDEZVOUS_URL") {
            config.rendezvous_url = Some(url);
        }
        if let Ok(fps) = std::env::var("ZRC_CAPTURE_FPS") {
            if let Ok(fps_val) = fps.parse::<u32>() {
                config.capture_fps = fps_val;
            }
        }
        if let Ok(level) = std::env::var("RUST_LOG") {
            config.log_level = level;
        }
        
        config
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.capture_fps == 0 || self.capture_fps > 60 {
            return Err(ConfigError::ValidationError(
                "capture_fps must be between 1 and 60".to_string()
            ));
        }
        if self.capture_quality > 100 {
            return Err(ConfigError::ValidationError(
                "capture_quality must be between 0 and 100".to_string()
            ));
        }
        if self.max_concurrent_sessions == 0 {
            return Err(ConfigError::ValidationError(
                "max_concurrent_sessions must be at least 1".to_string()
            ));
        }
        Ok(())
    }
}
