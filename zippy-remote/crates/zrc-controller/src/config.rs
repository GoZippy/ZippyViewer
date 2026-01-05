//! Configuration management for zrc-controller
//!
//! This module handles loading, saving, and validating configuration for the
//! ZRC controller CLI. Configuration is stored in TOML format.
//!
//! # Configuration File Locations
//!
//! - Unix: `~/.config/zrc/controller.toml`
//! - Windows: `%APPDATA%\zrc\controller.toml`
//!
//! # Requirements Coverage
//!
//! - 10.1: Platform-specific config paths
//! - 10.2: default_transport, rendezvous_urls, relay_urls
//! - 10.3: timeout_seconds, output_format, log_level
//! - 10.4: identity_key_path, pairings_db_path
//! - 10.5: CLI override precedence
//! - 10.6: --config flag support
//! - 10.7: Default config creation
//! - 10.8: Config validation

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read config file
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    /// Failed to parse config file
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),

    /// Failed to serialize config
    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),

    /// Invalid configuration value
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Controller configuration
/// Requirements: 10.1, 10.2, 10.3, 10.4
///
/// # Example TOML
///
/// ```toml
/// [identity]
/// key_path = ""  # Empty = default location
/// key_store = "os"  # "os" | "file"
///
/// [transport]
/// default = "auto"  # "auto" | "mesh" | "rendezvous" | "direct" | "relay"
/// rendezvous_urls = ["https://rendezvous.zippyremote.io"]
/// relay_urls = ["https://relay.zippyremote.io"]
/// mesh_nodes = []
/// timeout_seconds = 30
///
/// [output]
/// format = "table"  # "table" | "json" | "quiet"
/// verbose = false
/// colors = true
///
/// [pairings]
/// db_path = ""  # Empty = default location
///
/// [logging]
/// level = "warn"
/// file = ""
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Identity configuration
    #[serde(default)]
    pub identity: IdentityConfig,

    /// Transport configuration
    #[serde(default)]
    pub transport: TransportConfig,

    /// Output configuration
    #[serde(default)]
    pub output: OutputConfig,

    /// Pairings configuration
    #[serde(default)]
    pub pairings: PairingsConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            identity: IdentityConfig::default(),
            transport: TransportConfig::default(),
            output: OutputConfig::default(),
            pairings: PairingsConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// Identity configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    /// Path to identity key file (empty = default location)
    #[serde(default)]
    pub key_path: Option<PathBuf>,

    /// Key storage method: "os" or "file"
    #[serde(default = "default_key_store")]
    pub key_store: String,
}

fn default_key_store() -> String {
    "os".to_string()
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            key_path: None,
            key_store: default_key_store(),
        }
    }
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Default transport: "auto", "mesh", "rendezvous", "direct", "relay"
    #[serde(default = "default_transport")]
    pub default: String,

    /// Rendezvous server URLs
    #[serde(default)]
    pub rendezvous_urls: Vec<String>,

    /// Relay server URLs
    #[serde(default)]
    pub relay_urls: Vec<String>,

    /// Mesh node addresses
    #[serde(default)]
    pub mesh_nodes: Vec<String>,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_transport() -> String {
    "auto".to_string()
}

fn default_timeout() -> u64 {
    30
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            default: default_transport(),
            rendezvous_urls: vec!["https://rendezvous.zippyremote.io".to_string()],
            relay_urls: vec!["https://relay.zippyremote.io".to_string()],
            mesh_nodes: Vec::new(),
            timeout_seconds: default_timeout(),
        }
    }
}

/// Output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output format: "table", "json", "quiet"
    #[serde(default = "default_format")]
    pub format: String,

    /// Verbose output
    #[serde(default)]
    pub verbose: bool,

    /// Enable colors
    #[serde(default = "default_colors")]
    pub colors: bool,
}

fn default_format() -> String {
    "table".to_string()
}

fn default_colors() -> bool {
    true
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            verbose: false,
            colors: default_colors(),
        }
    }
}

/// Pairings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingsConfig {
    /// Path to pairings database (empty = default location)
    #[serde(default)]
    pub db_path: Option<PathBuf>,
}

impl Default for PairingsConfig {
    fn default() -> Self {
        Self { db_path: None }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: "error", "warn", "info", "debug", "trace"
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log file path (empty = stderr only)
    #[serde(default)]
    pub file: Option<PathBuf>,
}

fn default_log_level() -> String {
    "warn".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
        }
    }
}

impl Config {
    /// Load configuration from file
    /// Requirements: 10.1, 10.6
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from default location
    /// Requirements: 10.1
    pub fn load_default() -> Result<Self, ConfigError> {
        if let Some(path) = Self::default_path() {
            if path.exists() {
                return Self::load(&path);
            }
        }
        Ok(Self::default())
    }

    /// Load configuration from custom path or default
    /// Requirements: 10.6
    pub fn load_from(custom_path: Option<&Path>) -> Result<Self, ConfigError> {
        if let Some(path) = custom_path {
            Self::load(path)
        } else {
            Self::load_default()
        }
    }

    /// Get default configuration file path
    /// Requirements: 10.1
    ///
    /// Returns platform-specific path:
    /// - Unix: `~/.config/zrc/controller.toml`
    /// - Windows: `%APPDATA%\zrc\controller.toml`
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("io", "zippyremote", "zrc")
            .map(|dirs| dirs.config_dir().join("controller.toml"))
    }

    /// Get the config directory path
    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("io", "zippyremote", "zrc")
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the data directory path (for pairings, etc.)
    pub fn data_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("io", "zippyremote", "zrc")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create default configuration file if it doesn't exist
    /// Requirements: 10.7
    pub fn create_default_if_missing() -> Result<bool, ConfigError> {
        if let Some(path) = Self::default_path() {
            if !path.exists() {
                let config = Self::default();
                config.save(&path)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Validate configuration values
    /// Requirements: 10.8
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate transport preference
        let valid_transports = ["auto", "mesh", "rendezvous", "direct", "relay"];
        if !valid_transports.contains(&self.transport.default.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid transport '{}'. Valid values: {:?}",
                self.transport.default, valid_transports
            )));
        }

        // Validate output format
        let valid_formats = ["table", "json", "quiet"];
        if !valid_formats.contains(&self.output.format.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid output format '{}'. Valid values: {:?}",
                self.output.format, valid_formats
            )));
        }

        // Validate log level
        let valid_levels = ["error", "warn", "info", "debug", "trace"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid log level '{}'. Valid values: {:?}",
                self.logging.level, valid_levels
            )));
        }

        // Validate key store
        let valid_stores = ["os", "file"];
        if !valid_stores.contains(&self.identity.key_store.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid key_store '{}'. Valid values: {:?}",
                self.identity.key_store, valid_stores
            )));
        }

        // Validate timeout
        if self.transport.timeout_seconds == 0 {
            return Err(ConfigError::ValidationError(
                "timeout_seconds must be greater than 0".to_string(),
            ));
        }

        // Validate URLs (basic check)
        for url in &self.transport.rendezvous_urls {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid rendezvous URL '{}': must start with http:// or https://",
                    url
                )));
            }
        }

        for url in &self.transport.relay_urls {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid relay URL '{}': must start with http:// or https://",
                    url
                )));
            }
        }

        Ok(())
    }

    /// Generate a sample configuration file content
    pub fn sample_toml() -> &'static str {
        r#"# ZRC Controller Configuration
# Requirements: 10.1, 10.2, 10.3, 10.4

[identity]
# Path to identity key file (empty = default location)
# key_path = ""
# Key storage method: "os" (OS keystore) or "file"
key_store = "os"

[transport]
# Default transport: "auto", "mesh", "rendezvous", "direct", "relay"
default = "auto"
# Rendezvous server URLs
rendezvous_urls = ["https://rendezvous.zippyremote.io"]
# Relay server URLs
relay_urls = ["https://relay.zippyremote.io"]
# Mesh node addresses
mesh_nodes = []
# Connection timeout in seconds
timeout_seconds = 30

[output]
# Output format: "table", "json", "quiet"
format = "table"
# Enable verbose output
verbose = false
# Enable colored output
colors = true

[pairings]
# Path to pairings database (empty = default location)
# db_path = ""

[logging]
# Log level: "error", "warn", "info", "debug", "trace"
level = "warn"
# Log file path (empty = stderr only)
# file = ""
"#
    }
}


/// CLI configuration overrides
/// Requirements: 10.5
///
/// This struct captures CLI flags that can override config file values.
/// Command-line arguments take precedence over config file values.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// Output format override
    pub output_format: Option<String>,
    /// Verbose flag override
    pub verbose: Option<bool>,
    /// Debug flag override
    pub debug: Option<bool>,
    /// Transport preference override
    pub transport: Option<String>,
    /// Rendezvous URLs override
    pub rendezvous_urls: Option<Vec<String>>,
    /// Relay URLs override
    pub relay_urls: Option<Vec<String>>,
    /// Mesh nodes override
    pub mesh_nodes: Option<Vec<String>>,
}

impl Config {
    /// Apply CLI overrides to configuration
    /// Requirements: 10.5
    ///
    /// CLI arguments take precedence over config file values.
    pub fn with_overrides(mut self, overrides: &CliOverrides) -> Self {
        if let Some(ref format) = overrides.output_format {
            self.output.format = format.clone();
        }
        if let Some(verbose) = overrides.verbose {
            self.output.verbose = verbose;
        }
        if let Some(debug) = overrides.debug {
            if debug {
                self.logging.level = "debug".to_string();
            }
        }
        if let Some(ref transport) = overrides.transport {
            self.transport.default = transport.clone();
        }
        if let Some(ref urls) = overrides.rendezvous_urls {
            if !urls.is_empty() {
                self.transport.rendezvous_urls = urls.clone();
            }
        }
        if let Some(ref urls) = overrides.relay_urls {
            if !urls.is_empty() {
                self.transport.relay_urls = urls.clone();
            }
        }
        if let Some(ref nodes) = overrides.mesh_nodes {
            if !nodes.is_empty() {
                self.transport.mesh_nodes = nodes.clone();
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Test default configuration values
    #[test]
    fn test_default_config() {
        let config = Config::default();
        
        // Identity defaults
        assert_eq!(config.identity.key_store, "os");
        assert!(config.identity.key_path.is_none());
        
        // Transport defaults
        assert_eq!(config.transport.default, "auto");
        assert!(!config.transport.rendezvous_urls.is_empty());
        assert!(!config.transport.relay_urls.is_empty());
        assert!(config.transport.mesh_nodes.is_empty());
        assert_eq!(config.transport.timeout_seconds, 30);
        
        // Output defaults
        assert_eq!(config.output.format, "table");
        assert!(!config.output.verbose);
        assert!(config.output.colors);
        
        // Pairings defaults
        assert!(config.pairings.db_path.is_none());
        
        // Logging defaults
        assert_eq!(config.logging.level, "warn");
        assert!(config.logging.file.is_none());
    }

    /// Test config validation - valid config
    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    /// Test config validation - invalid transport
    #[test]
    fn test_validate_invalid_transport() {
        let mut config = Config::default();
        config.transport.default = "invalid".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid transport"));
    }

    /// Test config validation - invalid output format
    #[test]
    fn test_validate_invalid_output_format() {
        let mut config = Config::default();
        config.output.format = "xml".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid output format"));
    }

    /// Test config validation - invalid log level
    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "verbose".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid log level"));
    }

    /// Test config validation - invalid key store
    #[test]
    fn test_validate_invalid_key_store() {
        let mut config = Config::default();
        config.identity.key_store = "cloud".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid key_store"));
    }

    /// Test config validation - zero timeout
    #[test]
    fn test_validate_zero_timeout() {
        let mut config = Config::default();
        config.transport.timeout_seconds = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout_seconds"));
    }

    /// Test config validation - invalid URL
    #[test]
    fn test_validate_invalid_url() {
        let mut config = Config::default();
        config.transport.rendezvous_urls = vec!["not-a-url".to_string()];
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid rendezvous URL"));
    }

    /// Test config save and load round-trip
    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let config = Config::default();
        config.save(&config_path).unwrap();
        
        let loaded = Config::load(&config_path).unwrap();
        
        assert_eq!(config.transport.default, loaded.transport.default);
        assert_eq!(config.output.format, loaded.output.format);
        assert_eq!(config.logging.level, loaded.logging.level);
    }

    /// Test CLI overrides
    /// Requirements: 10.5
    #[test]
    fn test_cli_overrides() {
        let config = Config::default();
        
        let overrides = CliOverrides {
            output_format: Some("json".to_string()),
            verbose: Some(true),
            debug: Some(true),
            transport: Some("direct".to_string()),
            rendezvous_urls: Some(vec!["https://custom.example.com".to_string()]),
            relay_urls: None,
            mesh_nodes: None,
        };
        
        let config = config.with_overrides(&overrides);
        
        assert_eq!(config.output.format, "json");
        assert!(config.output.verbose);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.transport.default, "direct");
        assert_eq!(config.transport.rendezvous_urls, vec!["https://custom.example.com"]);
        // relay_urls should remain default since override was None
        assert!(!config.transport.relay_urls.is_empty());
    }

    /// Test CLI overrides with empty vectors don't override
    #[test]
    fn test_cli_overrides_empty_vectors() {
        let config = Config::default();
        let original_urls = config.transport.rendezvous_urls.clone();
        
        let overrides = CliOverrides {
            rendezvous_urls: Some(vec![]), // Empty vector
            ..Default::default()
        };
        
        let config = config.with_overrides(&overrides);
        
        // Empty vector should not override
        assert_eq!(config.transport.rendezvous_urls, original_urls);
    }

    /// Test TOML parsing
    #[test]
    fn test_toml_parsing() {
        let toml_content = r#"
[identity]
key_store = "file"

[transport]
default = "rendezvous"
rendezvous_urls = ["https://example.com"]
timeout_seconds = 60

[output]
format = "json"
verbose = true

[logging]
level = "debug"
"#;
        
        let config: Config = toml::from_str(toml_content).unwrap();
        
        assert_eq!(config.identity.key_store, "file");
        assert_eq!(config.transport.default, "rendezvous");
        assert_eq!(config.transport.rendezvous_urls, vec!["https://example.com"]);
        assert_eq!(config.transport.timeout_seconds, 60);
        assert_eq!(config.output.format, "json");
        assert!(config.output.verbose);
        assert_eq!(config.logging.level, "debug");
    }

    /// Test sample TOML is valid
    #[test]
    fn test_sample_toml_is_valid() {
        let sample = Config::sample_toml();
        let config: Result<Config, _> = toml::from_str(sample);
        assert!(config.is_ok(), "Sample TOML should be valid: {:?}", config.err());
    }

    /// Test default path exists on supported platforms
    #[test]
    fn test_default_path() {
        // This should return Some on all supported platforms
        let path = Config::default_path();
        assert!(path.is_some());
        
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("controller.toml"));
    }

    /// Test config directory path
    #[test]
    fn test_config_dir() {
        let dir = Config::config_dir();
        assert!(dir.is_some());
    }

    /// Test data directory path
    #[test]
    fn test_data_dir() {
        let dir = Config::data_dir();
        assert!(dir.is_some());
    }

    /// Test create_default_if_missing
    #[test]
    fn test_create_default_if_missing() {
        // This test just verifies the function doesn't panic
        // Actual file creation depends on filesystem permissions
        let _ = Config::create_default_if_missing();
    }

    /// Test load_from with custom path
    #[test]
    fn test_load_from_custom_path() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("custom.toml");
        
        let config = Config::default();
        config.save(&config_path).unwrap();
        
        let loaded = Config::load_from(Some(&config_path)).unwrap();
        assert_eq!(config.transport.default, loaded.transport.default);
    }

    /// Test load_from with None uses default
    #[test]
    fn test_load_from_none() {
        // Should not panic, returns default if no config exists
        let config = Config::load_from(None);
        assert!(config.is_ok());
    }
}
