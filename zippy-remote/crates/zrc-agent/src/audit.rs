//! Audit logging with cryptographic signing.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use zrc_crypto::identity::Identity;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit log write failed: {0}")]
    WriteFailed(String),
    #[error("audit log read failed: {0}")]
    ReadFailed(String),
    #[error("signature verification failed")]
    SignatureVerificationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub operator_id: Option<Vec<u8>>,
    pub session_id: Option<Vec<u8>>,
    pub details: serde_json::Value,
    pub signature: Vec<u8>,
}

pub struct AuditLogger {
    log_path: PathBuf,
    identity: Arc<Identity>,
    events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl AuditLogger {
    pub fn new(log_path: PathBuf, identity: Arc<Identity>) -> Result<Self, AuditError> {
        // Ensure log directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AuditError::WriteFailed(e.to_string()))?;
        }

        Ok(Self {
            log_path,
            identity,
            events: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub async fn log_pairing(
        &self,
        operator_id: &[u8; 32],
        approved: bool,
    ) -> Result<(), AuditError> {
        let event = AuditEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            event_type: if approved { "pairing_approved" } else { "pairing_denied" }.to_string(),
            operator_id: Some(operator_id.to_vec()),
            session_id: None,
            details: serde_json::json!({
                "approved": approved,
            }),
            signature: Vec::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_session_start(
        &self,
        operator_id: &[u8; 32],
        session_id: &[u8],
    ) -> Result<(), AuditError> {
        let event = AuditEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            event_type: "session_start".to_string(),
            operator_id: Some(operator_id.to_vec()),
            session_id: Some(session_id.to_vec()),
            details: serde_json::json!({}),
            signature: Vec::new(),
        };

        self.log_event(event).await
    }

    pub async fn log_session_end(
        &self,
        operator_id: &[u8; 32],
        session_id: &[u8],
        reason: &str,
    ) -> Result<(), AuditError> {
        let event = AuditEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            event_type: "session_end".to_string(),
            operator_id: Some(operator_id.to_vec()),
            session_id: Some(session_id.to_vec()),
            details: serde_json::json!({
                "reason": reason,
            }),
            signature: Vec::new(),
        };

        self.log_event(event).await
    }

    async fn log_event(&self, mut event: AuditEvent) -> Result<(), AuditError> {
        // Sign the event
        let event_json = serde_json::to_string(&event)
            .map_err(|e| AuditError::WriteFailed(e.to_string()))?;
        let signature = self.identity.sign(event_json.as_bytes());
        event.signature = signature.to_vec();

        // Store event
        let mut events = self.events.write().await;
        events.push(event.clone());

        // Write to file (append)
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| AuditError::WriteFailed(e.to_string()))?;
        
        use std::io::Write;
        writeln!(file, "{}", serde_json::to_string(&event).unwrap())
            .map_err(|e| AuditError::WriteFailed(e.to_string()))?;

        info!("Audit event logged: {}", event.event_type);
        Ok(())
    }
}
