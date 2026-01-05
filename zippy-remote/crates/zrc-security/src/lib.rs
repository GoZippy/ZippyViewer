#![forbid(unsafe_code)]

//! ZRC Security Module
//!
//! This module provides security controls for the ZRC system including:
//! - Identity pinning and verification
//! - SAS (Short Authentication String) verification
//! - Replay protection
//! - Session key derivation
//! - Rate limiting
//! - Audit logging
//! - Downgrade protection
//! - Key compromise recovery

pub mod error;
pub mod identity;
pub mod sas;
pub mod replay;
pub mod session_keys;
pub mod rate_limit;
pub mod audit;
pub mod downgrade;
pub mod key_recovery;

#[cfg(test)]
mod proptests;

pub use error::SecurityError;
