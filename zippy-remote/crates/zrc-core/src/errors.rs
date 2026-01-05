//! Error types for ZRC Core.
//!
//! This module defines comprehensive error types for the ZRC system and provides
//! mapping to wire-safe ErrorV1 messages for transmission to remote peers.
//!
//! Requirements: 11.1, 11.2, 11.3, 11.4

use std::collections::HashMap;
use thiserror::Error;
use zrc_proto::v1::{ErrorCodeV1, ErrorV1};

// ============================================================================
// Core Error Types (Requirements: 11.1, 11.2)
// ============================================================================

/// Authentication errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum AuthError {
    /// Invalid credentials or signature
    #[error("authentication failed: invalid credentials")]
    InvalidCredentials,

    /// Signature verification failed
    #[error("authentication failed: signature verification failed")]
    SignatureInvalid,

    /// Identity not recognized
    #[error("authentication failed: unknown identity")]
    UnknownIdentity,

    /// Pairing required before operation
    #[error("authentication failed: pairing required")]
    PairingRequired,

    /// Invite proof verification failed
    #[error("authentication failed: invalid invite proof")]
    InvalidInviteProof,

    /// Invite has expired
    #[error("authentication failed: invite expired")]
    InviteExpired,
}

/// Permission errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum PermissionError {
    /// Requested permission not granted
    #[error("permission denied: {0}")]
    Denied(String),

    /// Permission exceeds paired permissions
    #[error("permission denied: exceeds paired permissions")]
    ExceedsPairedPermissions,

    /// Permission exceeds policy limits
    #[error("permission denied: exceeds policy limits")]
    ExceedsPolicyLimits,

    /// Consent required but not granted
    #[error("permission denied: consent required")]
    ConsentRequired,

    /// Consent was explicitly denied
    #[error("permission denied: consent denied by user")]
    ConsentDenied,
}

/// Session ticket errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum TicketError {
    /// Ticket has expired
    #[error("ticket expired")]
    Expired,

    /// Ticket has been revoked
    #[error("ticket revoked")]
    Revoked,

    /// Ticket signature is invalid
    #[error("ticket invalid: signature verification failed")]
    InvalidSignature,

    /// Ticket binding mismatch
    #[error("ticket invalid: session binding mismatch")]
    BindingMismatch,

    /// Ticket not found
    #[error("ticket not found")]
    NotFound,

    /// Ticket format is invalid
    #[error("ticket invalid: malformed")]
    Malformed,
}

/// Transport errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum TransportError {
    /// No compatible transport available
    #[error("transport failed: no compatible transport")]
    NoCompatibleTransport,

    /// Transport not allowed by policy
    #[error("transport failed: not allowed by policy")]
    NotAllowedByPolicy,

    /// Connection failed
    #[error("transport failed: connection failed")]
    ConnectionFailed,

    /// Connection timed out
    #[error("transport failed: connection timeout")]
    Timeout,

    /// Transport parameters missing
    #[error("transport failed: missing parameters")]
    MissingParameters,

    /// Transport disconnected
    #[error("transport failed: disconnected")]
    Disconnected,
}

/// Storage errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum StoreError {
    /// Record not found
    #[error("not found: {0}")]
    NotFound(String),

    /// Record already exists
    #[error("already exists: {0}")]
    AlreadyExists(String),

    /// Storage operation failed
    #[error("storage operation failed")]
    OperationFailed,

    /// Data corruption detected
    #[error("data corruption detected")]
    DataCorruption,

    /// Serialization error
    #[error("serialization error")]
    Serialization,
}

/// Policy errors.
/// Requirements: 11.1
#[derive(Debug, Error, Clone)]
pub enum PolicyError {
    /// Permission denied by policy
    #[error("policy violation: permission denied")]
    PermissionDenied,

    /// Time restriction violated
    #[error("policy violation: time restriction")]
    TimeRestriction,

    /// Operator not trusted
    #[error("policy violation: operator not trusted")]
    OperatorNotTrusted,

    /// Rate limit exceeded
    #[error("policy violation: rate limit exceeded")]
    RateLimited,
}

// ============================================================================
// Unified Core Error (Requirements: 11.1, 11.2)
// ============================================================================

/// Unified error type for ZRC Core operations.
/// Requirements: 11.1, 11.2
#[derive(Debug, Error, Clone)]
pub enum CoreError {
    /// Authentication error
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),

    /// Permission error
    #[error("permission error: {0}")]
    Permission(#[from] PermissionError),

    /// Ticket error
    #[error("ticket error: {0}")]
    Ticket(#[from] TicketError),

    /// Transport error (typed)
    #[error("transport error: {0}")]
    TransportTyped(#[from] TransportError),

    /// Store error (typed)
    #[error("store error: {0}")]
    StoreTyped(#[from] StoreError),

    /// Policy error
    #[error("policy error: {0}")]
    Policy(#[from] PolicyError),

    // -------------------------------------------------------------------------
    // Legacy variants for backward compatibility with existing code
    // -------------------------------------------------------------------------

    /// Decode/parse error (legacy)
    #[error("decode error: {0}")]
    Decode(String),

    /// Cryptographic operation failed (legacy)
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Policy denied (legacy)
    #[error("policy denied: {0}")]
    Denied(String),

    /// Not found (legacy)
    #[error("not found: {0}")]
    NotFound(String),

    /// Bad request (legacy)
    #[error("bad request: {0}")]
    BadRequest(String),

    // -------------------------------------------------------------------------
    // New structured error variants
    // -------------------------------------------------------------------------

    /// Invalid state transition
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Missing required field
    #[error("bad request: missing field")]
    MissingField,

    /// Invalid message format
    #[error("bad request: invalid message")]
    InvalidMessage,

    /// Operation timed out
    #[error("timeout: operation timed out")]
    Timeout,

    /// Operation was cancelled
    #[error("cancelled: operation cancelled")]
    Cancelled,

    /// Internal error (should not expose details to remote)
    #[error("internal error")]
    Internal,

    /// Session not found
    #[error("session not found")]
    SessionNotFound,

    /// Device is offline
    #[error("device offline")]
    DeviceOffline,
}


// ============================================================================
// Error Mapping to Wire Format (Requirements: 11.3, 11.4)
// ============================================================================

impl CoreError {
    /// Map internal error to wire-safe ErrorV1 message.
    /// Requirements: 11.3, 11.4
    ///
    /// This method converts internal errors to wire-safe messages that:
    /// - Do not expose internal implementation details
    /// - Provide sufficient information for debugging
    /// - Use standardized error codes
    pub fn to_error_v1(&self) -> ErrorV1 {
        let (code, message) = self.to_wire_safe();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        ErrorV1 {
            error_code: code as i32,
            error_message: message,
            details: HashMap::new(),
            timestamp,
        }
    }

    /// Map internal error to wire-safe ErrorV1 with additional details.
    /// Requirements: 11.3, 11.4
    ///
    /// # Arguments
    /// * `details` - Additional key-value pairs for debugging (must not contain sensitive data)
    pub fn to_error_v1_with_details(&self, details: HashMap<String, String>) -> ErrorV1 {
        let (code, message) = self.to_wire_safe();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        ErrorV1 {
            error_code: code as i32,
            error_message: message,
            details,
            timestamp,
        }
    }

    /// Convert to wire-safe error code and message.
    /// Requirements: 11.4 - Never expose internal implementation details
    fn to_wire_safe(&self) -> (ErrorCodeV1, String) {
        match self {
            // Authentication errors
            CoreError::Auth(auth_err) => match auth_err {
                AuthError::InvalidCredentials => (
                    ErrorCodeV1::AuthFailed,
                    "Authentication failed".to_string(),
                ),
                AuthError::SignatureInvalid => (
                    ErrorCodeV1::AuthFailed,
                    "Authentication failed".to_string(),
                ),
                AuthError::UnknownIdentity => (
                    ErrorCodeV1::AuthFailed,
                    "Authentication failed".to_string(),
                ),
                AuthError::PairingRequired => (
                    ErrorCodeV1::PairingRequired,
                    "Pairing required".to_string(),
                ),
                AuthError::InvalidInviteProof => (
                    ErrorCodeV1::AuthFailed,
                    "Authentication failed".to_string(),
                ),
                AuthError::InviteExpired => (
                    ErrorCodeV1::AuthFailed,
                    "Invite expired".to_string(),
                ),
            },

            // Permission errors
            CoreError::Permission(perm_err) => match perm_err {
                PermissionError::Denied(_) => (
                    ErrorCodeV1::PermissionDenied,
                    "Permission denied".to_string(),
                ),
                PermissionError::ExceedsPairedPermissions => (
                    ErrorCodeV1::PermissionDenied,
                    "Permission denied".to_string(),
                ),
                PermissionError::ExceedsPolicyLimits => (
                    ErrorCodeV1::PermissionDenied,
                    "Permission denied".to_string(),
                ),
                PermissionError::ConsentRequired => (
                    ErrorCodeV1::ConsentRequired,
                    "Consent required".to_string(),
                ),
                PermissionError::ConsentDenied => (
                    ErrorCodeV1::PermissionDenied,
                    "Consent denied".to_string(),
                ),
            },

            // Ticket errors
            CoreError::Ticket(ticket_err) => match ticket_err {
                TicketError::Expired => (
                    ErrorCodeV1::TicketExpired,
                    "Session ticket expired".to_string(),
                ),
                TicketError::Revoked => (
                    ErrorCodeV1::TicketExpired,
                    "Session ticket revoked".to_string(),
                ),
                TicketError::InvalidSignature => (
                    ErrorCodeV1::AuthFailed,
                    "Invalid ticket".to_string(),
                ),
                TicketError::BindingMismatch => (
                    ErrorCodeV1::AuthFailed,
                    "Invalid ticket".to_string(),
                ),
                TicketError::NotFound => (
                    ErrorCodeV1::SessionNotFound,
                    "Session not found".to_string(),
                ),
                TicketError::Malformed => (
                    ErrorCodeV1::InvalidMessage,
                    "Invalid ticket format".to_string(),
                ),
            },

            // Transport errors (typed)
            CoreError::TransportTyped(transport_err) => match transport_err {
                TransportError::NoCompatibleTransport => (
                    ErrorCodeV1::TransportFailed,
                    "No compatible transport".to_string(),
                ),
                TransportError::NotAllowedByPolicy => (
                    ErrorCodeV1::TransportFailed,
                    "Transport not allowed".to_string(),
                ),
                TransportError::ConnectionFailed => (
                    ErrorCodeV1::TransportFailed,
                    "Connection failed".to_string(),
                ),
                TransportError::Timeout => (
                    ErrorCodeV1::Timeout,
                    "Connection timeout".to_string(),
                ),
                TransportError::MissingParameters => (
                    ErrorCodeV1::TransportFailed,
                    "Transport configuration error".to_string(),
                ),
                TransportError::Disconnected => (
                    ErrorCodeV1::TransportFailed,
                    "Disconnected".to_string(),
                ),
            },

            // Store errors (typed) - map to generic errors to avoid exposing internals
            CoreError::StoreTyped(store_err) => match store_err {
                StoreError::NotFound(_) => (
                    ErrorCodeV1::SessionNotFound,
                    "Resource not found".to_string(),
                ),
                StoreError::AlreadyExists(_) => (
                    ErrorCodeV1::InternalError,
                    "Operation failed".to_string(),
                ),
                StoreError::OperationFailed => (
                    ErrorCodeV1::InternalError,
                    "Operation failed".to_string(),
                ),
                StoreError::DataCorruption => (
                    ErrorCodeV1::InternalError,
                    "Operation failed".to_string(),
                ),
                StoreError::Serialization => (
                    ErrorCodeV1::InternalError,
                    "Operation failed".to_string(),
                ),
            },

            // Policy errors
            CoreError::Policy(policy_err) => match policy_err {
                PolicyError::PermissionDenied => (
                    ErrorCodeV1::PermissionDenied,
                    "Permission denied by policy".to_string(),
                ),
                PolicyError::TimeRestriction => (
                    ErrorCodeV1::PermissionDenied,
                    "Access not allowed at this time".to_string(),
                ),
                PolicyError::OperatorNotTrusted => (
                    ErrorCodeV1::PermissionDenied,
                    "Operator not trusted".to_string(),
                ),
                PolicyError::RateLimited => (
                    ErrorCodeV1::RateLimited,
                    "Rate limit exceeded".to_string(),
                ),
            },

            // Legacy variants - map to appropriate wire codes
            CoreError::Decode(_) => (
                ErrorCodeV1::InvalidMessage,
                "Invalid message format".to_string(),
            ),
            CoreError::Crypto(_) => (
                ErrorCodeV1::InternalError,
                "Cryptographic operation failed".to_string(),
            ),
            CoreError::Denied(_) => (
                ErrorCodeV1::PermissionDenied,
                "Permission denied".to_string(),
            ),
            CoreError::NotFound(_) => (
                ErrorCodeV1::SessionNotFound,
                "Resource not found".to_string(),
            ),
            CoreError::BadRequest(_) => (
                ErrorCodeV1::InvalidMessage,
                "Invalid request".to_string(),
            ),

            // Other errors
            CoreError::InvalidState(_) => (
                ErrorCodeV1::InternalError,
                "Invalid operation".to_string(),
            ),
            CoreError::MissingField => (
                ErrorCodeV1::InvalidMessage,
                "Missing required field".to_string(),
            ),
            CoreError::InvalidMessage => (
                ErrorCodeV1::InvalidMessage,
                "Invalid message".to_string(),
            ),
            CoreError::Timeout => (
                ErrorCodeV1::Timeout,
                "Operation timed out".to_string(),
            ),
            CoreError::Cancelled => (
                ErrorCodeV1::Cancelled,
                "Operation cancelled".to_string(),
            ),
            CoreError::Internal => (
                ErrorCodeV1::InternalError,
                "Internal error".to_string(),
            ),
            CoreError::SessionNotFound => (
                ErrorCodeV1::SessionNotFound,
                "Session not found".to_string(),
            ),
            CoreError::DeviceOffline => (
                ErrorCodeV1::DeviceOffline,
                "Device is offline".to_string(),
            ),
        }
    }

    /// Get the error code for this error.
    pub fn error_code(&self) -> ErrorCodeV1 {
        self.to_wire_safe().0
    }

    /// Check if this error should be logged with detailed information.
    /// Requirements: 11.5 - Log detailed error information locally
    pub fn should_log_details(&self) -> bool {
        matches!(
            self,
            CoreError::Auth(_)
                | CoreError::Permission(_)
                | CoreError::Policy(_)
                | CoreError::StoreTyped(_)
                | CoreError::Internal
                | CoreError::Denied(_)
                | CoreError::Crypto(_)
        )
    }
}

// ============================================================================
// Conversion from ErrorV1 to CoreError
// ============================================================================

impl From<ErrorV1> for CoreError {
    fn from(error: ErrorV1) -> Self {
        match ErrorCodeV1::try_from(error.error_code).unwrap_or(ErrorCodeV1::Unspecified) {
            ErrorCodeV1::AuthFailed => CoreError::Auth(AuthError::InvalidCredentials),
            ErrorCodeV1::PermissionDenied => {
                CoreError::Permission(PermissionError::Denied(error.error_message))
            }
            ErrorCodeV1::TicketExpired => CoreError::Ticket(TicketError::Expired),
            ErrorCodeV1::RateLimited => CoreError::Policy(PolicyError::RateLimited),
            ErrorCodeV1::TransportFailed => {
                CoreError::TransportTyped(TransportError::ConnectionFailed)
            }
            ErrorCodeV1::InternalError => CoreError::Internal,
            ErrorCodeV1::InvalidMessage => CoreError::InvalidMessage,
            ErrorCodeV1::SessionNotFound => CoreError::SessionNotFound,
            ErrorCodeV1::DeviceOffline => CoreError::DeviceOffline,
            ErrorCodeV1::PairingRequired => CoreError::Auth(AuthError::PairingRequired),
            ErrorCodeV1::ConsentRequired => {
                CoreError::Permission(PermissionError::ConsentRequired)
            }
            ErrorCodeV1::Timeout => CoreError::Timeout,
            ErrorCodeV1::Cancelled => CoreError::Cancelled,
            ErrorCodeV1::Unspecified => CoreError::Internal,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an ErrorV1 from an error code and message.
pub fn make_error_v1(code: ErrorCodeV1, message: impl Into<String>) -> ErrorV1 {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    ErrorV1 {
        error_code: code as i32,
        error_message: message.into(),
        details: HashMap::new(),
        timestamp,
    }
}

/// Create an ErrorV1 with details.
pub fn make_error_v1_with_details(
    code: ErrorCodeV1,
    message: impl Into<String>,
    details: HashMap<String, String>,
) -> ErrorV1 {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    ErrorV1 {
        error_code: code as i32,
        error_message: message.into(),
        details,
        timestamp,
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_to_wire() {
        let err = CoreError::Auth(AuthError::InvalidCredentials);
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::AuthFailed as i32);
        assert_eq!(wire.error_message, "Authentication failed");
        // Should not expose internal details
        assert!(!wire.error_message.contains("credentials"));
    }

    #[test]
    fn test_permission_error_to_wire() {
        let err = CoreError::Permission(PermissionError::Denied("internal reason".to_string()));
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::PermissionDenied as i32);
        // Should not expose internal reason
        assert!(!wire.error_message.contains("internal reason"));
    }

    #[test]
    fn test_ticket_expired_to_wire() {
        let err = CoreError::Ticket(TicketError::Expired);
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::TicketExpired as i32);
    }

    #[test]
    fn test_store_error_hides_details() {
        let err = CoreError::StoreTyped(StoreError::NotFound("secret_table".to_string()));
        let wire = err.to_error_v1();
        // Should not expose internal table name
        assert!(!wire.error_message.contains("secret_table"));
        assert_eq!(wire.error_code, ErrorCodeV1::SessionNotFound as i32);
    }

    #[test]
    fn test_policy_rate_limited() {
        let err = CoreError::Policy(PolicyError::RateLimited);
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::RateLimited as i32);
    }

    #[test]
    fn test_transport_timeout() {
        let err = CoreError::TransportTyped(TransportError::Timeout);
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::Timeout as i32);
    }

    #[test]
    fn test_error_v1_roundtrip() {
        let original = CoreError::Auth(AuthError::PairingRequired);
        let wire = original.to_error_v1();
        let recovered = CoreError::from(wire);
        
        // Should recover to the same error type
        assert!(matches!(recovered, CoreError::Auth(AuthError::PairingRequired)));
    }

    #[test]
    fn test_make_error_v1() {
        let err = make_error_v1(ErrorCodeV1::AuthFailed, "Test error");
        assert_eq!(err.error_code, ErrorCodeV1::AuthFailed as i32);
        assert_eq!(err.error_message, "Test error");
        assert!(err.timestamp > 0);
    }

    #[test]
    fn test_make_error_v1_with_details() {
        let mut details = HashMap::new();
        details.insert("key".to_string(), "value".to_string());
        
        let err = make_error_v1_with_details(
            ErrorCodeV1::InvalidMessage,
            "Test error",
            details,
        );
        
        assert_eq!(err.error_code, ErrorCodeV1::InvalidMessage as i32);
        assert_eq!(err.details.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_error_code_getter() {
        let err = CoreError::Timeout;
        assert_eq!(err.error_code(), ErrorCodeV1::Timeout);
    }

    #[test]
    fn test_should_log_details() {
        // Auth errors should be logged with details
        assert!(CoreError::Auth(AuthError::InvalidCredentials).should_log_details());
        
        // Permission errors should be logged with details
        assert!(CoreError::Permission(PermissionError::Denied("test".into())).should_log_details());
        
        // Timeout errors don't need detailed logging
        assert!(!CoreError::Timeout.should_log_details());
    }

    #[test]
    fn test_consent_required_error() {
        let err = CoreError::Permission(PermissionError::ConsentRequired);
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::ConsentRequired as i32);
    }

    #[test]
    fn test_device_offline_error() {
        let err = CoreError::DeviceOffline;
        let wire = err.to_error_v1();
        assert_eq!(wire.error_code, ErrorCodeV1::DeviceOffline as i32);
    }

    #[test]
    fn test_internal_error_hides_state() {
        let err = CoreError::InvalidState("internal state machine details".to_string());
        let wire = err.to_error_v1();
        // Should not expose internal state details
        assert!(!wire.error_message.contains("internal state machine"));
        assert_eq!(wire.error_code, ErrorCodeV1::InternalError as i32);
    }
}
