//! Error types for Android platform

use thiserror::Error;

/// ZRC Android platform errors
#[derive(Debug, Error)]
pub enum ZrcError {
    #[error("Core error: {0}")]
    Core(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Frame error: {0}")]
    Frame(String),

    #[error("Input error: {0}")]
    Input(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("JNI error: {0}")]
    Jni(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    #[error("General error: {0}")]
    General(String),
    
    #[error("Crypto error: {0}")]
    Crypto(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
}

impl From<zrc_core::errors::CoreError> for ZrcError {
    fn from(e: zrc_core::errors::CoreError) -> Self {
        ZrcError::Core(e.to_string())
    }
}

impl From<serde_json::Error> for ZrcError {
    fn from(e: serde_json::Error) -> Self {
        ZrcError::Config(format!("JSON error: {}", e))
    }
}
