//! Error types for iOS platform

use thiserror::Error;
use uniffi::Enum;

/// ZRC error types exposed to Swift
#[derive(Debug, Error, Enum, Clone)]
pub enum ZrcError {
    #[error("General error: {0}")]
    General(String),

    #[error("Device not paired")]
    NotPaired,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Timeout")]
    Timeout,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Transport error: {0}")]
    Transport(String),
}

impl From<anyhow::Error> for ZrcError {
    fn from(e: anyhow::Error) -> Self {
        ZrcError::General(e.to_string())
    }
}

impl From<zrc_core::errors::ZrcError> for ZrcError {
    fn from(e: zrc_core::errors::ZrcError) -> Self {
        match e {
            zrc_core::errors::ZrcError::NotPaired => ZrcError::NotPaired,
            zrc_core::errors::ZrcError::SessionNotFound => ZrcError::SessionNotFound,
            zrc_core::errors::ZrcError::ConnectionFailed(msg) => ZrcError::ConnectionFailed(msg),
            zrc_core::errors::ZrcError::AuthenticationFailed(msg) => {
                ZrcError::AuthenticationFailed(msg)
            }
            zrc_core::errors::ZrcError::Timeout => ZrcError::Timeout,
            zrc_core::errors::ZrcError::InvalidInput(msg) => ZrcError::InvalidInput(msg),
            zrc_core::errors::ZrcError::PermissionDenied(msg) => ZrcError::PermissionDenied(msg),
            zrc_core::errors::ZrcError::Crypto(msg) => ZrcError::Crypto(msg),
            zrc_core::errors::ZrcError::Store(msg) => ZrcError::Store(msg),
            zrc_core::errors::ZrcError::Transport(msg) => ZrcError::Transport(msg),
            _ => ZrcError::General(e.to_string()),
        }
    }
}
