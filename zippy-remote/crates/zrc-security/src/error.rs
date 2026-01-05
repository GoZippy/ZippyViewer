//! Security error types.

use thiserror::Error;

/// Security-related errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityError {
    #[error("unknown peer: {peer_id:?}")]
    UnknownPeer { peer_id: Vec<u8> },

    #[error("identity mismatch for peer {peer_id:?}: expected {expected}, received {received}")]
    IdentityMismatch {
        peer_id: Vec<u8>,
        expected: String,
        received: String,
    },

    #[error("replay detected: sequence {sequence}")]
    ReplayDetected { sequence: u64 },

    #[error("invalid sequence number")]
    InvalidSequence,

    #[error("ticket expired")]
    TicketExpired,

    #[error("ticket from future")]
    TicketFromFuture,

    #[error("rate limited: operation {operation}, retry after {retry_after:?}")]
    RateLimited {
        operation: String,
        retry_after: std::time::Duration,
    },

    #[error("audit log error: {0}")]
    AuditError(String),

    #[error("downgrade detected: {algorithm}")]
    DowngradeDetected { algorithm: String },

    #[error("unsupported algorithm: {algorithm}")]
    UnsupportedAlgorithm { algorithm: String },

    #[error("key rotation failed: {reason}")]
    KeyRotationFailed { reason: String },

    #[error("invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },
}
