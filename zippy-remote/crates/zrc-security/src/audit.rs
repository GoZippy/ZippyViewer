//! Audit logging with cryptographic signing.
//!
//! Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7

use std::io::Write;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::SecurityError;

/// Security event types for audit logging.
///
/// Requirements: 9.1, 9.2, 9.3, 9.4, 9.5
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityEvent {
    /// Authentication attempt
    AuthenticationAttempt {
        success: bool,
        source: String,
    },
    /// Pairing request
    PairingRequest {
        operator_id: String,
        device_id: String,
    },
    /// Pairing approved
    PairingApproved {
        operator_id: String,
        device_id: String,
    },
    /// Pairing revoked
    PairingRevoked {
        operator_id: String,
        device_id: String,
        reason: String,
    },
    /// Session started
    SessionStarted {
        session_id: String,
        operator_id: String,
        device_id: String,
    },
    /// Session ended
    SessionEnded {
        session_id: String,
        reason: String,
    },
    /// Permission escalation
    PermissionEscalation {
        session_id: String,
        new_permissions: u32,
    },
    /// Identity mismatch detected
    IdentityMismatch {
        peer_id: String,
    },
    /// Replay attempt detected
    ReplayAttempt {
        sequence: u64,
    },
    /// Rate limit exceeded
    RateLimitExceeded {
        source: String,
        operation: String,
    },
}

impl SecurityEvent {
    /// Get the event type as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            SecurityEvent::AuthenticationAttempt { .. } => "authentication_attempt",
            SecurityEvent::PairingRequest { .. } => "pairing_request",
            SecurityEvent::PairingApproved { .. } => "pairing_approved",
            SecurityEvent::PairingRevoked { .. } => "pairing_revoked",
            SecurityEvent::SessionStarted { .. } => "session_started",
            SecurityEvent::SessionEnded { .. } => "session_ended",
            SecurityEvent::PermissionEscalation { .. } => "permission_escalation",
            SecurityEvent::IdentityMismatch { .. } => "identity_mismatch",
            SecurityEvent::ReplayAttempt { .. } => "replay_attempt",
            SecurityEvent::RateLimitExceeded { .. } => "rate_limit_exceeded",
        }
    }

    /// Get the actor (who performed the action).
    pub fn actor(&self) -> Option<String> {
        match self {
            SecurityEvent::AuthenticationAttempt { source, .. } => Some(source.clone()),
            SecurityEvent::PairingRequest { operator_id, .. } => Some(operator_id.clone()),
            SecurityEvent::PairingApproved { operator_id, .. } => Some(operator_id.clone()),
            SecurityEvent::PairingRevoked { operator_id, .. } => Some(operator_id.clone()),
            SecurityEvent::SessionStarted { operator_id, .. } => Some(operator_id.clone()),
            SecurityEvent::SessionEnded { .. } => None,
            SecurityEvent::PermissionEscalation { session_id, .. } => Some(session_id.clone()),
            SecurityEvent::IdentityMismatch { peer_id, .. } => Some(peer_id.clone()),
            SecurityEvent::ReplayAttempt { .. } => None,
            SecurityEvent::RateLimitExceeded { source, .. } => Some(source.clone()),
        }
    }

    /// Get the target (what was acted upon).
    pub fn target(&self) -> Option<String> {
        match self {
            SecurityEvent::AuthenticationAttempt { .. } => None,
            SecurityEvent::PairingRequest { device_id, .. } => Some(device_id.clone()),
            SecurityEvent::PairingApproved { device_id, .. } => Some(device_id.clone()),
            SecurityEvent::PairingRevoked { device_id, .. } => Some(device_id.clone()),
            SecurityEvent::SessionStarted { session_id, .. } => Some(session_id.clone()),
            SecurityEvent::SessionEnded { session_id, .. } => Some(session_id.clone()),
            SecurityEvent::PermissionEscalation { session_id, .. } => Some(session_id.clone()),
            SecurityEvent::IdentityMismatch { peer_id, .. } => Some(peer_id.clone()),
            SecurityEvent::ReplayAttempt { sequence, .. } => Some(format!("seq:{}", sequence)),
            SecurityEvent::RateLimitExceeded { operation, .. } => Some(operation.clone()),
        }
    }

    /// Get event details as JSON.
    pub fn details(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::json!({}))
    }
}

/// Audit log entry with signature.
///
/// Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: String,
    /// Actor (who performed the action)
    pub actor: Option<String>,
    /// Target (what was acted upon)
    pub target: Option<String>,
    /// Event details
    pub details: serde_json::Value,
    /// Cryptographic signature
    pub signature: Vec<u8>,
}

impl AuditEntry {
    /// Convert entry to canonical bytes for signing (without signature).
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut entry = self.clone();
        entry.signature = Vec::new();
        // This should never fail for valid AuditEntry, but we handle it gracefully
        serde_json::to_vec(&entry)
            .unwrap_or_else(|_| b"{}".to_vec())
    }

    /// Convert entry to canonical bytes without signature (for verification).
    pub fn to_canonical_bytes_without_signature(&self) -> Vec<u8> {
        let mut entry = self.clone();
        entry.signature = Vec::new();
        // This should never fail for valid AuditEntry, but we handle it gracefully
        serde_json::to_vec(&entry)
            .unwrap_or_else(|_| b"{}".to_vec())
    }
}

/// Trait for writing audit log entries.
pub trait AuditLogWriter: Send + Sync {
    /// Write an audit entry.
    fn write(&self, entry: &AuditEntry) -> Result<(), SecurityError>;
}

/// File-based audit log writer.
pub struct FileAuditLogWriter {
    path: PathBuf,
}

impl FileAuditLogWriter {
    /// Create a new file-based audit log writer.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl AuditLogWriter for FileAuditLogWriter {
    fn write(&self, entry: &AuditEntry) -> Result<(), SecurityError> {
        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SecurityError::AuditError(format!("Failed to create log directory: {}", e)))?;
        }

        // Append entry to file
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| SecurityError::AuditError(format!("Failed to open log file: {}", e)))?;

        let entry_json = serde_json::to_string(entry)
            .map_err(|e| SecurityError::AuditError(format!("Failed to serialize entry: {}", e)))?;

        writeln!(file, "{}", entry_json)
            .map_err(|e| SecurityError::AuditError(format!("Failed to write entry: {}", e)))?;

        Ok(())
    }
}

/// Audit logger with cryptographic signing.
///
/// Requirements: 9.1, 9.6
pub struct AuditLogger {
    signing_key: SigningKey,
    log_writer: Box<dyn AuditLogWriter>,
}

impl AuditLogger {
    /// Create a new audit logger.
    ///
    /// # Arguments
    /// * `signing_key` - Ed25519 signing key for signing entries
    /// * `log_writer` - Writer for audit entries
    pub fn new(signing_key: SigningKey, log_writer: Box<dyn AuditLogWriter>) -> Self {
        Self {
            signing_key,
            log_writer,
        }
    }

    /// Log a security event.
    ///
    /// Requirements: 9.1, 9.2, 9.3, 9.4, 9.5
    pub fn log(&self, event: SecurityEvent) -> Result<(), SecurityError> {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: event.event_type().to_string(),
            actor: event.actor(),
            target: event.target(),
            details: event.details(),
            signature: Vec::new(), // Filled below
        };

        // Sign the entry
        let entry_bytes = entry.to_canonical_bytes();
        let signature = self.signing_key.sign(&entry_bytes);
        
        let signed_entry = AuditEntry {
            signature: signature.to_bytes().to_vec(),
            ..entry
        };

        // Write to log
        self.log_writer.write(&signed_entry)?;

        Ok(())
    }

    /// Verify audit log integrity.
    ///
    /// Requirements: 9.7
    pub fn verify_log(&self, entries: &[AuditEntry]) -> Result<(), SecurityError> {
        let verifying_key = VerifyingKey::from(&self.signing_key);

        for entry in entries {
            let entry_bytes = entry.to_canonical_bytes_without_signature();
            
            if entry.signature.len() != 64 {
                return Err(SecurityError::AuditError("Invalid signature length".to_string()));
            }
            
            let sig_bytes: [u8; 64] = entry.signature.as_slice()
                .try_into()
                .map_err(|_| SecurityError::AuditError("Invalid signature length".to_string()))?;
            
            let signature = Signature::from_bytes(&sig_bytes);

            verifying_key.verify_strict(&entry_bytes, &signature)
                .map_err(|e| SecurityError::AuditError(format!("Signature verification failed: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;
    use std::fs;

    #[test]
    fn test_audit_logging() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("test_audit.log");
        
        // Clean up any existing file
        let _ = fs::remove_file(&log_path);

        let writer = Box::new(FileAuditLogWriter::new(log_path.clone()));
        let logger = AuditLogger::new(signing_key.clone(), writer);

        let event = SecurityEvent::AuthenticationAttempt {
            success: true,
            source: "test_source".to_string(),
        };

        assert!(logger.log(event).is_ok());

        // Verify file was created
        assert!(log_path.exists());
    }

    #[test]
    fn test_audit_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let writer = Box::new(FileAuditLogWriter::new(PathBuf::from("/dev/null")));
        let logger = AuditLogger::new(signing_key.clone(), writer);

        let event = SecurityEvent::SessionStarted {
            session_id: "test_session".to_string(),
            operator_id: "test_operator".to_string(),
            device_id: "test_device".to_string(),
        };

        // Create entry manually for testing
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: event.event_type().to_string(),
            actor: event.actor(),
            target: event.target(),
            details: event.details(),
            signature: Vec::new(),
        };

        let entry_bytes = entry.to_canonical_bytes();
        let signature = signing_key.sign(&entry_bytes);
        let signed_entry = AuditEntry {
            signature: signature.to_bytes().to_vec(),
            ..entry
        };

        // Verify the entry
        assert!(logger.verify_log(&[signed_entry]).is_ok());
    }
}
