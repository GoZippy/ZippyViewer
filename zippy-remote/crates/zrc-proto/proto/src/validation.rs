//! Validation helpers for ZRC protocol messages.
//!
//! This module provides validation methods for message fields including:
//! - Size validation for byte fields (identifiers, keys, nonces)
//! - Timestamp validation for expiration and creation times
//!
//! Requirements: 10.1, 10.2

use crate::v1::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Validation error types for protocol messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Field has invalid size (expected, actual)
    InvalidSize { field: &'static str, expected: usize, actual: usize },
    /// Field size is out of allowed range
    SizeOutOfRange { field: &'static str, min: usize, max: usize, actual: usize },
    /// Timestamp is in the past (for expiration fields)
    TimestampExpired { field: &'static str, timestamp: u64 },
    /// Timestamp is too far in the future
    TimestampTooFarFuture { field: &'static str, timestamp: u64, max_future_secs: u64 },
    /// TTL exceeds maximum allowed value
    TtlExceedsMax { field: &'static str, value: u32, max: u32 },
    /// Required field is empty
    EmptyField { field: &'static str },
    /// Field contains invalid data
    InvalidData { field: &'static str, reason: &'static str },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSize { field, expected, actual } => {
                write!(f, "field '{}' has invalid size: expected {}, got {}", field, expected, actual)
            }
            Self::SizeOutOfRange { field, min, max, actual } => {
                write!(f, "field '{}' size {} is out of range [{}, {}]", field, actual, min, max)
            }
            Self::TimestampExpired { field, timestamp } => {
                write!(f, "field '{}' timestamp {} has expired", field, timestamp)
            }
            Self::TimestampTooFarFuture { field, timestamp, max_future_secs } => {
                write!(f, "field '{}' timestamp {} is more than {}s in the future", field, timestamp, max_future_secs)
            }
            Self::TtlExceedsMax { field, value, max } => {
                write!(f, "field '{}' TTL {} exceeds maximum {}", field, value, max)
            }
            Self::EmptyField { field } => {
                write!(f, "required field '{}' is empty", field)
            }
            Self::InvalidData { field, reason } => {
                write!(f, "field '{}' contains invalid data: {}", field, reason)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result type for validation operations.
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Constants for field sizes.
pub mod sizes {
    /// Size of device/operator identifiers (SHA-256 hash).
    pub const ID_SIZE: usize = 32;
    /// Size of Ed25519 public keys.
    pub const ED25519_PUB_SIZE: usize = 32;
    /// Size of X25519 public keys.
    pub const X25519_PUB_SIZE: usize = 32;
    /// Size of Ed25519 signatures.
    pub const ED25519_SIG_SIZE: usize = 64;
    /// Size of nonces for ChaCha20Poly1305.
    pub const NONCE_SIZE: usize = 24;
    /// Size of replay protection nonces.
    pub const REPLAY_NONCE_SIZE: usize = 32;
    /// Size of ticket identifiers.
    pub const TICKET_ID_SIZE: usize = 16;
    /// Size of session identifiers.
    pub const SESSION_ID_SIZE: usize = 32;
    /// Size of discovery token identifiers.
    pub const DISCOVERY_TOKEN_ID_SIZE: usize = 16;
    /// Size of DTLS fingerprints (SHA-256).
    pub const DTLS_FINGERPRINT_SIZE: usize = 32;
    /// Maximum TTL for directory records (24 hours).
    pub const MAX_DIR_RECORD_TTL: u32 = 86400;
}

/// Get current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Validate that a byte field has the expected exact size.
fn validate_exact_size(field: &'static str, data: &[u8], expected: usize) -> ValidationResult<()> {
    if data.len() != expected {
        return Err(ValidationError::InvalidSize {
            field,
            expected,
            actual: data.len(),
        });
    }
    Ok(())
}

/// Validate that a byte field is not empty.
fn validate_not_empty(field: &'static str, data: &[u8]) -> ValidationResult<()> {
    if data.is_empty() {
        return Err(ValidationError::EmptyField { field });
    }
    Ok(())
}

/// Validate that a timestamp is not expired.
fn validate_not_expired(field: &'static str, timestamp: u64) -> ValidationResult<()> {
    let now = current_timestamp();
    if timestamp < now {
        return Err(ValidationError::TimestampExpired { field, timestamp });
    }
    Ok(())
}

/// Validate that a timestamp is not too far in the future.
fn validate_not_too_future(field: &'static str, timestamp: u64, max_future_secs: u64) -> ValidationResult<()> {
    let now = current_timestamp();
    let max_allowed = now.saturating_add(max_future_secs);
    if timestamp > max_allowed {
        return Err(ValidationError::TimestampTooFarFuture {
            field,
            timestamp,
            max_future_secs,
        });
    }
    Ok(())
}

// ============================================================================
// Validation trait and implementations
// ============================================================================

/// Trait for validating protocol messages.
pub trait Validate {
    /// Validate the message fields.
    fn validate(&self) -> ValidationResult<()>;
}

impl Validate for DeviceIdV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("id", &self.id, sizes::ID_SIZE)
    }
}

impl Validate for OperatorIdV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("id", &self.id, sizes::ID_SIZE)
    }
}

impl Validate for PublicKeyBundleV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("sign_pub", &self.sign_pub, sizes::ED25519_PUB_SIZE)?;
        validate_exact_size("kex_pub", &self.kex_pub, sizes::X25519_PUB_SIZE)?;
        Ok(())
    }
}

impl Validate for InviteV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("device_id", &self.device_id, sizes::ID_SIZE)?;
        validate_exact_size("device_sign_pub", &self.device_sign_pub, sizes::ED25519_PUB_SIZE)?;
        validate_exact_size("invite_secret_hash", &self.invite_secret_hash, sizes::ID_SIZE)?;
        validate_not_expired("expires_at", self.expires_at)?;
        Ok(())
    }
}

impl Validate for PairRequestV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("operator_id", &self.operator_id, sizes::ID_SIZE)?;
        validate_exact_size("operator_sign_pub", &self.operator_sign_pub, sizes::ED25519_PUB_SIZE)?;
        validate_exact_size("operator_kex_pub", &self.operator_kex_pub, sizes::X25519_PUB_SIZE)?;
        validate_not_empty("invite_proof", &self.invite_proof)?;
        validate_exact_size("nonce", &self.nonce, sizes::REPLAY_NONCE_SIZE)?;
        // Timestamp should be recent (within 5 minutes)
        let now = current_timestamp();
        let five_minutes = 300;
        if self.timestamp < now.saturating_sub(five_minutes) {
            return Err(ValidationError::TimestampExpired {
                field: "timestamp",
                timestamp: self.timestamp,
            });
        }
        validate_not_too_future("timestamp", self.timestamp, five_minutes)?;
        Ok(())
    }
}

impl Validate for PairReceiptV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("device_id", &self.device_id, sizes::ID_SIZE)?;
        validate_exact_size("operator_id", &self.operator_id, sizes::ID_SIZE)?;
        validate_not_empty("device_signature", &self.device_signature)?;
        Ok(())
    }
}

impl Validate for SessionTicketV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("ticket_id", &self.ticket_id, sizes::TICKET_ID_SIZE)?;
        validate_exact_size("session_id", &self.session_id, sizes::SESSION_ID_SIZE)?;
        validate_exact_size("operator_id", &self.operator_id, sizes::ID_SIZE)?;
        validate_exact_size("device_id", &self.device_id, sizes::ID_SIZE)?;
        validate_not_expired("expires_at", self.expires_at)?;
        validate_not_empty("device_signature", &self.device_signature)?;
        Ok(())
    }
}

impl Validate for SessionInitRequestV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("operator_id", &self.operator_id, sizes::ID_SIZE)?;
        validate_exact_size("device_id", &self.device_id, sizes::ID_SIZE)?;
        validate_exact_size("session_id", &self.session_id, sizes::SESSION_ID_SIZE)?;
        validate_not_empty("operator_signature", &self.operator_signature)?;
        Ok(())
    }
}

impl Validate for SessionInitResponseV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("session_id", &self.session_id, sizes::SESSION_ID_SIZE)?;
        validate_not_empty("device_signature", &self.device_signature)?;
        Ok(())
    }
}

impl Validate for EnvelopeHeaderV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("sender_id", &self.sender_id, sizes::ID_SIZE)?;
        validate_exact_size("recipient_id", &self.recipient_id, sizes::ID_SIZE)?;
        validate_exact_size("nonce", &self.nonce, sizes::NONCE_SIZE)?;
        Ok(())
    }
}

impl Validate for EnvelopeV1 {
    fn validate(&self) -> ValidationResult<()> {
        if let Some(ref header) = self.header {
            header.validate()?;
        } else {
            return Err(ValidationError::EmptyField { field: "header" });
        }
        validate_exact_size("sender_kex_pub", &self.sender_kex_pub, sizes::X25519_PUB_SIZE)?;
        validate_not_empty("encrypted_payload", &self.encrypted_payload)?;
        validate_not_empty("signature", &self.signature)?;
        Ok(())
    }
}

impl Validate for DirRecordV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("subject_id", &self.subject_id, sizes::ID_SIZE)?;
        validate_exact_size("device_sign_pub", &self.device_sign_pub, sizes::ED25519_PUB_SIZE)?;
        if self.ttl_seconds > sizes::MAX_DIR_RECORD_TTL {
            return Err(ValidationError::TtlExceedsMax {
                field: "ttl_seconds",
                value: self.ttl_seconds,
                max: sizes::MAX_DIR_RECORD_TTL,
            });
        }
        validate_not_empty("signature", &self.signature)?;
        Ok(())
    }
}

impl Validate for DiscoveryTokenV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("token_id", &self.token_id, sizes::DISCOVERY_TOKEN_ID_SIZE)?;
        validate_exact_size("subject_id", &self.subject_id, sizes::ID_SIZE)?;
        validate_not_expired("expires_at", self.expires_at)?;
        validate_not_empty("signature", &self.signature)?;
        Ok(())
    }
}

impl Validate for CertBindingV1 {
    fn validate(&self) -> ValidationResult<()> {
        validate_exact_size("dtls_fingerprint", &self.dtls_fingerprint, sizes::DTLS_FINGERPRINT_SIZE)?;
        validate_not_empty("fingerprint_signature", &self.fingerprint_signature)?;
        validate_exact_size("signer_pub", &self.signer_pub, sizes::ED25519_PUB_SIZE)?;
        Ok(())
    }
}

impl Validate for ErrorV1 {
    fn validate(&self) -> ValidationResult<()> {
        // Error messages should have a non-empty message
        if self.error_message.is_empty() {
            return Err(ValidationError::EmptyField { field: "error_message" });
        }
        Ok(())
    }
}

// ============================================================================
// Helper methods for timestamp validation
// ============================================================================

/// Check if a Unix timestamp is expired (in the past).
pub fn is_timestamp_expired(timestamp: u64) -> bool {
    timestamp < current_timestamp()
}

/// Check if a Unix timestamp is valid (not expired and not too far in the future).
/// 
/// # Arguments
/// * `timestamp` - Unix timestamp to validate
/// * `max_future_secs` - Maximum seconds in the future allowed
pub fn is_timestamp_valid(timestamp: u64, max_future_secs: u64) -> bool {
    let now = current_timestamp();
    timestamp >= now && timestamp <= now.saturating_add(max_future_secs)
}

/// Validate a timestamp for expiration fields (must be in the future).
pub fn validate_expiration_timestamp(field: &'static str, timestamp: u64) -> ValidationResult<()> {
    validate_not_expired(field, timestamp)
}

/// Validate a timestamp for creation fields (must be recent, within tolerance).
pub fn validate_creation_timestamp(field: &'static str, timestamp: u64, tolerance_secs: u64) -> ValidationResult<()> {
    let now = current_timestamp();
    if timestamp < now.saturating_sub(tolerance_secs) {
        return Err(ValidationError::TimestampExpired { field, timestamp });
    }
    validate_not_too_future(field, timestamp, tolerance_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_validation() {
        let valid = DeviceIdV1 { id: vec![0u8; 32] };
        assert!(valid.validate().is_ok());

        let invalid = DeviceIdV1 { id: vec![0u8; 16] };
        assert!(matches!(
            invalid.validate(),
            Err(ValidationError::InvalidSize { field: "id", expected: 32, actual: 16 })
        ));
    }

    #[test]
    fn test_public_key_bundle_validation() {
        let valid = PublicKeyBundleV1 {
            sign_pub: vec![0u8; 32],
            kex_pub: vec![0u8; 32],
        };
        assert!(valid.validate().is_ok());

        let invalid_sign = PublicKeyBundleV1 {
            sign_pub: vec![0u8; 16],
            kex_pub: vec![0u8; 32],
        };
        assert!(invalid_sign.validate().is_err());
    }

    #[test]
    fn test_dir_record_ttl_validation() {
        let valid = DirRecordV1 {
            subject_id: vec![0u8; 32],
            device_sign_pub: vec![0u8; 32],
            endpoints: None,
            ttl_seconds: 3600, // 1 hour
            timestamp: current_timestamp(),
            signature: vec![1, 2, 3],
        };
        assert!(valid.validate().is_ok());

        let invalid_ttl = DirRecordV1 {
            subject_id: vec![0u8; 32],
            device_sign_pub: vec![0u8; 32],
            endpoints: None,
            ttl_seconds: 100000, // > 24 hours
            timestamp: current_timestamp(),
            signature: vec![1, 2, 3],
        };
        assert!(matches!(
            invalid_ttl.validate(),
            Err(ValidationError::TtlExceedsMax { .. })
        ));
    }

    #[test]
    fn test_timestamp_helpers() {
        let now = current_timestamp();
        
        // Past timestamp should be expired
        assert!(is_timestamp_expired(now - 100));
        
        // Future timestamp should not be expired
        assert!(!is_timestamp_expired(now + 100));
        
        // Valid timestamp within range
        assert!(is_timestamp_valid(now + 100, 200));
        
        // Invalid timestamp too far in future
        assert!(!is_timestamp_valid(now + 1000, 200));
    }
}
