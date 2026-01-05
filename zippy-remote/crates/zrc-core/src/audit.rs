//! Audit event generation for security monitoring.
//!
//! Implements audit logging as specified in Requirements 9.1-9.7.
//!
//! Features:
//! - Comprehensive audit events for pairing and session lifecycle
//! - Pluggable sinks (memory buffer, file)
//! - Event signing with device key for non-repudiation
//! - Sensitive data exclusion

use async_trait::async_trait;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey, Verifier};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

/// Errors from audit operations.
#[derive(Debug, Error)]
pub enum AuditError {
    #[error("sink error: {0}")]
    SinkError(String),
    #[error("signing error: {0}")]
    SigningError(String),
    #[error("io error: {0}")]
    IoError(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl From<std::io::Error> for AuditError {
    fn from(e: std::io::Error) -> Self {
        AuditError::IoError(e.to_string())
    }
}

/// Reason for session ending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEndReason {
    /// User requested disconnect.
    UserRequested,
    /// Ticket expired.
    TicketExpired,
    /// Transport disconnected.
    TransportDisconnected,
    /// Policy violation.
    PolicyViolation,
    /// Error occurred.
    Error(String),
}

impl std::fmt::Display for SessionEndReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionEndReason::UserRequested => write!(f, "user_requested"),
            SessionEndReason::TicketExpired => write!(f, "ticket_expired"),
            SessionEndReason::TransportDisconnected => write!(f, "transport_disconnected"),
            SessionEndReason::PolicyViolation => write!(f, "policy_violation"),
            SessionEndReason::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}


/// Audit events for security monitoring.
///
/// All events include:
/// - timestamp: Unix timestamp in seconds
/// - event_type: String identifier for the event type
/// - device_id: The device where the event occurred
/// - operator_id: The operator involved (where applicable)
/// - details: Event-specific information
///
/// Note: Sensitive data (keys, secrets, tokens) is NEVER included in audit events.
#[derive(Debug, Clone)]
pub enum AuditEvent {
    // Pairing events (Requirements 9.1)
    PairRequestReceived {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        timestamp: u64,
    },
    PairApproved {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        permissions: u32,
        timestamp: u64,
    },
    PairDenied {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        reason: String,
        timestamp: u64,
    },
    PairRevoked {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        timestamp: u64,
    },

    // Session events (Requirements 9.2)
    SessionRequested {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        session_id: [u8; 32],
        timestamp: u64,
    },
    SessionStarted {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        session_id: [u8; 32],
        permissions: u32,
        timestamp: u64,
    },
    SessionEnded {
        device_id: [u8; 32],
        session_id: [u8; 32],
        reason: SessionEndReason,
        duration_seconds: u64,
        timestamp: u64,
    },
    SessionDenied {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        reason: String,
        timestamp: u64,
    },

    // Security events (Requirements 9.3)
    PermissionEscalationAttempted {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        requested_permissions: u32,
        allowed_permissions: u32,
        timestamp: u64,
    },
    PolicyViolation {
        device_id: [u8; 32],
        operator_id: [u8; 32],
        violation: String,
        timestamp: u64,
    },
    RateLimitExceeded {
        device_id: [u8; 32],
        source: String,
        limit_type: String,
        timestamp: u64,
    },
}


impl AuditEvent {
    /// Get the timestamp of the event.
    pub fn timestamp(&self) -> u64 {
        match self {
            AuditEvent::PairRequestReceived { timestamp, .. } => *timestamp,
            AuditEvent::PairApproved { timestamp, .. } => *timestamp,
            AuditEvent::PairDenied { timestamp, .. } => *timestamp,
            AuditEvent::PairRevoked { timestamp, .. } => *timestamp,
            AuditEvent::SessionRequested { timestamp, .. } => *timestamp,
            AuditEvent::SessionStarted { timestamp, .. } => *timestamp,
            AuditEvent::SessionEnded { timestamp, .. } => *timestamp,
            AuditEvent::SessionDenied { timestamp, .. } => *timestamp,
            AuditEvent::PermissionEscalationAttempted { timestamp, .. } => *timestamp,
            AuditEvent::PolicyViolation { timestamp, .. } => *timestamp,
            AuditEvent::RateLimitExceeded { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::PairRequestReceived { .. } => "PAIR_REQUEST_RECEIVED",
            AuditEvent::PairApproved { .. } => "PAIR_APPROVED",
            AuditEvent::PairDenied { .. } => "PAIR_DENIED",
            AuditEvent::PairRevoked { .. } => "PAIR_REVOKED",
            AuditEvent::SessionRequested { .. } => "SESSION_REQUESTED",
            AuditEvent::SessionStarted { .. } => "SESSION_STARTED",
            AuditEvent::SessionEnded { .. } => "SESSION_ENDED",
            AuditEvent::SessionDenied { .. } => "SESSION_DENIED",
            AuditEvent::PermissionEscalationAttempted { .. } => "PERMISSION_ESCALATION_ATTEMPTED",
            AuditEvent::PolicyViolation { .. } => "POLICY_VIOLATION",
            AuditEvent::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
        }
    }

    /// Get the device ID for this event.
    pub fn device_id(&self) -> &[u8; 32] {
        match self {
            AuditEvent::PairRequestReceived { device_id, .. } => device_id,
            AuditEvent::PairApproved { device_id, .. } => device_id,
            AuditEvent::PairDenied { device_id, .. } => device_id,
            AuditEvent::PairRevoked { device_id, .. } => device_id,
            AuditEvent::SessionRequested { device_id, .. } => device_id,
            AuditEvent::SessionStarted { device_id, .. } => device_id,
            AuditEvent::SessionEnded { device_id, .. } => device_id,
            AuditEvent::SessionDenied { device_id, .. } => device_id,
            AuditEvent::PermissionEscalationAttempted { device_id, .. } => device_id,
            AuditEvent::PolicyViolation { device_id, .. } => device_id,
            AuditEvent::RateLimitExceeded { device_id, .. } => device_id,
        }
    }

    /// Get the operator ID for this event (if applicable).
    pub fn operator_id(&self) -> Option<&[u8; 32]> {
        match self {
            AuditEvent::PairRequestReceived { operator_id, .. } => Some(operator_id),
            AuditEvent::PairApproved { operator_id, .. } => Some(operator_id),
            AuditEvent::PairDenied { operator_id, .. } => Some(operator_id),
            AuditEvent::PairRevoked { operator_id, .. } => Some(operator_id),
            AuditEvent::SessionRequested { operator_id, .. } => Some(operator_id),
            AuditEvent::SessionStarted { operator_id, .. } => Some(operator_id),
            AuditEvent::SessionEnded { .. } => None,
            AuditEvent::SessionDenied { operator_id, .. } => Some(operator_id),
            AuditEvent::PermissionEscalationAttempted { operator_id, .. } => Some(operator_id),
            AuditEvent::PolicyViolation { operator_id, .. } => Some(operator_id),
            AuditEvent::RateLimitExceeded { .. } => None,
        }
    }

    /// Serialize the event to a canonical byte representation for signing.
    /// This excludes any sensitive data and produces a deterministic output.
    pub fn to_signable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Event type prefix
        bytes.extend_from_slice(self.event_type().as_bytes());
        bytes.push(b'|');
        
        // Timestamp
        bytes.extend_from_slice(&self.timestamp().to_be_bytes());
        bytes.push(b'|');
        
        // Device ID
        bytes.extend_from_slice(self.device_id());
        bytes.push(b'|');
        
        // Operator ID (if present)
        if let Some(op_id) = self.operator_id() {
            bytes.extend_from_slice(op_id);
        }
        bytes.push(b'|');
        
        // Event-specific details (non-sensitive only)
        match self {
            AuditEvent::PairApproved { permissions, .. } => {
                bytes.extend_from_slice(&permissions.to_be_bytes());
            }
            AuditEvent::PairDenied { reason, .. } => {
                bytes.extend_from_slice(reason.as_bytes());
            }
            AuditEvent::SessionRequested { session_id, .. } => {
                bytes.extend_from_slice(session_id);
            }
            AuditEvent::SessionStarted { session_id, permissions, .. } => {
                bytes.extend_from_slice(session_id);
                bytes.extend_from_slice(&permissions.to_be_bytes());
            }
            AuditEvent::SessionEnded { session_id, reason, duration_seconds, .. } => {
                bytes.extend_from_slice(session_id);
                bytes.extend_from_slice(reason.to_string().as_bytes());
                bytes.extend_from_slice(&duration_seconds.to_be_bytes());
            }
            AuditEvent::SessionDenied { reason, .. } => {
                bytes.extend_from_slice(reason.as_bytes());
            }
            AuditEvent::PermissionEscalationAttempted { 
                requested_permissions, 
                allowed_permissions, 
                .. 
            } => {
                bytes.extend_from_slice(&requested_permissions.to_be_bytes());
                bytes.extend_from_slice(&allowed_permissions.to_be_bytes());
            }
            AuditEvent::PolicyViolation { violation, .. } => {
                bytes.extend_from_slice(violation.as_bytes());
            }
            AuditEvent::RateLimitExceeded { source, limit_type, .. } => {
                bytes.extend_from_slice(source.as_bytes());
                bytes.extend_from_slice(limit_type.as_bytes());
            }
            _ => {}
        }
        
        bytes
    }

    /// Format the event as a human-readable log line.
    pub fn to_log_line(&self) -> String {
        let device_hex = hex::encode(&self.device_id()[..8]);
        let op_hex = self.operator_id()
            .map(|id| hex::encode(&id[..8]))
            .unwrap_or_else(|| "N/A".to_string());
        
        match self {
            AuditEvent::PairRequestReceived { timestamp, .. } => {
                format!("[{}] {} device={} operator={}", 
                    timestamp, self.event_type(), device_hex, op_hex)
            }
            AuditEvent::PairApproved { permissions, timestamp, .. } => {
                format!("[{}] {} device={} operator={} permissions=0x{:08x}", 
                    timestamp, self.event_type(), device_hex, op_hex, permissions)
            }
            AuditEvent::PairDenied { reason, timestamp, .. } => {
                format!("[{}] {} device={} operator={} reason=\"{}\"", 
                    timestamp, self.event_type(), device_hex, op_hex, reason)
            }
            AuditEvent::PairRevoked { timestamp, .. } => {
                format!("[{}] {} device={} operator={}", 
                    timestamp, self.event_type(), device_hex, op_hex)
            }
            AuditEvent::SessionRequested { session_id, timestamp, .. } => {
                format!("[{}] {} device={} operator={} session={}", 
                    timestamp, self.event_type(), device_hex, op_hex, 
                    hex::encode(&session_id[..8]))
            }
            AuditEvent::SessionStarted { session_id, permissions, timestamp, .. } => {
                format!("[{}] {} device={} operator={} session={} permissions=0x{:08x}", 
                    timestamp, self.event_type(), device_hex, op_hex, 
                    hex::encode(&session_id[..8]), permissions)
            }
            AuditEvent::SessionEnded { session_id, reason, duration_seconds, timestamp, .. } => {
                format!("[{}] {} device={} session={} reason=\"{}\" duration={}s", 
                    timestamp, self.event_type(), device_hex, 
                    hex::encode(&session_id[..8]), reason, duration_seconds)
            }
            AuditEvent::SessionDenied { reason, timestamp, .. } => {
                format!("[{}] {} device={} operator={} reason=\"{}\"", 
                    timestamp, self.event_type(), device_hex, op_hex, reason)
            }
            AuditEvent::PermissionEscalationAttempted { 
                requested_permissions, 
                allowed_permissions, 
                timestamp, 
                .. 
            } => {
                format!("[{}] {} device={} operator={} requested=0x{:08x} allowed=0x{:08x}", 
                    timestamp, self.event_type(), device_hex, op_hex, 
                    requested_permissions, allowed_permissions)
            }
            AuditEvent::PolicyViolation { violation, timestamp, .. } => {
                format!("[{}] {} device={} operator={} violation=\"{}\"", 
                    timestamp, self.event_type(), device_hex, op_hex, violation)
            }
            AuditEvent::RateLimitExceeded { source, limit_type, timestamp, .. } => {
                format!("[{}] {} device={} source=\"{}\" type=\"{}\"", 
                    timestamp, self.event_type(), device_hex, source, limit_type)
            }
        }
    }
}


/// A signed audit event with cryptographic non-repudiation.
#[derive(Debug, Clone)]
pub struct SignedAuditEvent {
    /// The audit event.
    pub event: AuditEvent,
    /// Ed25519 signature over the event's signable bytes.
    pub signature: [u8; 64],
    /// The public key that signed this event.
    pub signer_pub: [u8; 32],
}

impl SignedAuditEvent {
    /// Create a new signed audit event.
    pub fn sign(event: AuditEvent, signing_key: &SigningKey) -> Result<Self, AuditError> {
        let signable = event.to_signable_bytes();
        let signature: Signature = signing_key.sign(&signable);
        
        Ok(Self {
            event,
            signature: signature.to_bytes(),
            signer_pub: signing_key.verifying_key().to_bytes(),
        })
    }

    /// Verify the signature on this event.
    pub fn verify(&self) -> Result<bool, AuditError> {
        let signable = self.event.to_signable_bytes();
        let verifying_key = VerifyingKey::from_bytes(&self.signer_pub)
            .map_err(|e| AuditError::SigningError(e.to_string()))?;
        let signature = Signature::from_bytes(&self.signature);
        
        Ok(verifying_key.verify(&signable, &signature).is_ok())
    }

    /// Format as a log line with signature info.
    pub fn to_log_line(&self) -> String {
        format!("{} sig={}", 
            self.event.to_log_line(), 
            hex::encode(&self.signature[..8]))
    }
}

/// Trait for audit event sinks.
#[async_trait]
pub trait AuditSink: Send + Sync {
    /// Emit an audit event to the sink.
    async fn emit(&self, event: AuditEvent) -> Result<(), AuditError>;
    
    /// Emit a signed audit event to the sink.
    async fn emit_signed(&self, event: SignedAuditEvent) -> Result<(), AuditError>;
}

/// In-memory audit sink for testing and buffering.
#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    events: RwLock<Vec<AuditEvent>>,
    signed_events: RwLock<Vec<SignedAuditEvent>>,
    max_events: usize,
}

impl MemoryAuditSink {
    /// Create a new memory sink with the given capacity.
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(Vec::with_capacity(max_events)),
            signed_events: RwLock::new(Vec::with_capacity(max_events)),
            max_events,
        }
    }

    /// Get all stored unsigned events.
    pub async fn events(&self) -> Vec<AuditEvent> {
        self.events.read().await.clone()
    }

    /// Get all stored signed events.
    pub async fn signed_events(&self) -> Vec<SignedAuditEvent> {
        self.signed_events.read().await.clone()
    }

    /// Clear all stored events.
    pub async fn clear(&self) {
        self.events.write().await.clear();
        self.signed_events.write().await.clear();
    }

    /// Get the count of all events (signed + unsigned).
    pub async fn count(&self) -> usize {
        self.events.read().await.len() + self.signed_events.read().await.len()
    }
}

#[async_trait]
impl AuditSink for MemoryAuditSink {
    async fn emit(&self, event: AuditEvent) -> Result<(), AuditError> {
        let mut events = self.events.write().await;
        if events.len() >= self.max_events {
            events.remove(0); // Remove oldest event
        }
        events.push(event);
        Ok(())
    }

    async fn emit_signed(&self, event: SignedAuditEvent) -> Result<(), AuditError> {
        let mut events = self.signed_events.write().await;
        if events.len() >= self.max_events {
            events.remove(0); // Remove oldest event
        }
        events.push(event);
        Ok(())
    }
}


/// File-based audit sink for persistent logging.
pub struct FileAuditSink {
    path: std::path::PathBuf,
    sign_events: bool,
    signing_key: Option<SigningKey>,
}

impl FileAuditSink {
    /// Create a new file sink that writes to the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            sign_events: false,
            signing_key: None,
        }
    }

    /// Create a new file sink with event signing enabled.
    pub fn with_signing<P: AsRef<Path>>(path: P, signing_key: SigningKey) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            sign_events: true,
            signing_key: Some(signing_key),
        }
    }

    /// Append a line to the audit log file.
    async fn append_line(&self, line: &str) -> Result<(), AuditError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        
        Ok(())
    }
}

impl std::fmt::Debug for FileAuditSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileAuditSink")
            .field("path", &self.path)
            .field("sign_events", &self.sign_events)
            .finish()
    }
}

#[async_trait]
impl AuditSink for FileAuditSink {
    async fn emit(&self, event: AuditEvent) -> Result<(), AuditError> {
        if self.sign_events {
            if let Some(ref key) = self.signing_key {
                let signed = SignedAuditEvent::sign(event, key)?;
                self.append_line(&signed.to_log_line()).await
            } else {
                self.append_line(&event.to_log_line()).await
            }
        } else {
            self.append_line(&event.to_log_line()).await
        }
    }

    async fn emit_signed(&self, event: SignedAuditEvent) -> Result<(), AuditError> {
        self.append_line(&event.to_log_line()).await
    }
}

/// Audit logger that dispatches events to multiple sinks.
pub struct AuditLogger {
    sinks: Vec<Arc<dyn AuditSink>>,
    device_id: [u8; 32],
    signing_key: Option<SigningKey>,
}

impl AuditLogger {
    /// Create a new audit logger with no sinks.
    pub fn new(device_id: [u8; 32]) -> Self {
        Self { 
            sinks: Vec::new(),
            device_id,
            signing_key: None,
        }
    }

    /// Create a new audit logger with signing enabled.
    pub fn with_signing(device_id: [u8; 32], signing_key: SigningKey) -> Self {
        Self {
            sinks: Vec::new(),
            device_id,
            signing_key: Some(signing_key),
        }
    }

    /// Add a sink to the logger.
    pub fn add_sink(&mut self, sink: Arc<dyn AuditSink>) {
        self.sinks.push(sink);
    }

    /// Get the device ID for this logger.
    pub fn device_id(&self) -> &[u8; 32] {
        &self.device_id
    }

    /// Emit an event to all sinks.
    /// If signing is enabled, the event will be signed before emission.
    pub async fn emit(&self, event: AuditEvent) -> Result<(), AuditError> {
        if let Some(ref key) = self.signing_key {
            let signed = SignedAuditEvent::sign(event, key)?;
            for sink in &self.sinks {
                sink.emit_signed(signed.clone()).await?;
            }
        } else {
            for sink in &self.sinks {
                sink.emit(event.clone()).await?;
            }
        }
        Ok(())
    }

    /// Emit a pre-signed event to all sinks.
    pub async fn emit_signed(&self, event: SignedAuditEvent) -> Result<(), AuditError> {
        for sink in &self.sinks {
            sink.emit_signed(event.clone()).await?;
        }
        Ok(())
    }

    // Convenience methods for emitting specific events

    /// Emit a pair request received event.
    pub async fn pair_request_received(&self, operator_id: [u8; 32]) -> Result<(), AuditError> {
        self.emit(AuditEvent::PairRequestReceived {
            device_id: self.device_id,
            operator_id,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a pair approved event.
    pub async fn pair_approved(&self, operator_id: [u8; 32], permissions: u32) -> Result<(), AuditError> {
        self.emit(AuditEvent::PairApproved {
            device_id: self.device_id,
            operator_id,
            permissions,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a pair denied event.
    pub async fn pair_denied(&self, operator_id: [u8; 32], reason: &str) -> Result<(), AuditError> {
        self.emit(AuditEvent::PairDenied {
            device_id: self.device_id,
            operator_id,
            reason: reason.to_string(),
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a pair revoked event.
    pub async fn pair_revoked(&self, operator_id: [u8; 32]) -> Result<(), AuditError> {
        self.emit(AuditEvent::PairRevoked {
            device_id: self.device_id,
            operator_id,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a session requested event.
    pub async fn session_requested(&self, operator_id: [u8; 32], session_id: [u8; 32]) -> Result<(), AuditError> {
        self.emit(AuditEvent::SessionRequested {
            device_id: self.device_id,
            operator_id,
            session_id,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a session started event.
    pub async fn session_started(
        &self, 
        operator_id: [u8; 32], 
        session_id: [u8; 32], 
        permissions: u32
    ) -> Result<(), AuditError> {
        self.emit(AuditEvent::SessionStarted {
            device_id: self.device_id,
            operator_id,
            session_id,
            permissions,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a session ended event.
    pub async fn session_ended(
        &self, 
        session_id: [u8; 32], 
        reason: SessionEndReason,
        duration_seconds: u64
    ) -> Result<(), AuditError> {
        self.emit(AuditEvent::SessionEnded {
            device_id: self.device_id,
            session_id,
            reason,
            duration_seconds,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a session denied event.
    pub async fn session_denied(&self, operator_id: [u8; 32], reason: &str) -> Result<(), AuditError> {
        self.emit(AuditEvent::SessionDenied {
            device_id: self.device_id,
            operator_id,
            reason: reason.to_string(),
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a permission escalation attempted event.
    pub async fn permission_escalation_attempted(
        &self,
        operator_id: [u8; 32],
        requested_permissions: u32,
        allowed_permissions: u32,
    ) -> Result<(), AuditError> {
        self.emit(AuditEvent::PermissionEscalationAttempted {
            device_id: self.device_id,
            operator_id,
            requested_permissions,
            allowed_permissions,
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a policy violation event.
    pub async fn policy_violation(&self, operator_id: [u8; 32], violation: &str) -> Result<(), AuditError> {
        self.emit(AuditEvent::PolicyViolation {
            device_id: self.device_id,
            operator_id,
            violation: violation.to_string(),
            timestamp: current_timestamp(),
        }).await
    }

    /// Emit a rate limit exceeded event.
    pub async fn rate_limit_exceeded(&self, source: &str, limit_type: &str) -> Result<(), AuditError> {
        self.emit(AuditEvent::RateLimitExceeded {
            device_id: self.device_id,
            source: source.to_string(),
            limit_type: limit_type.to_string(),
            timestamp: current_timestamp(),
        }).await
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new([0u8; 32])
    }
}

/// Get the current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}


#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn test_device_id() -> [u8; 32] {
        [1u8; 32]
    }

    fn test_operator_id() -> [u8; 32] {
        [2u8; 32]
    }

    fn test_session_id() -> [u8; 32] {
        [3u8; 32]
    }

    #[tokio::test]
    async fn test_memory_sink() {
        let sink = MemoryAuditSink::new(100);
        let event = AuditEvent::PairRequestReceived {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            timestamp: 1234567890,
        };

        sink.emit(event.clone()).await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "PAIR_REQUEST_RECEIVED");
    }

    #[tokio::test]
    async fn test_memory_sink_capacity() {
        let sink = MemoryAuditSink::new(2);

        for i in 0..3 {
            let event = AuditEvent::PairRequestReceived {
                device_id: test_device_id(),
                operator_id: [i as u8; 32],
                timestamp: i as u64,
            };
            sink.emit(event).await.unwrap();
        }

        let events = sink.events().await;
        assert_eq!(events.len(), 2);
        // Oldest event should be removed
        assert_eq!(events[0].timestamp(), 1);
        assert_eq!(events[1].timestamp(), 2);
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(sink.clone());

        let event = AuditEvent::SessionStarted {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            session_id: test_session_id(),
            permissions: 0b0111,
            timestamp: 9876543210,
        };

        logger.emit(event).await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "SESSION_STARTED");
    }

    #[tokio::test]
    async fn test_audit_logger_convenience_methods() {
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(sink.clone());

        logger.pair_request_received(test_operator_id()).await.unwrap();
        logger.pair_approved(test_operator_id(), 0x07).await.unwrap();
        logger.session_requested(test_operator_id(), test_session_id()).await.unwrap();
        logger.session_started(test_operator_id(), test_session_id(), 0x07).await.unwrap();
        logger.session_ended(test_session_id(), SessionEndReason::UserRequested, 300).await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].event_type(), "PAIR_REQUEST_RECEIVED");
        assert_eq!(events[1].event_type(), "PAIR_APPROVED");
        assert_eq!(events[2].event_type(), "SESSION_REQUESTED");
        assert_eq!(events[3].event_type(), "SESSION_STARTED");
        assert_eq!(events[4].event_type(), "SESSION_ENDED");
    }

    #[tokio::test]
    async fn test_signed_audit_event() {
        let signing_key = SigningKey::generate(&mut OsRng);
        
        let event = AuditEvent::PairApproved {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            permissions: 0x07,
            timestamp: 1234567890,
        };

        let signed = SignedAuditEvent::sign(event, &signing_key).unwrap();
        
        // Verify the signature
        assert!(signed.verify().unwrap());
        
        // Check signer public key matches
        assert_eq!(signed.signer_pub, signing_key.verifying_key().to_bytes());
    }

    #[tokio::test]
    async fn test_signed_audit_event_tamper_detection() {
        let signing_key = SigningKey::generate(&mut OsRng);
        
        let event = AuditEvent::PairApproved {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            permissions: 0x07,
            timestamp: 1234567890,
        };

        let mut signed = SignedAuditEvent::sign(event, &signing_key).unwrap();
        
        // Tamper with the event
        signed.event = AuditEvent::PairApproved {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            permissions: 0xFF, // Changed permissions
            timestamp: 1234567890,
        };
        
        // Verification should fail
        assert!(!signed.verify().unwrap());
    }

    #[tokio::test]
    async fn test_audit_logger_with_signing() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::with_signing(test_device_id(), signing_key);
        logger.add_sink(sink.clone());

        logger.pair_approved(test_operator_id(), 0x07).await.unwrap();

        // Events should be in signed_events, not events
        let events = sink.events().await;
        let signed_events = sink.signed_events().await;
        
        assert_eq!(events.len(), 0);
        assert_eq!(signed_events.len(), 1);
        assert!(signed_events[0].verify().unwrap());
    }

    #[tokio::test]
    async fn test_event_to_log_line() {
        let event = AuditEvent::SessionStarted {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            session_id: test_session_id(),
            permissions: 0x07,
            timestamp: 1234567890,
        };

        let log_line = event.to_log_line();
        assert!(log_line.contains("SESSION_STARTED"));
        assert!(log_line.contains("1234567890"));
        assert!(log_line.contains("0x00000007"));
    }

    #[tokio::test]
    async fn test_permission_escalation_event() {
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(sink.clone());

        logger.permission_escalation_attempted(
            test_operator_id(),
            0xFF, // requested
            0x07, // allowed
        ).await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "PERMISSION_ESCALATION_ATTEMPTED");
    }

    #[tokio::test]
    async fn test_policy_violation_event() {
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(sink.clone());

        logger.policy_violation(test_operator_id(), "time_restriction_violated").await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "POLICY_VIOLATION");
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded_event() {
        let sink = Arc::new(MemoryAuditSink::new(100));
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(sink.clone());

        logger.rate_limit_exceeded("192.168.1.1", "pairing_attempts").await.unwrap();

        let events = sink.events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "RATE_LIMIT_EXCEEDED");
    }

    #[tokio::test]
    async fn test_event_device_id_accessor() {
        let device_id = test_device_id();
        let event = AuditEvent::PairRequestReceived {
            device_id,
            operator_id: test_operator_id(),
            timestamp: 1234567890,
        };

        assert_eq!(event.device_id(), &device_id);
    }

    #[tokio::test]
    async fn test_event_operator_id_accessor() {
        let operator_id = test_operator_id();
        let event = AuditEvent::PairRequestReceived {
            device_id: test_device_id(),
            operator_id,
            timestamp: 1234567890,
        };

        assert_eq!(event.operator_id(), Some(&operator_id));

        // SessionEnded doesn't have operator_id
        let event2 = AuditEvent::SessionEnded {
            device_id: test_device_id(),
            session_id: test_session_id(),
            reason: SessionEndReason::UserRequested,
            duration_seconds: 100,
            timestamp: 1234567890,
        };

        assert_eq!(event2.operator_id(), None);
    }

    #[tokio::test]
    async fn test_signable_bytes_deterministic() {
        let event = AuditEvent::PairApproved {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            permissions: 0x07,
            timestamp: 1234567890,
        };

        let bytes1 = event.to_signable_bytes();
        let bytes2 = event.to_signable_bytes();

        assert_eq!(bytes1, bytes2);
    }

    #[tokio::test]
    async fn test_session_end_reason_display() {
        assert_eq!(SessionEndReason::UserRequested.to_string(), "user_requested");
        assert_eq!(SessionEndReason::TicketExpired.to_string(), "ticket_expired");
        assert_eq!(SessionEndReason::TransportDisconnected.to_string(), "transport_disconnected");
        assert_eq!(SessionEndReason::PolicyViolation.to_string(), "policy_violation");
        assert_eq!(SessionEndReason::Error("test".to_string()).to_string(), "error: test");
    }

    #[tokio::test]
    async fn test_file_sink() {
        use tokio::fs;

        // Create a temp file path
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("zrc_audit_test_{}.log", std::process::id()));

        // Clean up any existing file
        let _ = fs::remove_file(&file_path).await;

        let sink = FileAuditSink::new(&file_path);
        
        let event = AuditEvent::PairRequestReceived {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            timestamp: 1234567890,
        };

        sink.emit(event).await.unwrap();

        // Read the file and verify content
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("PAIR_REQUEST_RECEIVED"));
        assert!(content.contains("1234567890"));

        // Clean up
        let _ = fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_file_sink_with_signing() {
        use tokio::fs;

        let signing_key = SigningKey::generate(&mut OsRng);

        // Create a temp file path
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("zrc_audit_test_signed_{}.log", std::process::id()));

        // Clean up any existing file
        let _ = fs::remove_file(&file_path).await;

        let sink = FileAuditSink::with_signing(&file_path, signing_key);
        
        let event = AuditEvent::PairApproved {
            device_id: test_device_id(),
            operator_id: test_operator_id(),
            permissions: 0x07,
            timestamp: 1234567890,
        };

        sink.emit(event).await.unwrap();

        // Read the file and verify content includes signature
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("PAIR_APPROVED"));
        assert!(content.contains("sig=")); // Signature should be present

        // Clean up
        let _ = fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_multiple_sinks() {
        let memory_sink = Arc::new(MemoryAuditSink::new(100));
        
        let mut logger = AuditLogger::new(test_device_id());
        logger.add_sink(memory_sink.clone());

        // Emit multiple events
        logger.pair_request_received(test_operator_id()).await.unwrap();
        logger.pair_approved(test_operator_id(), 0x07).await.unwrap();

        // Both events should be in memory sink
        let events = memory_sink.events().await;
        assert_eq!(events.len(), 2);
    }
}
