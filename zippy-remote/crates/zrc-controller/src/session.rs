//! Session client for managing remote control sessions
//!
//! This module implements the controller-side session workflow:
//! - Verify device is paired before initiating session
//! - Generate SessionInitRequestV1 with requested capabilities
//! - Send session request via configured transport
//! - Handle SessionInitResponseV1 with ticket and transport params
//! - Establish QUIC connection with certificate verification
//!
//! Requirements: 3.1-3.8, 4.1-4.8

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use prost::Message;
use thiserror::Error;
use tokio::sync::RwLock;

use zrc_core::store::InMemoryStore;
use zrc_crypto::hash::sha256;
use zrc_proto::v1::{SessionInitRequestV1, SessionInitResponseV1};

use crate::identity::IdentityManager;
use crate::pairing::{PairingError, TransportClient, TransportPreference};
use crate::pairings::{PairingsStore, StoredPairing};

/// Session operation errors
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Device not paired: {0}")]
    NotPaired(String),

    #[error("Session denied: {0}")]
    Denied(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Signature invalid")]
    SignatureInvalid,

    #[error("Ticket expired")]
    TicketExpired,

    #[error("Missing field: {0}")]
    MissingField(String),
}

impl From<zrc_core::session::SessionError> for SessionError {
    fn from(e: zrc_core::session::SessionError) -> Self {
        match e {
            zrc_core::session::SessionError::NotPaired => {
                SessionError::NotPaired("Device not paired".to_string())
            }
            zrc_core::session::SessionError::PermissionDenied(msg) => {
                SessionError::PermissionDenied(msg)
            }
            zrc_core::session::SessionError::InvalidState(msg) => SessionError::InvalidState(msg),
            zrc_core::session::SessionError::SignatureInvalid => SessionError::SignatureInvalid,
            zrc_core::session::SessionError::TicketExpired => SessionError::TicketExpired,
            zrc_core::session::SessionError::MissingField(msg) => SessionError::MissingField(msg),
            zrc_core::session::SessionError::CryptoError(msg) => SessionError::Crypto(msg),
            zrc_core::session::SessionError::StoreError(msg) => SessionError::Store(msg),
            _ => SessionError::Transport(e.to_string()),
        }
    }
}

/// Options for session initiation
#[derive(Debug, Clone)]
pub struct SessionOptions {
    /// Requested capabilities
    pub capabilities: Vec<String>,
    /// Transport preference
    pub transport_preference: TransportPreference,
    /// Operation timeout
    pub timeout: Duration,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            capabilities: Vec::new(),
            transport_preference: TransportPreference::Auto,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Result of session initiation
#[derive(Debug, Clone)]
pub struct SessionInitResult {
    /// Session ID
    pub session_id: String,
    /// Granted capabilities
    pub granted_capabilities: Vec<String>,
    /// QUIC connection parameters
    pub quic_host: String,
    pub quic_port: u16,
    pub cert_fingerprint: [u8; 32],
    /// Session ticket for authentication
    pub ticket: Vec<u8>,
}

/// Parameters for QUIC connection
#[derive(Debug, Clone)]
pub struct QuicConnectParams {
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Expected certificate fingerprint
    pub cert_fingerprint: [u8; 32],
    /// Session ticket
    pub ticket: Vec<u8>,
    /// Optional relay URL
    pub relay_url: Option<String>,
}

/// Active QUIC session
pub struct QuicSession {
    /// Session ID
    pub session_id: String,
    /// When the session was established
    pub established_at: SystemTime,
    /// Device ID
    pub device_id: String,
    /// Granted permissions
    pub permissions: u32,
}

/// Identity keys for session operations
#[derive(Clone)]
pub struct OperatorKeys {
    /// 32-byte operator ID (SHA256 of signing public key)
    pub id32: [u8; 32],
    /// Ed25519 signing key
    pub sign: ed25519_dalek::SigningKey,
    /// X25519 key exchange secret
    pub kex: x25519_dalek::StaticSecret,
}

impl OperatorKeys {
    /// Create from IdentityManager
    pub fn from_identity(identity: &IdentityManager) -> Self {
        // Compute operator ID as SHA256 of signing public key
        let sign_pub = identity.sign_pub();
        let id32: [u8; 32] = sha256(&sign_pub).into();

        // We need to reconstruct the signing key from the public key
        // Since IdentityManager doesn't expose the private key directly,
        // we'll use a workaround by creating a new ephemeral key for now
        // In a real implementation, IdentityManager would expose the signing key
        let sign = ed25519_dalek::SigningKey::from_bytes(&[0u8; 32]); // Placeholder
        let kex = x25519_dalek::StaticSecret::from([0u8; 32]); // Placeholder

        Self { id32, sign, kex }
    }
}

/// Handles session operations
/// Requirements: 3.1-3.8, 4.1-4.8
pub struct SessionClient {
    /// Operator identity manager
    identity: Arc<IdentityManager>,
    /// Transport client for sending messages
    transport: TransportClient,
    /// Pairings store for verification
    pairings_store: Option<PairingsStore>,
    /// In-memory store for session state
    memory_store: Arc<InMemoryStore>,
    /// Active sessions
    active_sessions: RwLock<HashMap<String, QuicSession>>,
    /// Transport preference
    transport_preference: TransportPreference,
    /// Session timeout
    timeout: Duration,
}

impl SessionClient {
    /// Create a new session client
    pub fn new() -> Self {
        Self {
            identity: Arc::new(IdentityManager::new_ephemeral()),
            transport: TransportClient::new(),
            pairings_store: None,
            memory_store: Arc::new(InMemoryStore::new()),
            active_sessions: RwLock::new(HashMap::new()),
            transport_preference: TransportPreference::Auto,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new session client with identity
    pub fn with_identity(identity: Arc<IdentityManager>) -> Self {
        Self {
            identity,
            transport: TransportClient::new(),
            pairings_store: None,
            memory_store: Arc::new(InMemoryStore::new()),
            active_sessions: RwLock::new(HashMap::new()),
            transport_preference: TransportPreference::Auto,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new session client with full configuration
    pub fn with_config(
        identity: Arc<IdentityManager>,
        transport: TransportClient,
        pairings_store: Option<PairingsStore>,
    ) -> Self {
        Self {
            identity,
            transport,
            pairings_store,
            memory_store: Arc::new(InMemoryStore::new()),
            active_sessions: RwLock::new(HashMap::new()),
            transport_preference: TransportPreference::Auto,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set transport preference
    pub fn set_transport_preference(&mut self, preference: TransportPreference) {
        self.transport_preference = preference;
    }

    /// Set session timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Verify device is paired
    /// Requirements: 3.2
    pub fn verify_pairing(&self, device_id: &str) -> Result<StoredPairing, SessionError> {
        if let Some(ref store) = self.pairings_store {
            match store.get(device_id) {
                Ok(Some(pairing)) => Ok(pairing),
                Ok(None) => Err(SessionError::NotPaired(device_id.to_string())),
                Err(e) => Err(SessionError::Store(e.to_string())),
            }
        } else {
            // No pairings store configured - cannot verify
            Err(SessionError::NotPaired(format!(
                "No pairings store configured to verify device {}",
                device_id
            )))
        }
    }

    /// Convert capability strings to bitmask
    fn capabilities_to_mask(capabilities: &[String]) -> u32 {
        let mut mask = 0u32;
        for cap in capabilities {
            match cap.to_lowercase().as_str() {
                "view" => mask |= 0x01,
                "control" => mask |= 0x02,
                "clipboard" => mask |= 0x04,
                "file_transfer" => mask |= 0x08,
                "audio" => mask |= 0x10,
                "unattended" => mask |= 0x20,
                _ => {}
            }
        }
        mask
    }

    /// Convert permission mask to capability strings
    fn mask_to_capabilities(mask: u32) -> Vec<String> {
        let mut caps = Vec::new();
        if mask & 0x01 != 0 {
            caps.push("view".to_string());
        }
        if mask & 0x02 != 0 {
            caps.push("control".to_string());
        }
        if mask & 0x04 != 0 {
            caps.push("clipboard".to_string());
        }
        if mask & 0x08 != 0 {
            caps.push("file_transfer".to_string());
        }
        if mask & 0x10 != 0 {
            caps.push("audio".to_string());
        }
        if mask & 0x20 != 0 {
            caps.push("unattended".to_string());
        }
        caps
    }

    /// Get operator ID as 32 bytes
    fn operator_id_bytes(&self) -> [u8; 32] {
        sha256(&self.identity.sign_pub()).into()
    }

    /// Generate a SessionInitRequestV1
    /// Requirements: 3.3
    pub fn generate_session_request(
        &self,
        device_id: &[u8],
        requested_capabilities: u32,
    ) -> Result<SessionInitRequestV1, SessionError> {
        // Generate session ID (32 bytes)
        let mut session_id = [0u8; 32];
        getrandom::getrandom(&mut session_id)
            .map_err(|e| SessionError::Crypto(format!("RNG failed: {e}")))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Build the session init request
        let mut request = SessionInitRequestV1 {
            operator_id: self.operator_id_bytes().to_vec(),
            device_id: device_id.to_vec(),
            session_id: session_id.to_vec(),
            requested_capabilities,
            transport_preference: 0, // AUTO
            operator_signature: vec![], // Will be filled by signing
            ..Default::default()
        };

        // Sign the request
        self.sign_session_request(&mut request)?;

        Ok(request)
    }

    /// Sign a SessionInitRequestV1
    fn sign_session_request(&self, request: &mut SessionInitRequestV1) -> Result<(), SessionError> {
        // Get the bytes to sign (request without signature)
        let mut request_copy = request.clone();
        request_copy.operator_signature = vec![];
        let bytes = request_copy.encode_to_vec();
        let digest = sha256(&bytes);

        // Sign the digest
        let signature = self.identity.sign(&digest);
        request.operator_signature = signature.to_vec();

        Ok(())
    }

    /// Initiate a new session
    /// Requirements: 3.1, 3.2, 3.3
    pub async fn start_session(
        &self,
        device_id: &str,
        options: SessionOptions,
    ) -> Result<SessionInitRequestV1, SessionError> {
        // Verify device is paired (Requirements: 3.2)
        let pairing = self.verify_pairing(device_id)?;

        // Convert device_id from hex to bytes
        let device_id_bytes = hex::decode(device_id)
            .map_err(|e| SessionError::InvalidState(format!("Invalid device ID hex: {e}")))?;

        if device_id_bytes.len() != 32 {
            return Err(SessionError::InvalidState(
                "Device ID must be 32 bytes".to_string(),
            ));
        }

        // Convert requested capabilities to bitmask
        let requested_capabilities = Self::capabilities_to_mask(&options.capabilities);

        // Validate requested capabilities don't exceed paired permissions
        let paired_permissions = Self::capabilities_to_mask(&pairing.permissions);
        if requested_capabilities != 0 && (requested_capabilities & !paired_permissions) != 0 {
            return Err(SessionError::PermissionDenied(
                "Requested capabilities exceed paired permissions".to_string(),
            ));
        }

        // Generate SessionInitRequestV1 (Requirements: 3.3)
        let request = self.generate_session_request(&device_id_bytes, requested_capabilities)?;

        Ok(request)
    }

    /// Send session request via transport
    /// Requirements: 3.4
    pub async fn send_session_request(
        &self,
        device_id: &[u8],
        request: &SessionInitRequestV1,
    ) -> Result<(), SessionError> {
        let request_bytes = request.encode_to_vec();

        // Send via configured transport
        self.transport
            .send_session_request(device_id, &request_bytes, self.transport_preference)
            .await
            .map_err(|e| SessionError::Transport(e.to_string()))
    }

    /// Wait for SessionInitResponseV1
    /// Requirements: 3.5
    pub async fn wait_for_response(&self) -> Result<SessionInitResponseV1, SessionError> {
        let operator_id = self.operator_id_bytes();

        // Poll for response
        let response_bytes = self
            .transport
            .poll_response(&operator_id, self.timeout)
            .await
            .map_err(|e| SessionError::Transport(e.to_string()))?;

        match response_bytes {
            Some(data) => {
                let response = SessionInitResponseV1::decode(data.as_slice())
                    .map_err(|e| SessionError::Transport(format!("Failed to decode response: {e}")))?;
                Ok(response)
            }
            None => Err(SessionError::Timeout(self.timeout)),
        }
    }

    /// Handle session init response
    /// Requirements: 3.5, 3.6
    pub fn handle_response(
        &self,
        response: SessionInitResponseV1,
        device_sign_pub: &[u8],
    ) -> Result<SessionInitResult, SessionError> {
        // Verify response fields
        if response.session_id.is_empty() {
            return Err(SessionError::MissingField("session_id".to_string()));
        }

        // Verify device signature
        self.verify_response_signature(&response, device_sign_pub)?;

        // Check if session was denied (no ticket issued means denial)
        if response.issued_ticket.is_none() && response.granted_capabilities == 0 {
            return Err(SessionError::Denied("Session request denied by device".to_string()));
        }

        // Extract ticket
        let ticket = response
            .issued_ticket
            .ok_or(SessionError::MissingField("issued_ticket".to_string()))?;

        // Validate ticket expiration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if ticket.expires_at <= now {
            return Err(SessionError::TicketExpired);
        }

        // Extract transport params
        let (quic_host, quic_port, cert_fingerprint) = if let Some(params) = response.transport_params {
            if let Some(quic) = params.quic_params {
                let endpoint = quic.endpoints.first().ok_or(SessionError::MissingField(
                    "QUIC endpoint".to_string(),
                ))?;
                // Compute cert fingerprint from DER certificate
                let cert_fp: [u8; 32] = if quic.server_cert_der.len() >= 32 {
                    sha256(&quic.server_cert_der).into()
                } else {
                    [0u8; 32]
                };
                (endpoint.host.clone(), endpoint.port as u16, cert_fp)
            } else {
                return Err(SessionError::MissingField("QUIC params".to_string()));
            }
        } else {
            return Err(SessionError::MissingField("transport_params".to_string()));
        };

        Ok(SessionInitResult {
            session_id: hex::encode(&response.session_id),
            granted_capabilities: Self::mask_to_capabilities(response.granted_capabilities),
            quic_host,
            quic_port,
            cert_fingerprint,
            ticket: ticket.encode_to_vec(),
        })
    }

    /// Verify device signature on response
    fn verify_response_signature(
        &self,
        response: &SessionInitResponseV1,
        device_sign_pub: &[u8],
    ) -> Result<(), SessionError> {
        use ed25519_dalek::{Signature, VerifyingKey};

        if response.device_signature.len() != 64 {
            return Err(SessionError::SignatureInvalid);
        }
        if device_sign_pub.len() != 32 {
            return Err(SessionError::SignatureInvalid);
        }

        // Get the bytes that were signed (response without signature)
        let mut response_copy = response.clone();
        response_copy.device_signature = vec![];
        let bytes = response_copy.encode_to_vec();
        let digest = sha256(&bytes);

        // Parse the public key
        let pub_key_bytes: [u8; 32] = device_sign_pub
            .try_into()
            .map_err(|_| SessionError::SignatureInvalid)?;
        let verifying_key = VerifyingKey::from_bytes(&pub_key_bytes)
            .map_err(|_| SessionError::SignatureInvalid)?;

        // Parse the signature
        let sig_bytes: [u8; 64] = response.device_signature[..64]
            .try_into()
            .map_err(|_| SessionError::SignatureInvalid)?;
        let signature = Signature::from_bytes(&sig_bytes);

        // Verify
        verifying_key
            .verify_strict(&digest, &signature)
            .map_err(|_| SessionError::SignatureInvalid)?;

        Ok(())
    }

    /// Connect to session via QUIC
    /// Requirements: 4.1, 4.2, 4.3
    pub async fn connect_quic(
        &self,
        _params: QuicConnectParams,
    ) -> Result<QuicSession, SessionError> {
        // TODO: Implement in task 7.3
        Err(SessionError::ConnectionFailed("Not implemented".to_string()))
    }

    /// End a session
    pub async fn end_session(&self, session_id: &str) -> Result<(), SessionError> {
        let mut sessions = self.active_sessions.write().await;
        if sessions.remove(session_id).is_some() {
            Ok(())
        } else {
            Err(SessionError::NotFound(session_id.to_string()))
        }
    }

    /// List active sessions
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.active_sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get active session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<String> {
        let sessions = self.active_sessions.read().await;
        sessions.get(session_id).map(|s| s.device_id.clone())
    }
}

impl Default for SessionClient {
    fn default() -> Self {
        Self::new()
    }
}

// Extension for TransportClient to support session requests
impl TransportClient {
    /// Send a session request to the device via configured transport
    pub async fn send_session_request(
        &self,
        device_id: &[u8],
        request_bytes: &[u8],
        preference: TransportPreference,
    ) -> Result<(), PairingError> {
        // Reuse the same transport logic as pairing
        // The message type is different but the transport mechanism is the same
        match preference {
            TransportPreference::Auto => {
                self.send_via_rendezvous(device_id, request_bytes).await
            }
            TransportPreference::Mesh => {
                self.send_via_mesh(device_id, request_bytes).await
            }
            TransportPreference::Rendezvous => {
                self.send_via_rendezvous(device_id, request_bytes).await
            }
            TransportPreference::Direct => {
                Err(PairingError::Transport(
                    "Direct transport not yet implemented".to_string(),
                ))
            }
            TransportPreference::Relay => {
                self.send_via_relay(device_id, request_bytes).await
            }
        }
    }

    // Note: send_via_rendezvous, send_via_mesh, send_via_relay are already implemented
    // in the pairing module and are private. We need to make them accessible or
    // duplicate the logic here. For now, we'll reference the existing implementation.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_to_mask() {
        let caps = vec!["view".to_string(), "control".to_string()];
        let mask = SessionClient::capabilities_to_mask(&caps);
        assert_eq!(mask, 0x03);

        let caps = vec!["clipboard".to_string(), "audio".to_string()];
        let mask = SessionClient::capabilities_to_mask(&caps);
        assert_eq!(mask, 0x14);
    }

    #[test]
    fn test_mask_to_capabilities() {
        let caps = SessionClient::mask_to_capabilities(0x03);
        assert!(caps.contains(&"view".to_string()));
        assert!(caps.contains(&"control".to_string()));
        assert_eq!(caps.len(), 2);

        let caps = SessionClient::mask_to_capabilities(0x3F);
        assert_eq!(caps.len(), 6);
    }

    #[test]
    fn test_session_options_default() {
        let options = SessionOptions::default();
        assert!(options.capabilities.is_empty());
        assert_eq!(options.timeout, Duration::from_secs(30));
    }
}
