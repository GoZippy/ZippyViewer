//! Pairing client for device pairing operations
//!
//! This module implements the controller-side pairing workflow:
//! - Import and validate invites from various sources
//! - Generate PairRequestV1 with invite proof
//! - Send requests via configured transport
//! - Handle PairReceiptV1 responses
//! - Verify SAS codes for MITM protection
//! - Store successful pairings
//!
//! Requirements: 2.1-2.8

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use prost::Message;
use thiserror::Error;

use zrc_core::store::{InMemoryStore, PairingRecord, Store};
use zrc_crypto::hash::sha256;
use zrc_crypto::pairing::{
    canonical_pair_request_fields_without_proof_v1, compute_pair_proof_v1,
    compute_pairing_sas_6digit_v1, pair_proof_input_v1, pairing_sas_transcript_v1,
};
use zrc_proto::v1::{
    DeviceIdV1, EndpointHintsV1, InviteV1, KeyTypeV1, PairReceiptV1, PairRequestV1,
    PublicKeyV1, TimestampV1, UserIdV1,
};
use zrc_proto::Validate;

use crate::identity::IdentityManager;
use crate::pairings::{PairingsStore, StoredPairing};

/// Transport client for sending pairing messages
/// Requirements: 2.3, 8.1-8.6
pub struct TransportClient {
    /// Rendezvous server URLs
    rendezvous_urls: Vec<String>,
    /// Relay server URLs  
    relay_urls: Vec<String>,
    /// Mesh node addresses
    mesh_nodes: Vec<String>,
    /// HTTP client for rendezvous
    #[cfg(feature = "http-mailbox")]
    http_client: Option<reqwest::Client>,
}

impl TransportClient {
    /// Create a new transport client with default configuration
    pub fn new() -> Self {
        Self {
            rendezvous_urls: vec!["https://rendezvous.zippyremote.io".to_string()],
            relay_urls: vec!["https://relay.zippyremote.io".to_string()],
            mesh_nodes: Vec::new(),
            #[cfg(feature = "http-mailbox")]
            http_client: reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .ok(),
        }
    }

    /// Create a transport client with custom URLs
    pub fn with_urls(
        rendezvous_urls: Vec<String>,
        relay_urls: Vec<String>,
        mesh_nodes: Vec<String>,
    ) -> Self {
        Self {
            rendezvous_urls,
            relay_urls,
            mesh_nodes,
            #[cfg(feature = "http-mailbox")]
            http_client: reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .ok(),
        }
    }

    /// Send a pair request to the device via configured transport
    /// Requirements: 2.3, 8.3
    pub async fn send_pair_request(
        &self,
        device_id: &[u8],
        request: &PairRequestV1,
        preference: TransportPreference,
    ) -> Result<(), PairingError> {
        let request_bytes = request.encode_to_vec();

        match preference {
            TransportPreference::Auto => {
                // Try transports in ladder order: mesh → direct → rendezvous → relay
                // Requirements: 8.3
                self.send_with_ladder(device_id, &request_bytes).await
            }
            TransportPreference::Mesh => {
                self.send_via_mesh(device_id, &request_bytes).await
            }
            TransportPreference::Rendezvous => {
                self.send_via_rendezvous(device_id, &request_bytes).await
            }
            TransportPreference::Direct => {
                self.send_via_direct(device_id, &request_bytes).await
            }
            TransportPreference::Relay => {
                self.send_via_relay(device_id, &request_bytes).await
            }
        }
    }

    /// Send using transport ladder (mesh → direct → rendezvous → relay)
    /// Requirements: 8.3
    async fn send_with_ladder(
        &self,
        device_id: &[u8],
        data: &[u8],
    ) -> Result<(), PairingError> {
        let mut errors = Vec::new();

        // 1. Try mesh first (if configured)
        if !self.mesh_nodes.is_empty() {
            tracing::debug!("Transport ladder: trying mesh...");
            match self.send_via_mesh(device_id, data).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::debug!("Mesh transport failed: {}", e);
                    errors.push(format!("mesh: {}", e));
                }
            }
        }

        // 2. Try direct (if we have endpoint hints)
        tracing::debug!("Transport ladder: trying direct...");
        match self.send_via_direct(device_id, data).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::debug!("Direct transport failed: {}", e);
                errors.push(format!("direct: {}", e));
            }
        }

        // 3. Try rendezvous
        if !self.rendezvous_urls.is_empty() {
            tracing::debug!("Transport ladder: trying rendezvous...");
            match self.send_via_rendezvous(device_id, data).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::debug!("Rendezvous transport failed: {}", e);
                    errors.push(format!("rendezvous: {}", e));
                }
            }
        }

        // 4. Try relay as last resort
        if !self.relay_urls.is_empty() {
            tracing::debug!("Transport ladder: trying relay...");
            match self.send_via_relay(device_id, data).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::debug!("Relay transport failed: {}", e);
                    errors.push(format!("relay: {}", e));
                }
            }
        }

        // All transports failed
        Err(PairingError::Transport(format!(
            "All transports failed: {}",
            errors.join("; ")
        )))
    }

    /// Send via direct connection
    async fn send_via_direct(
        &self,
        _device_id: &[u8],
        _data: &[u8],
    ) -> Result<(), PairingError> {
        // Direct transport requires endpoint hints from the invite
        // This would typically use UDP hole punching or direct TCP
        Err(PairingError::Transport(
            "Direct transport not yet implemented".to_string(),
        ))
    }

    /// Send via rendezvous server
    pub async fn send_via_rendezvous(
        &self,
        device_id: &[u8],
        data: &[u8],
    ) -> Result<(), PairingError> {
        if self.rendezvous_urls.is_empty() {
            return Err(PairingError::Transport(
                "No rendezvous URLs configured".to_string(),
            ));
        }

        #[cfg(feature = "http-mailbox")]
        {
            let client = self.http_client.as_ref().ok_or_else(|| {
                PairingError::Transport("HTTP client not available".to_string())
            })?;

            // Convert device_id to 32-byte array
            let device_id_32: [u8; 32] = device_id
                .try_into()
                .map_err(|_| PairingError::Transport("Invalid device ID length".to_string()))?;

            // Try each rendezvous URL
            let mut last_error = None;
            for url in &self.rendezvous_urls {
                let mailbox_url = format!(
                    "{}/v1/mailbox/{}",
                    url.trim_end_matches('/'),
                    hex::encode(device_id_32)
                );

                match client
                    .post(&mailbox_url)
                    .body(data.to_vec())
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 202 => {
                        return Ok(());
                    }
                    Ok(resp) => {
                        last_error = Some(format!(
                            "Rendezvous returned status {}",
                            resp.status()
                        ));
                    }
                    Err(e) => {
                        last_error = Some(format!("Rendezvous request failed: {e}"));
                    }
                }
            }

            Err(PairingError::Transport(
                last_error.unwrap_or_else(|| "All rendezvous URLs failed".to_string()),
            ))
        }

        #[cfg(not(feature = "http-mailbox"))]
        {
            let _ = (device_id, data);
            Err(PairingError::Transport(
                "HTTP mailbox feature not enabled".to_string(),
            ))
        }
    }

    /// Send via mesh network
    pub async fn send_via_mesh(
        &self,
        _device_id: &[u8],
        _data: &[u8],
    ) -> Result<(), PairingError> {
        if self.mesh_nodes.is_empty() {
            return Err(PairingError::Transport(
                "No mesh nodes configured".to_string(),
            ));
        }
        // TODO: Implement mesh transport
        Err(PairingError::Transport(
            "Mesh transport not yet implemented".to_string(),
        ))
    }

    /// Send via relay server
    pub async fn send_via_relay(
        &self,
        _device_id: &[u8],
        _data: &[u8],
    ) -> Result<(), PairingError> {
        if self.relay_urls.is_empty() {
            return Err(PairingError::Transport(
                "No relay URLs configured".to_string(),
            ));
        }
        // TODO: Implement relay transport
        Err(PairingError::Transport(
            "Relay transport not yet implemented".to_string(),
        ))
    }

    /// Poll for a response from the device
    /// Requirements: 2.4
    pub async fn poll_response(
        &self,
        operator_id: &[u8],
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, PairingError> {
        #[cfg(feature = "http-mailbox")]
        {
            let client = self.http_client.as_ref().ok_or_else(|| {
                PairingError::Transport("HTTP client not available".to_string())
            })?;

            // Convert operator_id to 32-byte array
            let operator_id_32: [u8; 32] = operator_id
                .try_into()
                .map_err(|_| PairingError::Transport("Invalid operator ID length".to_string()))?;

            let start = std::time::Instant::now();
            let wait_ms = 5000u64; // Poll every 5 seconds

            while start.elapsed() < timeout {
                for url in &self.rendezvous_urls {
                    let mailbox_url = format!(
                        "{}/v1/mailbox/{}?wait_ms={}",
                        url.trim_end_matches('/'),
                        hex::encode(operator_id_32),
                        wait_ms
                    );

                    match client.get(&mailbox_url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(bytes) = resp.bytes().await {
                                if !bytes.is_empty() {
                                    return Ok(Some(bytes.to_vec()));
                                }
                            }
                        }
                        Ok(resp) if resp.status().as_u16() == 204 => {
                            // No content, continue polling
                        }
                        Ok(resp) => {
                            tracing::debug!(
                                "Poll returned status {}: {:?}",
                                resp.status(),
                                resp.text().await.ok()
                            );
                        }
                        Err(e) => {
                            tracing::debug!("Poll request failed: {e}");
                        }
                    }
                }

                // Small delay between poll attempts
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            Ok(None)
        }

        #[cfg(not(feature = "http-mailbox"))]
        {
            let _ = (operator_id, timeout);
            Err(PairingError::Transport(
                "HTTP mailbox feature not enabled".to_string(),
            ))
        }
    }

    /// Get configured rendezvous URLs
    pub fn rendezvous_urls(&self) -> &[String] {
        &self.rendezvous_urls
    }

    /// Get configured relay URLs
    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
    }

    /// Get configured mesh nodes
    pub fn mesh_nodes(&self) -> &[String] {
        &self.mesh_nodes
    }

    /// Get available transports based on configuration
    /// Requirements: 8.7
    pub fn available_transports(&self) -> Vec<TransportPreference> {
        let mut available = Vec::new();
        
        if !self.mesh_nodes.is_empty() {
            available.push(TransportPreference::Mesh);
        }
        // Direct is always potentially available (depends on endpoint hints)
        available.push(TransportPreference::Direct);
        if !self.rendezvous_urls.is_empty() {
            available.push(TransportPreference::Rendezvous);
        }
        if !self.relay_urls.is_empty() {
            available.push(TransportPreference::Relay);
        }
        
        available
    }

    /// Display transport ladder info for verbose mode
    /// Requirements: 8.7
    pub fn display_ladder_info(&self) -> String {
        let mut info = String::from("Transport ladder order:\n");
        
        for (i, transport) in TransportPreference::ladder_order().iter().enumerate() {
            let status = match transport {
                TransportPreference::Mesh => {
                    if self.mesh_nodes.is_empty() {
                        "not configured"
                    } else {
                        "configured"
                    }
                }
                TransportPreference::Direct => "available (requires endpoint hints)",
                TransportPreference::Rendezvous => {
                    if self.rendezvous_urls.is_empty() {
                        "not configured"
                    } else {
                        "configured"
                    }
                }
                TransportPreference::Relay => {
                    if self.relay_urls.is_empty() {
                        "not configured"
                    } else {
                        "configured"
                    }
                }
                TransportPreference::Auto => continue,
            };
            info.push_str(&format!("  {}. {} ({})\n", i + 1, transport, status));
        }
        
        info
    }
}

impl Default for TransportClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Pairing operation errors
#[derive(Debug, Error)]
pub enum PairingError {
    #[error("Invalid invite: {0}")]
    InvalidInvite(String),

    #[error("Invite expired at {0:?}")]
    InviteExpired(SystemTime),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Pairing rejected: {0}")]
    Rejected(String),

    #[error("SAS verification failed")]
    SasVerificationFailed,

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Base64 decode error: {0}")]
    Base64Decode(String),

    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(String),

    #[error("JSON parse error: {0}")]
    JsonParse(String),

    #[error("QR code error: {0}")]
    QrCode(String),

    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Invalid proof: invite secret does not match")]
    InvalidProof,

    #[error("Signature verification failed: {0}")]
    SignatureInvalid(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Device not paired: {0}")]
    NotPaired(String),
}

/// Source for importing invites
#[derive(Debug, Clone)]
pub enum InviteSource {
    /// Base64-encoded invite string
    Base64(String),
    /// File path (JSON or binary)
    File(PathBuf),
    /// QR code image file
    QrImage(PathBuf),
    /// System clipboard
    Clipboard,
}

/// Options for pairing operation
#[derive(Debug, Clone)]
pub struct PairOptions {
    /// Requested permissions
    pub requested_permissions: Vec<String>,
    /// Operation timeout
    pub timeout: Duration,
    /// Transport preference
    pub transport_preference: TransportPreference,
}

impl Default for PairOptions {
    fn default() -> Self {
        Self {
            requested_permissions: Vec::new(),
            timeout: Duration::from_secs(30),
            transport_preference: TransportPreference::Auto,
        }
    }
}

/// Transport preference for pairing
/// Requirements: 8.1, 8.2
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransportPreference {
    /// Try transports in ladder order: mesh → direct → rendezvous → relay
    #[default]
    Auto,
    /// Use mesh network only
    Mesh,
    /// Use rendezvous server only
    Rendezvous,
    /// Use direct connection only
    Direct,
    /// Use relay server only
    Relay,
}

impl TransportPreference {
    /// Get the transport ladder order
    /// Requirements: 8.3
    pub fn ladder_order() -> &'static [TransportPreference] {
        &[
            TransportPreference::Mesh,
            TransportPreference::Direct,
            TransportPreference::Rendezvous,
            TransportPreference::Relay,
        ]
    }

    /// Get all valid transport values
    pub fn all_values() -> &'static [&'static str] {
        &["auto", "mesh", "rendezvous", "direct", "relay"]
    }
}

impl std::str::FromStr for TransportPreference {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "mesh" => Ok(Self::Mesh),
            "rendezvous" => Ok(Self::Rendezvous),
            "direct" => Ok(Self::Direct),
            "relay" => Ok(Self::Relay),
            _ => Err(format!(
                "Unknown transport '{}'. Valid values: {}",
                s,
                Self::all_values().join(", ")
            )),
        }
    }
}

impl std::fmt::Display for TransportPreference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Mesh => write!(f, "mesh"),
            Self::Rendezvous => write!(f, "rendezvous"),
            Self::Direct => write!(f, "direct"),
            Self::Relay => write!(f, "relay"),
        }
    }
}

/// Result of successful pairing
#[derive(Debug, Clone)]
pub struct PairingResult {
    /// Device ID that was paired
    pub device_id: String,
    /// Permissions granted by the device
    pub permissions_granted: Vec<String>,
    /// When the pairing was established
    pub paired_at: SystemTime,
    /// Whether SAS verification was completed
    pub sas_verified: bool,
}

/// Parsed invite data with display-friendly fields
#[derive(Debug, Clone)]
pub struct ParsedInvite {
    /// Device ID (hex-encoded)
    pub device_id: String,
    /// Expiration time
    pub expires_at: SystemTime,
    /// Transport hints (human-readable)
    pub transport_hints: Vec<String>,
    /// Raw invite protobuf bytes
    pub raw: Vec<u8>,
    /// The parsed InviteV1 protobuf
    pub invite: InviteV1,
}

impl ParsedInvite {
    /// Check if the invite is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at < SystemTime::now()
    }

    /// Get time until expiration (None if already expired)
    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.expires_at.duration_since(SystemTime::now()).ok()
    }
}

/// JSON representation of an invite for file import/export
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InviteJson {
    /// Device ID (hex-encoded)
    pub device_id: String,
    /// Device signing public key (hex-encoded)
    pub device_sign_pub: String,
    /// Invite secret hash (hex-encoded)
    pub invite_secret_hash: String,
    /// Expiration timestamp (Unix seconds)
    pub expires_at: u64,
    /// Transport hints
    #[serde(default)]
    pub transport_hints: Option<TransportHintsJson>,
}

/// JSON representation of transport hints
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TransportHintsJson {
    #[serde(default)]
    pub direct_addrs: Vec<String>,
    #[serde(default)]
    pub rendezvous_urls: Vec<String>,
    #[serde(default)]
    pub mesh_hints: Vec<String>,
}

impl From<&EndpointHintsV1> for TransportHintsJson {
    fn from(hints: &EndpointHintsV1) -> Self {
        Self {
            direct_addrs: hints.direct_addrs.clone(),
            rendezvous_urls: hints.rendezvous_urls.clone(),
            mesh_hints: hints.mesh_hints.clone(),
        }
    }
}

impl From<TransportHintsJson> for EndpointHintsV1 {
    fn from(json: TransportHintsJson) -> Self {
        Self {
            direct_addrs: json.direct_addrs,
            rendezvous_urls: json.rendezvous_urls,
            mesh_hints: json.mesh_hints,
            relay_tokens: Vec::new(),
        }
    }
}

impl TryFrom<InviteJson> for InviteV1 {
    type Error = PairingError;

    fn try_from(json: InviteJson) -> Result<Self, Self::Error> {
        let device_id = hex::decode(&json.device_id)
            .map_err(|e| PairingError::InvalidInvite(format!("Invalid device_id hex: {e}")))?;
        let device_sign_pub = hex::decode(&json.device_sign_pub)
            .map_err(|e| PairingError::InvalidInvite(format!("Invalid device_sign_pub hex: {e}")))?;
        let invite_secret_hash = hex::decode(&json.invite_secret_hash)
            .map_err(|e| PairingError::InvalidInvite(format!("Invalid invite_secret_hash hex: {e}")))?;

        Ok(Self {
            device_id,
            device_sign_pub,
            invite_secret_hash,
            expires_at: json.expires_at,
            transport_hints: json.transport_hints.map(|h| h.into()),
        })
    }
}

impl From<&InviteV1> for InviteJson {
    fn from(invite: &InviteV1) -> Self {
        Self {
            device_id: hex::encode(&invite.device_id),
            device_sign_pub: hex::encode(&invite.device_sign_pub),
            invite_secret_hash: hex::encode(&invite.invite_secret_hash),
            expires_at: invite.expires_at,
            transport_hints: invite.transport_hints.as_ref().map(|h| h.into()),
        }
    }
}

/// Handles pairing operations
/// Requirements: 2.1-2.8
pub struct PairingClient {
    /// Operator identity manager
    identity: Arc<IdentityManager>,
    /// Transport client for sending messages
    transport: TransportClient,
    /// Pairings store for persistence
    pairings_store: Option<PairingsStore>,
    /// In-memory store for pairing state (used with zrc-core)
    memory_store: Arc<InMemoryStore>,
    /// Current pairing state
    state: PairingState,
    /// Timeout for pairing operations (default: 5 minutes)
    timeout: Duration,
    /// When the current pairing attempt started
    started_at: Option<SystemTime>,
    /// Transport preference
    transport_preference: TransportPreference,
}

/// State of the pairing operation
#[derive(Debug, Clone)]
pub enum PairingState {
    /// Initial state, ready to import invite
    Idle,
    /// Invite has been imported and validated
    InviteImported {
        invite: ParsedInvite,
        invite_secret: Option<[u8; 32]>,
    },
    /// Pair request has been sent, awaiting receipt
    RequestSent {
        request: PairRequestV1,
        invite: ParsedInvite,
    },
    /// Receipt received, awaiting SAS verification
    AwaitingSAS {
        sas: String,
        receipt: PairReceiptV1,
        invite: ParsedInvite,
    },
    /// Pairing completed successfully
    Paired {
        device_id: String,
        permissions: u32,
    },
    /// Pairing failed
    Failed {
        reason: String,
    },
}

impl Default for PairingState {
    fn default() -> Self {
        Self::Idle
    }
}

impl PairingClient {
    /// Create a new pairing client with identity manager
    /// Requirements: 2.1
    pub fn with_identity(identity: Arc<IdentityManager>) -> Self {
        Self {
            identity,
            transport: TransportClient::new(),
            pairings_store: None,
            memory_store: Arc::new(InMemoryStore::new()),
            state: PairingState::Idle,
            timeout: Duration::from_secs(300), // 5 minutes
            started_at: None,
            transport_preference: TransportPreference::Auto,
        }
    }

    /// Create a new pairing client with identity and pairings store
    pub fn with_stores(identity: Arc<IdentityManager>, pairings_store: PairingsStore) -> Self {
        Self {
            identity,
            transport: TransportClient::new(),
            pairings_store: Some(pairings_store),
            memory_store: Arc::new(InMemoryStore::new()),
            state: PairingState::Idle,
            timeout: Duration::from_secs(300),
            started_at: None,
            transport_preference: TransportPreference::Auto,
        }
    }

    /// Create a new pairing client with full configuration
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
            state: PairingState::Idle,
            timeout: Duration::from_secs(300),
            started_at: None,
            transport_preference: TransportPreference::Auto,
        }
    }

    /// Create a new pairing client (legacy, for backward compatibility)
    pub fn new() -> Self {
        Self {
            identity: Arc::new(IdentityManager::new_ephemeral()),
            transport: TransportClient::new(),
            pairings_store: None,
            memory_store: Arc::new(InMemoryStore::new()),
            state: PairingState::Idle,
            timeout: Duration::from_secs(300),
            started_at: None,
            transport_preference: TransportPreference::Auto,
        }
    }

    /// Get the current pairing state
    pub fn state(&self) -> &PairingState {
        &self.state
    }

    /// Set the timeout for pairing operations
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Set the transport preference
    pub fn set_transport_preference(&mut self, preference: TransportPreference) {
        self.transport_preference = preference;
    }

    /// Import and validate an invite from various sources
    /// Requirements: 1.2, 1.3, 1.4, 1.5, 1.6
    pub fn import_invite(&mut self, source: InviteSource) -> Result<ParsedInvite, PairingError> {
        let (raw_bytes, invite) = match source {
            InviteSource::Base64(data) => self.parse_base64(&data)?,
            InviteSource::File(path) => self.parse_file(&path)?,
            InviteSource::QrImage(path) => self.parse_qr_image(&path)?,
            InviteSource::Clipboard => {
                return Err(PairingError::InvalidInvite(
                    "Clipboard import not yet implemented".to_string(),
                ));
            }
        };

        // Validate the invite
        self.validate_invite(&invite)?;

        // Convert to ParsedInvite
        let parsed = self.to_parsed_invite(raw_bytes, invite);

        // Update state to InviteImported
        self.state = PairingState::InviteImported {
            invite: parsed.clone(),
            invite_secret: None,
        };
        self.started_at = Some(SystemTime::now());

        Ok(parsed)
    }

    /// Import invite with the invite secret (for pairing)
    /// Requirements: 2.2
    pub fn import_invite_with_secret(
        &mut self,
        source: InviteSource,
        invite_secret: [u8; 32],
    ) -> Result<ParsedInvite, PairingError> {
        let parsed = self.import_invite(source)?;

        // Verify the secret matches the invite's hash
        let computed_hash = sha256(&invite_secret);
        if computed_hash.to_vec() != parsed.invite.invite_secret_hash {
            self.state = PairingState::Failed {
                reason: "Invite secret does not match".to_string(),
            };
            return Err(PairingError::InvalidProof);
        }

        // Update state with the secret
        self.state = PairingState::InviteImported {
            invite: parsed.clone(),
            invite_secret: Some(invite_secret),
        };

        Ok(parsed)
    }

    /// Generate a PairRequestV1 with invite proof
    /// Requirements: 2.1, 2.2
    pub fn generate_pair_request(
        &mut self,
        invite_secret: &[u8; 32],
        requested_permissions: u32,
    ) -> Result<PairRequestV1, PairingError> {
        // Check timeout
        self.check_timeout()?;

        // Validate state and extract invite
        let invite = match &self.state {
            PairingState::InviteImported { invite, .. } => invite.clone(),
            _ => {
                return Err(PairingError::InvalidState(
                    "Must import invite before generating pair request".to_string(),
                ));
            }
        };

        // Verify the secret matches the invite's hash
        let computed_hash = sha256(invite_secret);
        if computed_hash.to_vec() != invite.invite.invite_secret_hash {
            self.state = PairingState::Failed {
                reason: "Invite secret does not match".to_string(),
            };
            return Err(PairingError::InvalidProof);
        }

        // Check invite hasn't expired
        if invite.is_expired() {
            self.state = PairingState::Failed {
                reason: "Invite has expired".to_string(),
            };
            return Err(PairingError::InviteExpired(invite.expires_at));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Generate nonce for replay protection and SAS computation
        let mut nonce = [0u8; 32];
        getrandom::getrandom(&mut nonce)
            .map_err(|e| PairingError::Identity(format!("RNG failed: {e}")))?;

        // Build proof input using canonical format
        let device_id = DeviceIdV1 {
            id: invite.invite.device_id.clone(),
        };
        let user_id = UserIdV1 {
            id: self.operator_id_bytes(),
        };
        let created_at = TimestampV1 { unix_seconds: now };

        let op_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: self.identity.sign_pub().to_vec(),
        };
        let op_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: self.identity.kex_pub().to_vec(),
        };

        // Compute invite_proof = HMAC-SHA256(invite_secret, proof_input)
        let proof_input = pair_proof_input_v1(
            &user_id,
            &op_sign_pub,
            &op_kex_pub,
            &device_id,
            &created_at,
        );
        let invite_proof = compute_pair_proof_v1(invite_secret, &proof_input);

        // Build the pair request
        let request = PairRequestV1 {
            operator_id: self.operator_id_bytes(),
            operator_sign_pub: self.identity.sign_pub().to_vec(),
            operator_kex_pub: self.identity.kex_pub().to_vec(),
            invite_proof: invite_proof.to_vec(),
            requested_permissions,
            nonce: nonce.to_vec(),
            timestamp: now,
        };

        // Transition to RequestSent state
        self.state = PairingState::RequestSent {
            request: request.clone(),
            invite,
        };

        Ok(request)
    }

    /// Get operator ID as bytes (32 bytes)
    fn operator_id_bytes(&self) -> Vec<u8> {
        // Compute SHA256 of signing public key to get 32-byte operator ID
        sha256(&self.identity.sign_pub()).to_vec()
    }

    /// Send a pair request via the configured transport
    /// Requirements: 2.3
    pub async fn send_pair_request(
        &mut self,
        invite_secret: &[u8; 32],
        requested_permissions: u32,
    ) -> Result<PairRequestV1, PairingError> {
        // Generate the pair request
        let request = self.generate_pair_request(invite_secret, requested_permissions)?;

        // Get the device ID from the invite
        let device_id = match &self.state {
            PairingState::RequestSent { invite, .. } => invite.invite.device_id.clone(),
            _ => {
                return Err(PairingError::InvalidState(
                    "Request not generated".to_string(),
                ));
            }
        };

        // Send via transport
        self.transport
            .send_pair_request(&device_id, &request, self.transport_preference)
            .await?;

        Ok(request)
    }

    /// Wait for a PairReceiptV1 response from the device
    /// Requirements: 2.4
    pub async fn wait_for_receipt(&mut self) -> Result<PairReceiptV1, PairingError> {
        // Check we're in the right state
        let _invite = match &self.state {
            PairingState::RequestSent { invite, .. } => invite.clone(),
            _ => {
                return Err(PairingError::InvalidState(
                    "Must send request before waiting for receipt".to_string(),
                ));
            }
        };

        // Poll for response
        let operator_id = self.operator_id_bytes();
        let response = self
            .transport
            .poll_response(&operator_id, self.timeout)
            .await?;

        match response {
            Some(data) => {
                // Decode the receipt
                let receipt = PairReceiptV1::decode(data.as_slice())
                    .map_err(|e| PairingError::ProtobufDecode(e.to_string()))?;
                Ok(receipt)
            }
            None => Err(PairingError::Timeout(self.timeout)),
        }
    }

    /// Parse invite from base64-encoded string
    fn parse_base64(&self, data: &str) -> Result<(Vec<u8>, InviteV1), PairingError> {
        // Try standard base64 first, then URL-safe
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(data.trim()))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data.trim()))
            .map_err(|e| PairingError::Base64Decode(e.to_string()))?;

        let invite = InviteV1::decode(decoded.as_slice())
            .map_err(|e| PairingError::ProtobufDecode(e.to_string()))?;

        Ok((decoded, invite))
    }

    /// Parse invite from file (JSON or binary protobuf)
    fn parse_file(&self, path: &PathBuf) -> Result<(Vec<u8>, InviteV1), PairingError> {
        let mut file = fs::File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        // Try to detect format: JSON starts with '{' or whitespace then '{'
        let trimmed = contents.iter().skip_while(|&&b| b.is_ascii_whitespace()).copied().collect::<Vec<_>>();
        
        if trimmed.first() == Some(&b'{') {
            // Try JSON format
            let json_str = String::from_utf8(contents.clone())
                .map_err(|e| PairingError::JsonParse(format!("Invalid UTF-8: {e}")))?;
            let invite_json: InviteJson = serde_json::from_str(&json_str)
                .map_err(|e| PairingError::JsonParse(e.to_string()))?;
            let invite: InviteV1 = invite_json.try_into()?;
            
            // Re-encode to protobuf for raw bytes
            let raw = invite.encode_to_vec();
            Ok((raw, invite))
        } else {
            // Try binary protobuf format
            let invite = InviteV1::decode(contents.as_slice())
                .map_err(|e| PairingError::ProtobufDecode(e.to_string()))?;
            Ok((contents, invite))
        }
    }

    /// Parse invite from QR code image file
    #[cfg(feature = "qr")]
    fn parse_qr_image(&self, path: &PathBuf) -> Result<(Vec<u8>, InviteV1), PairingError> {
        use image::GenericImageView;

        // Load the image
        let img = image::open(path)
            .map_err(|e| PairingError::QrCode(format!("Failed to open image: {e}")))?;

        // Convert to grayscale for QR detection
        let gray = img.to_luma8();
        let (width, height) = gray.dimensions();

        // Prepare image for rqrr
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
            width as usize,
            height as usize,
            |x, y| gray.get_pixel(x as u32, y as u32).0[0],
        );

        // Find and decode QR codes
        let grids = prepared.detect_grids();
        if grids.is_empty() {
            return Err(PairingError::QrCode("No QR code found in image".to_string()));
        }

        // Decode the first QR code found
        let (_, content) = grids[0]
            .decode()
            .map_err(|e| PairingError::QrCode(format!("Failed to decode QR: {e:?}")))?;

        // The QR code should contain base64-encoded invite data
        self.parse_base64(&content)
    }

    /// Parse invite from QR code image file (stub when feature not enabled)
    #[cfg(not(feature = "qr"))]
    fn parse_qr_image(&self, _path: &PathBuf) -> Result<(Vec<u8>, InviteV1), PairingError> {
        Err(PairingError::QrCode(
            "QR code support not enabled. Rebuild with --features qr".to_string(),
        ))
    }

    /// Validate an invite
    fn validate_invite(&self, invite: &InviteV1) -> Result<(), PairingError> {
        // Use the proto validation trait
        invite.validate().map_err(|e| PairingError::InvalidInvite(e.to_string()))?;

        // Additional check: ensure not expired (validation trait checks this, but we want a specific error)
        let expires_at = UNIX_EPOCH + Duration::from_secs(invite.expires_at);
        if expires_at < SystemTime::now() {
            return Err(PairingError::InviteExpired(expires_at));
        }

        Ok(())
    }

    /// Convert InviteV1 to ParsedInvite
    fn to_parsed_invite(&self, raw: Vec<u8>, invite: InviteV1) -> ParsedInvite {
        let device_id = hex::encode(&invite.device_id);
        let expires_at = UNIX_EPOCH + Duration::from_secs(invite.expires_at);

        // Extract transport hints as human-readable strings
        let transport_hints = self.extract_transport_hints(&invite);

        ParsedInvite {
            device_id,
            expires_at,
            transport_hints,
            raw,
            invite,
        }
    }

    /// Extract transport hints as human-readable strings
    fn extract_transport_hints(&self, invite: &InviteV1) -> Vec<String> {
        let mut hints = Vec::new();

        if let Some(ref transport) = invite.transport_hints {
            for addr in &transport.direct_addrs {
                hints.push(format!("direct: {addr}"));
            }
            for url in &transport.rendezvous_urls {
                hints.push(format!("rendezvous: {url}"));
            }
            for hint in &transport.mesh_hints {
                hints.push(format!("mesh: {hint}"));
            }
            for relay in &transport.relay_tokens {
                hints.push(format!("relay: {}", hex::encode(&relay.relay_id)));
            }
        }

        if hints.is_empty() {
            hints.push("none".to_string());
        }

        hints
    }

    /// Execute pairing flow
    /// Requirements: 2.1-2.8
    pub async fn pair(
        &mut self,
        invite: &ParsedInvite,
        invite_secret: &[u8; 32],
        options: PairOptions,
    ) -> Result<PairingResult, PairingError> {
        // Set timeout from options
        self.timeout = options.timeout;
        self.started_at = Some(SystemTime::now());

        // Import the invite if not already imported
        if !matches!(self.state, PairingState::InviteImported { .. }) {
            self.state = PairingState::InviteImported {
                invite: invite.clone(),
                invite_secret: Some(*invite_secret),
            };
        }

        // Generate pair request
        let permissions_mask = Self::permissions_to_mask(&options.requested_permissions);
        let _request = self.generate_pair_request(invite_secret, permissions_mask)?;

        // The actual sending and receipt handling would be done by the transport layer
        // For now, we return the request and let the caller handle transport
        // This is a placeholder for the full flow
        Err(PairingError::Transport(
            "Transport sending must be implemented by caller - use generate_pair_request() and handle_receipt()".to_string(),
        ))
    }

    /// Convert permission strings to bitmask
    fn permissions_to_mask(permissions: &[String]) -> u32 {
        let mut mask = 0u32;
        for perm in permissions {
            match perm.to_lowercase().as_str() {
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

    /// Handle a PairReceiptV1 from the device
    /// Requirements: 2.4, 2.5
    pub fn handle_receipt(&mut self, receipt: PairReceiptV1) -> Result<String, PairingError> {
        // Check timeout
        self.check_timeout()?;

        // Validate state and extract request + invite
        let (request, invite) = match &self.state {
            PairingState::RequestSent { request, invite } => (request.clone(), invite.clone()),
            _ => {
                return Err(PairingError::InvalidState(
                    "Must send request before handling receipt".to_string(),
                ));
            }
        };

        // Verify receipt is for this operator
        let our_operator_id = self.operator_id_bytes();
        if receipt.operator_id != our_operator_id {
            return Err(PairingError::InvalidState(
                "Receipt not for this operator".to_string(),
            ));
        }

        // Verify receipt is from the expected device
        if receipt.device_id != invite.invite.device_id {
            return Err(PairingError::InvalidState(
                "Receipt from unexpected device".to_string(),
            ));
        }

        // Verify device signature
        self.verify_receipt_signature(&receipt, &invite.invite.device_sign_pub)?;

        // Compute SAS for user verification
        let user_id = UserIdV1 {
            id: our_operator_id,
        };
        let device_id = DeviceIdV1 {
            id: invite.invite.device_id.clone(),
        };
        let created_at = TimestampV1 {
            unix_seconds: request.timestamp,
        };

        let op_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: self.identity.sign_pub().to_vec(),
        };
        let op_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: self.identity.kex_pub().to_vec(),
        };

        // Build canonical fields for SAS computation
        let fields_wo_proof = canonical_pair_request_fields_without_proof_v1(
            &user_id,
            &op_sign_pub,
            &op_kex_pub,
            &device_id,
            &created_at,
            true, // request_sas = true since we have a nonce
        );

        // Compute SAS transcript
        let sas_transcript = pairing_sas_transcript_v1(
            &fields_wo_proof,
            &self.identity.sign_pub(),
            &invite.invite.device_sign_pub,
            request.timestamp,
            invite.invite.expires_at,
        );

        // Compute 6-digit SAS code
        let sas = compute_pairing_sas_6digit_v1(&sas_transcript);

        // Transition to AwaitingSAS state
        self.state = PairingState::AwaitingSAS {
            sas: sas.clone(),
            receipt,
            invite,
        };

        Ok(sas)
    }

    /// Verify the device signature on a PairReceiptV1
    fn verify_receipt_signature(
        &self,
        receipt: &PairReceiptV1,
        device_sign_pub: &[u8],
    ) -> Result<(), PairingError> {
        use ed25519_dalek::{Signature, VerifyingKey};

        if receipt.device_signature.len() != 64 {
            return Err(PairingError::SignatureInvalid(
                "Invalid signature length".to_string(),
            ));
        }
        if device_sign_pub.len() != 32 {
            return Err(PairingError::SignatureInvalid(
                "Invalid public key length".to_string(),
            ));
        }

        // Get the bytes that were signed (receipt without signature)
        let mut receipt_copy = receipt.clone();
        receipt_copy.device_signature = vec![];
        let bytes = receipt_copy.encode_to_vec();
        let digest = sha256(&bytes);

        // Parse the public key
        let pub_key_bytes: [u8; 32] = device_sign_pub
            .try_into()
            .map_err(|_| PairingError::SignatureInvalid("Invalid public key length".to_string()))?;
        let verifying_key = VerifyingKey::from_bytes(&pub_key_bytes)
            .map_err(|e| PairingError::SignatureInvalid(format!("Invalid public key: {e}")))?;

        // Parse the signature
        let sig_bytes: [u8; 64] = receipt.device_signature[..64]
            .try_into()
            .map_err(|_| PairingError::SignatureInvalid("Invalid signature length".to_string()))?;
        let signature = Signature::from_bytes(&sig_bytes);

        // Verify
        verifying_key
            .verify_strict(&digest, &signature)
            .map_err(|e| PairingError::SignatureInvalid(format!("Verification failed: {e}")))?;

        Ok(())
    }

    /// Verify SAS code with user
    /// Requirements: 2.5
    pub fn verify_sas(&self, _sas: &str) -> Result<bool, PairingError> {
        // Get the expected SAS from state
        match &self.state {
            PairingState::AwaitingSAS { sas, .. } => {
                // In a real implementation, this would compare with user input
                // For now, we just return the SAS for display
                Ok(!sas.is_empty())
            }
            _ => Err(PairingError::InvalidState(
                "Not in SAS verification state".to_string(),
            )),
        }
    }

    /// Get the current SAS code if in AwaitingSAS state
    pub fn get_sas(&self) -> Option<&str> {
        match &self.state {
            PairingState::AwaitingSAS { sas, .. } => Some(sas),
            _ => None,
        }
    }

    /// Confirm SAS verification and complete pairing
    /// Requirements: 2.5, 2.6
    pub async fn confirm_sas(&mut self) -> Result<PairingResult, PairingError> {
        // Check timeout
        self.check_timeout()?;

        // Validate state and extract receipt + invite
        let (receipt, invite) = match &self.state {
            PairingState::AwaitingSAS { receipt, invite, .. } => (receipt.clone(), invite.clone()),
            _ => {
                return Err(PairingError::InvalidState(
                    "Must be in SAS verification state".to_string(),
                ));
            }
        };

        // Store the pairing
        self.store_pairing(&receipt, &invite).await?;

        // Build result
        let result = PairingResult {
            device_id: hex::encode(&receipt.device_id),
            permissions_granted: Self::mask_to_permissions(receipt.permissions_granted),
            paired_at: UNIX_EPOCH + Duration::from_secs(receipt.paired_at),
            sas_verified: true,
        };

        // Transition to Paired state
        self.state = PairingState::Paired {
            device_id: result.device_id.clone(),
            permissions: receipt.permissions_granted,
        };

        // Clear timeout tracking
        self.started_at = None;

        Ok(result)
    }

    /// Reject SAS verification
    pub fn reject_sas(&mut self) -> Result<(), PairingError> {
        match &self.state {
            PairingState::AwaitingSAS { .. } => {
                self.state = PairingState::Failed {
                    reason: "SAS verification rejected by user".to_string(),
                };
                self.started_at = None;
                Ok(())
            }
            _ => Err(PairingError::InvalidState(
                "Not in SAS verification state".to_string(),
            )),
        }
    }

    /// Store a successful pairing
    /// Requirements: 2.6
    async fn store_pairing(
        &self,
        receipt: &PairReceiptV1,
        invite: &ParsedInvite,
    ) -> Result<(), PairingError> {
        // Build pairing record for zrc-core store
        let device_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: invite.invite.device_sign_pub.clone(),
        };
        let device_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: vec![0u8; 32], // Placeholder - would be exchanged during pairing
        };
        let operator_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: self.identity.sign_pub().to_vec(),
        };
        let operator_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: self.identity.kex_pub().to_vec(),
        };

        let pairing_record = PairingRecord {
            pairing_id: receipt.session_binding.clone(),
            device_id: receipt.device_id.clone(),
            operator_id: receipt.operator_id.clone(),
            device_sign_pub,
            device_kex_pub,
            operator_sign_pub,
            operator_kex_pub,
            granted_perms: vec![receipt.permissions_granted as i32],
            unattended_enabled: (receipt.permissions_granted & 0x20) != 0,
            require_consent_each_time: false,
            issued_at: receipt.paired_at,
            last_session: None,
        };

        // Save to in-memory store
        self.memory_store
            .save_pairing(pairing_record)
            .await
            .map_err(|e| PairingError::Storage(e.to_string()))?;

        // Also save to pairings store if available
        if let Some(ref store) = self.pairings_store {
            let stored_pairing = StoredPairing {
                device_id: hex::encode(&receipt.device_id),
                device_name: None,
                device_sign_pub: invite.invite.device_sign_pub.clone().try_into().unwrap_or([0u8; 32]),
                device_kex_pub: [0u8; 32], // Placeholder
                permissions: Self::mask_to_permissions(receipt.permissions_granted),
                paired_at: UNIX_EPOCH + Duration::from_secs(receipt.paired_at),
                last_session: None,
                session_count: 0,
            };
            store
                .store(stored_pairing)
                .map_err(|e| PairingError::Storage(e.to_string()))?;
        }

        Ok(())
    }

    /// Convert permission mask to string list
    fn mask_to_permissions(mask: u32) -> Vec<String> {
        let mut perms = Vec::new();
        if mask & 0x01 != 0 {
            perms.push("view".to_string());
        }
        if mask & 0x02 != 0 {
            perms.push("control".to_string());
        }
        if mask & 0x04 != 0 {
            perms.push("clipboard".to_string());
        }
        if mask & 0x08 != 0 {
            perms.push("file_transfer".to_string());
        }
        if mask & 0x10 != 0 {
            perms.push("audio".to_string());
        }
        if mask & 0x20 != 0 {
            perms.push("unattended".to_string());
        }
        perms
    }

    /// Check if the pairing has timed out
    fn check_timeout(&mut self) -> Result<(), PairingError> {
        if let Some(started) = self.started_at {
            if let Ok(elapsed) = started.elapsed() {
                if elapsed > self.timeout {
                    self.state = PairingState::Failed {
                        reason: format!("Pairing timed out after {:?}", self.timeout),
                    };
                    self.started_at = None;
                    return Err(PairingError::Timeout(self.timeout));
                }
            }
        }
        Ok(())
    }

    /// Reset the pairing state machine
    pub fn reset(&mut self) {
        self.state = PairingState::Idle;
        self.started_at = None;
    }

    /// Check if pairing is complete
    pub fn is_paired(&self) -> bool {
        matches!(self.state, PairingState::Paired { .. })
    }

    /// Get the paired device ID if pairing is complete
    pub fn paired_device_id(&self) -> Option<&str> {
        match &self.state {
            PairingState::Paired { device_id, .. } => Some(device_id),
            _ => None,
        }
    }
}

impl Default for PairingClient {
    fn default() -> Self {
        Self::new()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_invite(expires_in_secs: u64) -> InviteV1 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        InviteV1 {
            device_id: vec![0u8; 32],
            device_sign_pub: vec![1u8; 32],
            invite_secret_hash: vec![2u8; 32],
            expires_at: now + expires_in_secs,
            transport_hints: Some(EndpointHintsV1 {
                direct_addrs: vec!["192.168.1.100:5000".to_string()],
                rendezvous_urls: vec!["https://rendezvous.example.com".to_string()],
                mesh_hints: vec![],
                relay_tokens: vec![],
            }),
        }
    }

    #[test]
    fn test_base64_parsing() {
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let result = client.import_invite(InviteSource::Base64(base64_str));
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.device_id, hex::encode(&invite.device_id));
        assert!(!parsed.is_expired());
    }

    #[test]
    fn test_base64_url_safe_parsing() {
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::URL_SAFE.encode(&encoded);

        let result = client.import_invite(InviteSource::Base64(base64_str));
        assert!(result.is_ok());
    }

    #[test]
    fn test_expired_invite() {
        let mut client = PairingClient::new();
        
        // Create an already-expired invite
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let invite = InviteV1 {
            device_id: vec![0u8; 32],
            device_sign_pub: vec![1u8; 32],
            invite_secret_hash: vec![2u8; 32],
            expires_at: now - 100, // Expired 100 seconds ago
            transport_hints: None,
        };

        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let result = client.import_invite(InviteSource::Base64(base64_str));
        // Should fail with either InviteExpired or InvalidInvite (from validation)
        assert!(matches!(result, Err(PairingError::InviteExpired(_)) | Err(PairingError::InvalidInvite(_))));
    }

    #[test]
    fn test_invalid_base64() {
        let mut client = PairingClient::new();
        let result = client.import_invite(InviteSource::Base64("not-valid-base64!!!".to_string()));
        assert!(matches!(result, Err(PairingError::Base64Decode(_))));
    }

    #[test]
    fn test_invalid_protobuf() {
        let mut client = PairingClient::new();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(b"not a protobuf");
        let result = client.import_invite(InviteSource::Base64(base64_str));
        // This might decode but fail validation due to wrong field sizes
        assert!(result.is_err());
    }

    #[test]
    fn test_json_conversion() {
        let invite = create_test_invite(3600);
        let json: InviteJson = (&invite).into();
        
        assert_eq!(json.device_id, hex::encode(&invite.device_id));
        assert_eq!(json.expires_at, invite.expires_at);
        
        // Convert back
        let back: InviteV1 = json.try_into().unwrap();
        assert_eq!(back.device_id, invite.device_id);
        assert_eq!(back.expires_at, invite.expires_at);
    }

    #[test]
    fn test_transport_hints_extraction() {
        let client = PairingClient::new();
        let invite = create_test_invite(3600);
        
        let hints = client.extract_transport_hints(&invite);
        assert!(hints.iter().any(|h| h.starts_with("direct:")));
        assert!(hints.iter().any(|h| h.starts_with("rendezvous:")));
    }

    #[test]
    fn test_parsed_invite_expiry_check() {
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let parsed = client.import_invite(InviteSource::Base64(base64_str)).unwrap();
        
        assert!(!parsed.is_expired());
        assert!(parsed.time_until_expiry().is_some());
        assert!(parsed.time_until_expiry().unwrap().as_secs() > 3500);
    }

    #[test]
    fn test_file_parsing_binary() {
        use std::io::Write;
        
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let encoded = invite.encode_to_vec();
        
        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_invite.bin");
        let mut file = fs::File::create(&temp_file).unwrap();
        file.write_all(&encoded).unwrap();
        drop(file);
        
        // Parse from file
        let result = client.import_invite(InviteSource::File(temp_file.clone()));
        assert!(result.is_ok());
        
        let parsed = result.unwrap();
        assert_eq!(parsed.device_id, hex::encode(&invite.device_id));
        
        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_file_parsing_json() {
        use std::io::Write;
        
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let invite_json: InviteJson = (&invite).into();
        let json_str = serde_json::to_string_pretty(&invite_json).unwrap();
        
        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_invite.json");
        let mut file = fs::File::create(&temp_file).unwrap();
        file.write_all(json_str.as_bytes()).unwrap();
        drop(file);
        
        // Parse from file
        let result = client.import_invite(InviteSource::File(temp_file.clone()));
        assert!(result.is_ok());
        
        let parsed = result.unwrap();
        assert_eq!(parsed.device_id, hex::encode(&invite.device_id));
        
        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_file_not_found() {
        let mut client = PairingClient::new();
        let result = client.import_invite(InviteSource::File(PathBuf::from("/nonexistent/path/invite.bin")));
        assert!(matches!(result, Err(PairingError::Io(_))));
    }

    #[test]
    fn test_qr_code_not_enabled() {
        // When QR feature is not enabled, should return appropriate error
        #[cfg(not(feature = "qr"))]
        {
            let mut client = PairingClient::new();
            let result = client.import_invite(InviteSource::QrImage(PathBuf::from("test.png")));
            assert!(matches!(result, Err(PairingError::QrCode(_))));
        }
    }

    #[cfg(feature = "qr")]
    #[test]
    fn test_qr_code_file_not_found() {
        let mut client = PairingClient::new();
        let result = client.import_invite(InviteSource::QrImage(PathBuf::from("/nonexistent/qr.png")));
        assert!(matches!(result, Err(PairingError::QrCode(_))));
    }

    #[test]
    fn test_validation_invalid_device_id_size() {
        let mut client = PairingClient::new();
        
        // Create invite with invalid device_id size (should be 32 bytes)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let invite = InviteV1 {
            device_id: vec![0u8; 16], // Wrong size!
            device_sign_pub: vec![1u8; 32],
            invite_secret_hash: vec![2u8; 32],
            expires_at: now + 3600,
            transport_hints: None,
        };

        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let result = client.import_invite(InviteSource::Base64(base64_str));
        assert!(matches!(result, Err(PairingError::InvalidInvite(_))));
    }

    #[test]
    fn test_validation_invalid_sign_pub_size() {
        let mut client = PairingClient::new();
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let invite = InviteV1 {
            device_id: vec![0u8; 32],
            device_sign_pub: vec![1u8; 16], // Wrong size!
            invite_secret_hash: vec![2u8; 32],
            expires_at: now + 3600,
            transport_hints: None,
        };

        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let result = client.import_invite(InviteSource::Base64(base64_str));
        assert!(matches!(result, Err(PairingError::InvalidInvite(_))));
    }

    #[test]
    fn test_parsed_invite_display_fields() {
        let mut client = PairingClient::new();
        let invite = create_test_invite(3600);
        let encoded = invite.encode_to_vec();
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&encoded);

        let parsed = client.import_invite(InviteSource::Base64(base64_str)).unwrap();
        
        // Verify device_id is hex-encoded
        assert_eq!(parsed.device_id.len(), 64); // 32 bytes = 64 hex chars
        assert!(parsed.device_id.chars().all(|c| c.is_ascii_hexdigit()));
        
        // Verify expires_at is in the future
        assert!(parsed.expires_at > SystemTime::now());
        
        // Verify transport_hints are populated
        assert!(!parsed.transport_hints.is_empty());
        assert!(parsed.transport_hints.iter().any(|h| h.contains("direct")));
        assert!(parsed.transport_hints.iter().any(|h| h.contains("rendezvous")));
    }

    // Transport ladder tests (Requirements: 8.1-8.3)

    #[test]
    fn test_transport_preference_from_str() {
        assert_eq!("auto".parse::<TransportPreference>().unwrap(), TransportPreference::Auto);
        assert_eq!("mesh".parse::<TransportPreference>().unwrap(), TransportPreference::Mesh);
        assert_eq!("rendezvous".parse::<TransportPreference>().unwrap(), TransportPreference::Rendezvous);
        assert_eq!("direct".parse::<TransportPreference>().unwrap(), TransportPreference::Direct);
        assert_eq!("relay".parse::<TransportPreference>().unwrap(), TransportPreference::Relay);
        
        // Case insensitive
        assert_eq!("AUTO".parse::<TransportPreference>().unwrap(), TransportPreference::Auto);
        assert_eq!("Mesh".parse::<TransportPreference>().unwrap(), TransportPreference::Mesh);
        
        // Invalid
        assert!("invalid".parse::<TransportPreference>().is_err());
    }

    #[test]
    fn test_transport_preference_display() {
        assert_eq!(TransportPreference::Auto.to_string(), "auto");
        assert_eq!(TransportPreference::Mesh.to_string(), "mesh");
        assert_eq!(TransportPreference::Rendezvous.to_string(), "rendezvous");
        assert_eq!(TransportPreference::Direct.to_string(), "direct");
        assert_eq!(TransportPreference::Relay.to_string(), "relay");
    }

    #[test]
    fn test_transport_ladder_order() {
        // Requirements: 8.3 - Transport ladder order: mesh → direct → rendezvous → relay
        let ladder = TransportPreference::ladder_order();
        
        assert_eq!(ladder.len(), 4);
        assert_eq!(ladder[0], TransportPreference::Mesh);
        assert_eq!(ladder[1], TransportPreference::Direct);
        assert_eq!(ladder[2], TransportPreference::Rendezvous);
        assert_eq!(ladder[3], TransportPreference::Relay);
    }

    #[test]
    fn test_transport_client_available_transports() {
        // With default config (rendezvous and relay configured)
        let client = TransportClient::new();
        let available = client.available_transports();
        
        // Should have direct (always), rendezvous, and relay
        assert!(available.contains(&TransportPreference::Direct));
        assert!(available.contains(&TransportPreference::Rendezvous));
        assert!(available.contains(&TransportPreference::Relay));
        // Mesh not configured by default
        assert!(!available.contains(&TransportPreference::Mesh));
    }

    #[test]
    fn test_transport_client_with_mesh() {
        let client = TransportClient::with_urls(
            vec!["https://rendezvous.example.com".to_string()],
            vec!["https://relay.example.com".to_string()],
            vec!["mesh.example.com:5000".to_string()],
        );
        let available = client.available_transports();
        
        // Should have all transports
        assert!(available.contains(&TransportPreference::Mesh));
        assert!(available.contains(&TransportPreference::Direct));
        assert!(available.contains(&TransportPreference::Rendezvous));
        assert!(available.contains(&TransportPreference::Relay));
    }

    #[test]
    fn test_transport_client_display_ladder_info() {
        let client = TransportClient::new();
        let info = client.display_ladder_info();
        
        // Should contain all transport types in order
        assert!(info.contains("mesh"));
        assert!(info.contains("direct"));
        assert!(info.contains("rendezvous"));
        assert!(info.contains("relay"));
        
        // Should indicate configuration status
        assert!(info.contains("configured") || info.contains("not configured"));
    }

    #[test]
    fn test_transport_preference_all_values() {
        let values = TransportPreference::all_values();
        
        assert_eq!(values.len(), 5);
        assert!(values.contains(&"auto"));
        assert!(values.contains(&"mesh"));
        assert!(values.contains(&"rendezvous"));
        assert!(values.contains(&"direct"));
        assert!(values.contains(&"relay"));
    }
}
