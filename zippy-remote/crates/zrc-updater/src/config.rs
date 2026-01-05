//! Configuration structures for the update system.
//!
//! Defines configuration for update behavior, security settings,
//! and rollback management.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::channel::UpdateChannel;

/// Main update configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Update channel (stable, beta, nightly, or custom)
    #[serde(default)]
    pub channel: UpdateChannel,

    /// Interval between automatic update checks in hours
    #[serde(default = "default_check_interval")]
    pub check_interval_hours: u32,

    /// Whether to automatically download updates
    #[serde(default = "default_true")]
    pub auto_download: bool,

    /// Whether to automatically install updates (requires restart)
    #[serde(default)]
    pub auto_install: bool,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// Rollback configuration
    #[serde(default)]
    pub rollback: RollbackConfig,

    /// Network configuration
    #[serde(default)]
    pub network: NetworkConfig,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: UpdateChannel::default(),
            check_interval_hours: default_check_interval(),
            auto_download: true,
            auto_install: false,
            security: SecurityConfig::default(),
            rollback: RollbackConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

impl UpdateConfig {
    /// Load configuration from a TOML file.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, crate::error::UpdateError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::error::UpdateError::ConfigError(e.to_string()))?;
        Ok(config)
    }

    /// Save configuration to a TOML file.
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), crate::error::UpdateError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::UpdateError::ConfigError(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Security configuration for update verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Pinned public keys for manifest signing (Ed25519, hex or base64 encoded)
    /// Format: "ed25519:<hex_or_base64_public_key>"
    #[serde(default)]
    pub manifest_keys: Vec<String>,

    /// Minimum number of valid signatures required
    #[serde(default = "default_signature_threshold")]
    pub signature_threshold: usize,

    /// Whether to verify platform code signatures (Authenticode, codesign)
    #[serde(default = "default_true")]
    pub verify_code_signature: bool,

    /// Expected certificate thumbprint for Windows Authenticode
    #[serde(default)]
    pub windows_cert_thumbprint: Option<String>,

    /// Expected team ID for macOS code signing
    #[serde(default)]
    pub macos_team_id: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            manifest_keys: Vec::new(),
            signature_threshold: default_signature_threshold(),
            verify_code_signature: true,
            windows_cert_thumbprint: None,
            macos_team_id: None,
        }
    }
}

impl SecurityConfig {
    /// Parse manifest keys into Ed25519 verifying keys.
    pub fn parse_manifest_keys(
        &self,
    ) -> Result<Vec<ed25519_dalek::VerifyingKey>, crate::error::UpdateError> {
        let mut keys = Vec::new();

        for key_str in &self.manifest_keys {
            let key = parse_ed25519_key(key_str)?;
            keys.push(key);
        }

        Ok(keys)
    }
}

/// Parse an Ed25519 public key from string format.
///
/// Supports formats:
/// - "ed25519:<hex_encoded_32_bytes>"
/// - "ed25519:<base64_encoded_32_bytes>"
fn parse_ed25519_key(s: &str) -> Result<ed25519_dalek::VerifyingKey, crate::error::UpdateError> {
    let key_data = if let Some(hex_str) = s.strip_prefix("ed25519:") {
        // Try hex first
        if let Ok(bytes) = hex::decode(hex_str) {
            bytes
        } else {
            // Try base64
            decode_base64(hex_str).map_err(|e| {
                crate::error::UpdateError::ConfigError(format!("invalid key encoding: {}", e))
            })?
        }
    } else {
        return Err(crate::error::UpdateError::ConfigError(
            "key must start with 'ed25519:'".to_string(),
        ));
    };

    let key_bytes: [u8; 32] = key_data.try_into().map_err(|_| {
        crate::error::UpdateError::ConfigError("Ed25519 public key must be 32 bytes".to_string())
    })?;

    ed25519_dalek::VerifyingKey::from_bytes(&key_bytes).map_err(|e| {
        crate::error::UpdateError::ConfigError(format!("invalid Ed25519 public key: {}", e))
    })
}

/// Simple base64 decoder.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        if c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        }
        let val = alphabet
            .iter()
            .position(|&x| x == c)
            .ok_or_else(|| format!("invalid base64 character: {}", c as char))?
            as u32;
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(result)
}

/// Rollback configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    /// Maximum number of backups to retain
    #[serde(default = "default_max_backups")]
    pub max_backups: usize,

    /// Directory for storing backups (empty = default location)
    #[serde(default)]
    pub backup_dir: Option<PathBuf>,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            max_backups: default_max_backups(),
            backup_dir: None,
        }
    }
}

impl RollbackConfig {
    /// Get the backup directory, using default if not specified.
    pub fn backup_dir(&self) -> PathBuf {
        if let Some(dir) = &self.backup_dir {
            dir.clone()
        } else {
            // Default to a subdirectory of the data directory
            dirs_default_backup_dir()
        }
    }
}

/// Network configuration for downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Proxy URL (empty = system proxy)
    #[serde(default)]
    pub proxy: Option<String>,

    /// Maximum download bandwidth in bytes/second (0 = unlimited)
    #[serde(default)]
    pub bandwidth_limit: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            max_retries: default_max_retries(),
            proxy: None,
            bandwidth_limit: 0,
        }
    }
}

// Default value functions for serde
fn default_check_interval() -> u32 {
    24 // Daily
}

fn default_true() -> bool {
    true
}

fn default_signature_threshold() -> usize {
    1
}

fn default_max_backups() -> usize {
    3
}

fn default_timeout() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

/// Get the default backup directory.
fn dirs_default_backup_dir() -> PathBuf {
    // Use platform-appropriate data directory
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(appdata).join("ZippyRemote").join("backups");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ZippyRemote")
                .join("backups");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(data_home).join("zippyremote").join("backups");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("zippyremote")
                .join("backups");
        }
    }

    // Fallback
    PathBuf::from(".").join("backups")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = UpdateConfig::default();
        assert_eq!(config.channel, UpdateChannel::Stable);
        assert_eq!(config.check_interval_hours, 24);
        assert!(config.auto_download);
        assert!(!config.auto_install);
        assert_eq!(config.security.signature_threshold, 1);
        assert_eq!(config.rollback.max_backups, 3);
    }

    #[test]
    fn test_parse_ed25519_key_hex() {
        // Test with invalid prefix
        let key_str = "invalid:0000000000000000000000000000000000000000000000000000000000000000";
        let result = parse_ed25519_key(key_str);
        assert!(result.is_err());

        // Test with wrong length
        let key_str = "ed25519:00000000";
        let result = parse_ed25519_key(key_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.manifest_keys.is_empty());
        assert_eq!(config.signature_threshold, 1);
        assert!(config.verify_code_signature);
    }

    #[test]
    fn test_rollback_config_default() {
        let config = RollbackConfig::default();
        assert_eq!(config.max_backups, 3);
        assert!(config.backup_dir.is_none());
    }

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
        assert!(config.proxy.is_none());
        assert_eq!(config.bandwidth_limit, 0);
    }
}
