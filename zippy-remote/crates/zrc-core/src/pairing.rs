//! Pairing state machines for ZRC.
//!
//! This module implements the pairing workflow state machines for both
//! host (device) and controller (operator) sides.
//!
//! Requirements: 1.1-1.8, 2.1-2.8

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use ed25519_dalek::{Signer, Signature, VerifyingKey};
use getrandom::getrandom;
use prost::Message;

use crate::{
    errors::CoreError,
    rate_limit::{RateLimiter, RequestType},
    store::{InviteRecord, MemoryStore, PairingRecord, Store},
    types::{IdentityKeys, Outgoing},
};
use zrc_crypto::{
    hash::sha256,
    pairing::{
        canonical_pair_request_fields_without_proof_v1, compute_pair_proof_v1,
        compute_pairing_sas_6digit_v1, pair_proof_input_v1, pairing_sas_transcript_v1,
    },
};
use zrc_proto::v1::{
    DeviceIdV1, EndpointHintsV1, InviteV1, KeyTypeV1, PairReceiptV1, PairRequestV1, PermissionV1,
    PublicKeyV1, TimestampV1, UserIdV1,
};

// ============================================================================
// Error Types
// ============================================================================

/// Errors specific to pairing operations.
#[derive(Debug, Clone)]
pub enum PairingError {
    /// Invite has expired
    InviteExpired,
    /// Invalid invite proof (HMAC verification failed)
    InvalidProof,
    /// No active invite exists
    NoActiveInvite,
    /// Invalid state transition
    InvalidState(String),
    /// Rate limit exceeded
    RateLimited { retry_after_secs: u64 },
    /// Cryptographic operation failed
    CryptoError(String),
    /// Missing required field
    MissingField(String),
    /// Signature verification failed
    SignatureInvalid,
    /// Pairing was rejected by user
    Rejected,
    /// Timeout occurred
    Timeout,
    /// Store operation failed
    StoreError(String),
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairingError::InviteExpired => write!(f, "invite has expired"),
            PairingError::InvalidProof => write!(f, "invalid invite proof"),
            PairingError::NoActiveInvite => write!(f, "no active invite"),
            PairingError::InvalidState(s) => write!(f, "invalid state: {}", s),
            PairingError::RateLimited { retry_after_secs } => {
                write!(f, "rate limited, retry after {} seconds", retry_after_secs)
            }
            PairingError::CryptoError(s) => write!(f, "crypto error: {}", s),
            PairingError::MissingField(s) => write!(f, "missing field: {}", s),
            PairingError::SignatureInvalid => write!(f, "signature verification failed"),
            PairingError::Rejected => write!(f, "pairing rejected by user"),
            PairingError::Timeout => write!(f, "pairing timeout"),
            PairingError::StoreError(s) => write!(f, "store error: {}", s),
        }
    }
}

impl std::error::Error for PairingError {}

// ============================================================================
// Consent Handler Trait
// ============================================================================

/// Decision returned by the consent handler.
#[derive(Clone, Debug)]
pub struct PairDecision {
    /// Whether pairing was approved
    pub approved: bool,
    /// Granted permissions (if approved)
    pub granted_perms: Vec<PermissionV1>,
    /// Whether unattended access is enabled
    pub unattended_enabled: bool,
    /// Whether consent is required for each session
    pub require_consent_each_time: bool,
}

/// Trait for handling pairing consent decisions.
/// Requirements: 1.5
#[async_trait]
pub trait ConsentHandler: Send + Sync {
    /// Called when a pairing request needs user approval.
    async fn request_consent(
        &self,
        operator_id: &[u8],
        sas: Option<&str>,
    ) -> Result<PairDecision, PairingError>;
}

// ============================================================================
// Pairing Host State Machine
// ============================================================================

/// State of the pairing host state machine.
/// Requirements: 1.1
#[derive(Clone, Debug)]
pub enum PairingHostState {
    /// Initial state, ready to generate invite
    Idle,
    /// Invite has been generated and is waiting for a request
    InviteGenerated {
        invite: InviteV1,
        secret: [u8; 32],
        expires_at: u64,
    },
    /// Invite sent out-of-band, awaiting pair request
    AwaitingRequest {
        invite: InviteV1,
        secret: [u8; 32],
    },
    /// Request received, awaiting user approval
    AwaitingApproval {
        request: PairRequestV1,
        operator_pub: PublicKeyV1,
        sas: Option<String>,
    },
    /// Pairing completed successfully
    Paired {
        operator_id: [u8; 32],
        permissions: u32,
    },
    /// Pairing failed
    Failed { reason: PairingError },
}

/// Action returned by pairing operations.
#[derive(Clone, Debug)]
pub enum PairingAction {
    /// Waiting for user consent
    AwaitingConsent {
        sas: Option<String>,
        operator_id: Vec<u8>,
    },
    /// Pairing was auto-approved
    AutoApproved { receipt: PairReceiptV1 },
    /// Pairing was rejected
    Rejected { reason: String },
}


/// Host-side pairing state machine.
/// Requirements: 1.1-1.8
pub struct PairingHost<S: Store, C: ConsentHandler> {
    /// Current state of the state machine
    state: PairingHostState,
    /// Device identity keys
    device_keys: IdentityKeys,
    /// Persistent storage
    store: Arc<S>,
    /// Consent handler for user approval
    consent_handler: Arc<C>,
    /// Rate limiter for protection
    rate_limiter: RateLimiter,
}

impl<S: Store, C: ConsentHandler> PairingHost<S, C> {
    /// Create a new pairing host state machine.
    pub fn new(device_keys: IdentityKeys, store: Arc<S>, consent_handler: Arc<C>) -> Self {
        Self {
            state: PairingHostState::Idle,
            device_keys,
            store,
            consent_handler,
            rate_limiter: RateLimiter::default(),
        }
    }

    /// Create a new pairing host with custom rate limiter.
    pub fn with_rate_limiter(
        device_keys: IdentityKeys,
        store: Arc<S>,
        consent_handler: Arc<C>,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self {
            state: PairingHostState::Idle,
            device_keys,
            store,
            consent_handler,
            rate_limiter,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &PairingHostState {
        &self.state
    }

    /// Generate a new invite for pairing.
    /// Requirements: 1.2
    pub async fn generate_invite(
        &mut self,
        ttl_seconds: u32,
        transport_hints: Option<EndpointHintsV1>,
    ) -> Result<InviteV1, PairingError> {
        // Validate state transition
        match &self.state {
            PairingHostState::Idle | PairingHostState::Failed { .. } => {}
            _ => {
                return Err(PairingError::InvalidState(
                    "can only generate invite from Idle or Failed state".into(),
                ));
            }
        }

        // Generate random invite secret (32 bytes)
        let mut secret = [0u8; 32];
        getrandom(&mut secret).map_err(|_| PairingError::CryptoError("RNG failed".into()))?;

        // Compute invite_secret_hash = SHA256(secret)
        let secret_hash = sha256(&secret);

        // Calculate expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now + ttl_seconds as u64;

        // Build invite
        let invite = InviteV1 {
            device_id: self.device_keys.id32.to_vec(),
            device_sign_pub: self.device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at,
            transport_hints,
        };

        // Store invite and secret
        let invite_record = InviteRecord {
            device_id: self.device_keys.id32.to_vec(),
            invite_secret: secret,
            expires_at_unix: expires_at,
        };
        self.store
            .save_invite(invite_record)
            .await
            .map_err(|e| PairingError::StoreError(e.to_string()))?;

        // Transition state
        self.state = PairingHostState::InviteGenerated {
            invite: invite.clone(),
            secret,
            expires_at,
        };

        Ok(invite)
    }

    /// Handle an incoming pair request.
    /// Requirements: 1.3, 1.4, 1.8
    pub async fn handle_request(
        &mut self,
        request: PairRequestV1,
        source: &str,
    ) -> Result<PairingAction, PairingError> {
        // Check rate limit (Requirements: 1.8)
        self.rate_limiter
            .check_rate_limit(source, RequestType::Pairing)
            .await
            .map_err(|e| match e {
                crate::rate_limit::RateLimitError::RateLimited {
                    retry_after_secs, ..
                } => PairingError::RateLimited { retry_after_secs },
            })?;

        // Get current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Get invite from state or store
        let (invite, secret) = match &self.state {
            PairingHostState::InviteGenerated {
                invite,
                secret,
                expires_at,
            } => {
                if *expires_at <= now {
                    self.state = PairingHostState::Failed {
                        reason: PairingError::InviteExpired,
                    };
                    return Err(PairingError::InviteExpired);
                }
                (invite.clone(), *secret)
            }
            PairingHostState::AwaitingRequest { invite, secret } => (invite.clone(), *secret),
            PairingHostState::Idle => {
                // Try to load from store
                let device_id = &self.device_keys.id32;
                match self.store.load_invite(device_id).await {
                    Ok(Some(record)) => {
                        if record.expires_at_unix <= now {
                            return Err(PairingError::InviteExpired);
                        }
                        let invite = InviteV1 {
                            device_id: record.device_id.clone(),
                            device_sign_pub: self.device_keys.sign_pub.key_bytes.clone(),
                            invite_secret_hash: sha256(&record.invite_secret).to_vec(),
                            expires_at: record.expires_at_unix,
                            transport_hints: None,
                        };
                        (invite, record.invite_secret)
                    }
                    Ok(None) => return Err(PairingError::NoActiveInvite),
                    Err(e) => return Err(PairingError::StoreError(e.to_string())),
                }
            }
            _ => {
                return Err(PairingError::InvalidState(
                    "cannot handle request in current state".into(),
                ));
            }
        };

        // Validate required fields (now Vec<u8>, check if empty)
        if request.operator_id.is_empty() {
            return Err(PairingError::MissingField("operator_id".into()));
        }
        if request.operator_sign_pub.is_empty() {
            return Err(PairingError::MissingField("operator_sign_pub".into()));
        }
        if request.operator_kex_pub.is_empty() {
            return Err(PairingError::MissingField("operator_kex_pub".into()));
        }

        // Build device_id for proof verification
        let device_id = DeviceIdV1 {
            id: self.device_keys.id32.to_vec(),
        };
        let user_id = UserIdV1 {
            id: request.operator_id.clone(),
        };
        let created_at = TimestampV1 {
            unix_seconds: request.timestamp,
        };

        // Convert operator keys to PublicKeyV1 format
        let op_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: request.operator_sign_pub.clone(),
        };
        let op_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: request.operator_kex_pub.clone(),
        };

        // Verify invite_proof (Requirements: 1.3)
        let proof_input =
            pair_proof_input_v1(&user_id, &op_sign_pub, &op_kex_pub, &device_id, &created_at);
        let expected_proof = compute_pair_proof_v1(&secret, &proof_input);

        if request.invite_proof != expected_proof {
            // Requirements: 1.4 - reject and return to Idle
            self.state = PairingHostState::Failed {
                reason: PairingError::InvalidProof,
            };
            return Err(PairingError::InvalidProof);
        }

        // Compute SAS if nonce is provided
        let sas = if request.nonce.len() == 32 {
            let fields_wo_proof = canonical_pair_request_fields_without_proof_v1(
                &user_id,
                &op_sign_pub,
                &op_kex_pub,
                &device_id,
                &created_at,
                true,
            );
            let sas_transcript = pairing_sas_transcript_v1(
                &fields_wo_proof,
                &request.operator_sign_pub,
                &self.device_keys.sign_pub.key_bytes,
                request.timestamp,
                invite.expires_at,
            );
            Some(compute_pairing_sas_6digit_v1(&sas_transcript))
        } else {
            None
        };

        // Transition to AwaitingApproval
        self.state = PairingHostState::AwaitingApproval {
            request: request.clone(),
            operator_pub: op_sign_pub.clone(),
            sas: sas.clone(),
        };

        Ok(PairingAction::AwaitingConsent {
            sas,
            operator_id: request.operator_id.clone(),
        })
    }


    /// Approve the pairing request.
    /// Requirements: 1.5, 1.6, 1.7
    pub async fn approve(&mut self, permissions: u32) -> Result<PairReceiptV1, PairingError> {
        // Validate state
        let (request, _operator_pub) = match &self.state {
            PairingHostState::AwaitingApproval {
                request,
                operator_pub,
                ..
            } => (request.clone(), operator_pub.clone()),
            _ => {
                return Err(PairingError::InvalidState(
                    "can only approve from AwaitingApproval state".into(),
                ));
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Generate session binding
        let mut session_binding = [0u8; 32];
        getrandom(&mut session_binding)
            .map_err(|_| PairingError::CryptoError("RNG failed".into()))?;

        // Build receipt (Requirements: 1.6)
        let mut receipt = PairReceiptV1 {
            device_id: self.device_keys.id32.to_vec(),
            operator_id: request.operator_id.clone(),
            permissions_granted: permissions,
            paired_at: now,
            session_binding: session_binding.to_vec(),
            device_signature: vec![], // Will be filled by signing
        };

        // Sign receipt
        sign_pair_receipt_v1(&self.device_keys.sign, &mut receipt)
            .map_err(|e| PairingError::CryptoError(e))?;

        // Store pairing (Requirements: 1.7)
        let op_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: request.operator_sign_pub.clone(),
        };
        let op_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: request.operator_kex_pub.clone(),
        };

        let pairing_record = PairingRecord {
            pairing_id: session_binding.to_vec(),
            device_id: self.device_keys.id32.to_vec(),
            operator_id: request.operator_id.clone(),
            device_sign_pub: self.device_keys.sign_pub.clone(),
            device_kex_pub: self.device_keys.kex_pub.clone(),
            operator_sign_pub: op_sign_pub,
            operator_kex_pub: op_kex_pub,
            granted_perms: vec![permissions as i32],
            unattended_enabled: (permissions & 0x20) != 0, // UNATTENDED flag
            require_consent_each_time: false,
            issued_at: now,
            last_session: None,
        };

        self.store
            .save_pairing(pairing_record)
            .await
            .map_err(|e| PairingError::StoreError(e.to_string()))?;

        // Remove the invite after successful pairing
        let _ = self.store.delete_invite(&self.device_keys.id32).await;

        // Convert operator_id to fixed array
        let mut op_id_arr = [0u8; 32];
        if request.operator_id.len() >= 32 {
            op_id_arr.copy_from_slice(&request.operator_id[..32]);
        }

        // Transition to Paired state
        self.state = PairingHostState::Paired {
            operator_id: op_id_arr,
            permissions,
        };

        Ok(receipt)
    }

    /// Reject the pairing request.
    /// Requirements: 1.5
    pub async fn reject(&mut self) -> Result<(), PairingError> {
        match &self.state {
            PairingHostState::AwaitingApproval { .. } => {
                self.state = PairingHostState::Failed {
                    reason: PairingError::Rejected,
                };
                Ok(())
            }
            _ => Err(PairingError::InvalidState(
                "can only reject from AwaitingApproval state".into(),
            )),
        }
    }

    /// Reset the state machine to Idle.
    pub fn reset(&mut self) {
        self.state = PairingHostState::Idle;
    }
}

// ============================================================================
// Pairing Controller State Machine
// ============================================================================

/// State of the pairing controller state machine.
/// Requirements: 2.1
#[derive(Clone, Debug)]
pub enum PairingControllerState {
    /// Initial state
    Idle,
    /// Invite has been imported and validated
    InviteImported { 
        /// The imported invite
        invite: InviteV1,
    },
    /// Pair request has been sent, awaiting receipt
    RequestSent { 
        /// The sent request
        request: PairRequestV1,
        /// The original invite (needed for SAS computation)
        invite: InviteV1,
    },
    /// Receipt received, awaiting SAS verification by user
    AwaitingSAS { 
        /// The 6-digit SAS code for user verification
        sas: String,
        /// The received receipt (stored for confirmation)
        receipt: PairReceiptV1,
        /// The original invite
        invite: InviteV1,
    },
    /// Pairing completed successfully
    Paired {
        /// Device identifier (32 bytes)
        device_id: [u8; 32],
        /// Granted permissions bitmask
        permissions: u32,
    },
    /// Pairing failed
    Failed { 
        /// The reason for failure
        reason: PairingError,
    },
}

/// Controller-side pairing state machine.
/// Requirements: 2.1-2.8
pub struct PairingController<S: Store> {
    /// Current state
    state: PairingControllerState,
    /// Operator identity keys
    operator_keys: IdentityKeys,
    /// Persistent storage
    store: Arc<S>,
    /// Timeout for pairing (5 minutes)
    timeout_secs: u64,
    /// When the current pairing attempt started
    started_at: Option<u64>,
}

impl<S: Store> PairingController<S> {
    /// Create a new pairing controller.
    pub fn new(operator_keys: IdentityKeys, store: Arc<S>) -> Self {
        Self {
            state: PairingControllerState::Idle,
            operator_keys,
            store,
            timeout_secs: 300, // 5 minutes (Requirements: 2.8)
            started_at: None,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &PairingControllerState {
        &self.state
    }

    /// Import an invite from raw bytes (protobuf-encoded InviteV1).
    /// Requirements: 2.2
    ///
    /// # Arguments
    /// * `invite_data` - Protobuf-encoded InviteV1 bytes
    ///
    /// # Returns
    /// * `Ok(())` on successful import
    /// * `Err(PairingError)` if validation fails
    pub fn import_invite(&mut self, invite_data: &[u8]) -> Result<InviteV1, PairingError> {
        // Validate state transition
        match &self.state {
            PairingControllerState::Idle | PairingControllerState::Failed { .. } => {}
            _ => {
                return Err(PairingError::InvalidState(
                    "can only import invite from Idle or Failed state".into(),
                ));
            }
        }

        // Decode invite from protobuf
        let invite = InviteV1::decode(invite_data)
            .map_err(|e| PairingError::CryptoError(format!("failed to decode invite: {}", e)))?;

        // Validate required fields
        if invite.device_id.is_empty() {
            return Err(PairingError::MissingField("device_id".into()));
        }
        if invite.device_sign_pub.is_empty() {
            return Err(PairingError::MissingField("device_sign_pub".into()));
        }
        if invite.invite_secret_hash.is_empty() {
            return Err(PairingError::MissingField("invite_secret_hash".into()));
        }

        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if invite.expires_at <= now {
            return Err(PairingError::InviteExpired);
        }

        // Record start time for timeout (Requirements: 2.8)
        self.started_at = Some(now);

        // Transition to InviteImported state
        self.state = PairingControllerState::InviteImported {
            invite: invite.clone(),
        };

        Ok(invite)
    }

    /// Import an invite from an already-decoded InviteV1.
    /// Requirements: 2.2
    ///
    /// # Arguments
    /// * `invite` - Already decoded InviteV1
    ///
    /// # Returns
    /// * `Ok(())` on successful import
    /// * `Err(PairingError)` if validation fails
    pub fn import_invite_decoded(&mut self, invite: InviteV1) -> Result<(), PairingError> {
        // Validate state transition
        match &self.state {
            PairingControllerState::Idle | PairingControllerState::Failed { .. } => {}
            _ => {
                return Err(PairingError::InvalidState(
                    "can only import invite from Idle or Failed state".into(),
                ));
            }
        }

        // Validate required fields
        if invite.device_id.is_empty() {
            return Err(PairingError::MissingField("device_id".into()));
        }
        if invite.device_sign_pub.is_empty() {
            return Err(PairingError::MissingField("device_sign_pub".into()));
        }

        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if invite.expires_at <= now {
            return Err(PairingError::InviteExpired);
        }

        // Record start time for timeout (Requirements: 2.8)
        self.started_at = Some(now);

        // Transition to InviteImported state
        self.state = PairingControllerState::InviteImported { invite };

        Ok(())
    }

    /// Send a pair request to the device.
    /// Requirements: 2.3
    ///
    /// # Arguments
    /// * `invite_secret` - The 32-byte invite secret (obtained out-of-band with the invite)
    /// * `requested_permissions` - Bitmask of requested permissions
    ///
    /// # Returns
    /// * `Ok(PairRequestV1)` - The generated request to send to the device
    /// * `Err(PairingError)` if generation fails
    pub async fn send_request(
        &mut self,
        invite_secret: &[u8; 32],
        requested_permissions: u32,
    ) -> Result<PairRequestV1, PairingError> {
        // Check timeout (Requirements: 2.8)
        self.check_timeout()?;

        // Validate state and extract invite
        let invite = match &self.state {
            PairingControllerState::InviteImported { invite } => invite.clone(),
            _ => {
                return Err(PairingError::InvalidState(
                    "can only send request from InviteImported state".into(),
                ));
            }
        };

        // Verify the invite_secret matches the invite's hash
        let computed_hash = sha256(invite_secret);
        if computed_hash.to_vec() != invite.invite_secret_hash {
            return Err(PairingError::InvalidProof);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check invite hasn't expired since import
        if invite.expires_at <= now {
            self.state = PairingControllerState::Failed {
                reason: PairingError::InviteExpired,
            };
            return Err(PairingError::InviteExpired);
        }

        // Generate nonce for replay protection and SAS computation
        let mut nonce = [0u8; 32];
        getrandom(&mut nonce).map_err(|_| PairingError::CryptoError("RNG failed".into()))?;

        // Build proof input using canonical format
        let device_id = DeviceIdV1 {
            id: invite.device_id.clone(),
        };
        let user_id = UserIdV1 {
            id: self.operator_keys.id32.to_vec(),
        };
        let created_at = TimestampV1 { unix_seconds: now };

        // Compute invite_proof = HMAC-SHA256(invite_secret, proof_input)
        let proof_input = pair_proof_input_v1(
            &user_id,
            &self.operator_keys.sign_pub,
            &self.operator_keys.kex_pub,
            &device_id,
            &created_at,
        );
        let invite_proof = compute_pair_proof_v1(invite_secret, &proof_input);

        // Build the pair request
        let request = PairRequestV1 {
            operator_id: self.operator_keys.id32.to_vec(),
            operator_sign_pub: self.operator_keys.sign_pub.key_bytes.clone(),
            operator_kex_pub: self.operator_keys.kex_pub.key_bytes.clone(),
            invite_proof: invite_proof.to_vec(),
            requested_permissions,
            nonce: nonce.to_vec(),
            timestamp: now,
        };

        // Transition to RequestSent state (keep invite for SAS computation later)
        self.state = PairingControllerState::RequestSent {
            request: request.clone(),
            invite,
        };

        Ok(request)
    }


    /// Handle a pair receipt from the device.
    /// Requirements: 2.4, 2.5, 2.6
    ///
    /// # Arguments
    /// * `receipt` - The PairReceiptV1 received from the device
    ///
    /// # Returns
    /// * `Ok(PairingAction)` - The action to take (usually AwaitingConsent with SAS)
    /// * `Err(PairingError)` if verification fails
    pub async fn handle_receipt(
        &mut self,
        receipt: PairReceiptV1,
    ) -> Result<PairingAction, PairingError> {
        // Check timeout (Requirements: 2.8)
        self.check_timeout()?;

        // Validate state and extract request + invite
        let (request, invite) = match &self.state {
            PairingControllerState::RequestSent { request, invite } => {
                (request.clone(), invite.clone())
            }
            _ => {
                return Err(PairingError::InvalidState(
                    "can only handle receipt from RequestSent state".into(),
                ));
            }
        };

        // Verify receipt is for this operator
        if receipt.operator_id != self.operator_keys.id32.to_vec() {
            return Err(PairingError::InvalidState(
                "receipt not for this operator".into(),
            ));
        }

        // Verify receipt is from the expected device
        if receipt.device_id != invite.device_id {
            return Err(PairingError::InvalidState(
                "receipt from unexpected device".into(),
            ));
        }

        // Verify device signature (Requirements: 2.4)
        verify_pair_receipt_with_key_v1(&receipt, &invite.device_sign_pub)
            .map_err(|_| PairingError::SignatureInvalid)?;

        // Compute SAS for user verification (Requirements: 2.5, 2.6)
        let user_id = UserIdV1 {
            id: self.operator_keys.id32.to_vec(),
        };
        let device_id = DeviceIdV1 {
            id: invite.device_id.clone(),
        };
        let created_at = TimestampV1 {
            unix_seconds: request.timestamp,
        };

        // Build canonical fields for SAS computation
        let fields_wo_proof = canonical_pair_request_fields_without_proof_v1(
            &user_id,
            &self.operator_keys.sign_pub,
            &self.operator_keys.kex_pub,
            &device_id,
            &created_at,
            true, // request_sas = true since we have a nonce
        );

        // Compute SAS transcript
        let sas_transcript = pairing_sas_transcript_v1(
            &fields_wo_proof,
            &self.operator_keys.sign_pub.key_bytes,
            &invite.device_sign_pub,
            request.timestamp,
            invite.expires_at,
        );

        // Compute 6-digit SAS code
        let sas = compute_pairing_sas_6digit_v1(&sas_transcript);

        // Transition to AwaitingSAS state
        self.state = PairingControllerState::AwaitingSAS {
            sas: sas.clone(),
            receipt,
            invite,
        };

        Ok(PairingAction::AwaitingConsent {
            sas: Some(sas),
            operator_id: self.operator_keys.id32.to_vec(),
        })
    }

    /// Confirm SAS verification and complete pairing.
    /// Requirements: 2.7
    ///
    /// This should be called after the user has verified that the SAS code
    /// displayed on both devices matches.
    ///
    /// # Returns
    /// * `Ok(())` on successful confirmation
    /// * `Err(PairingError)` if confirmation fails
    pub async fn confirm_sas(&mut self) -> Result<PairReceiptV1, PairingError> {
        // Check timeout (Requirements: 2.8)
        self.check_timeout()?;

        // Validate state and extract receipt + invite
        let (receipt, invite) = match &self.state {
            PairingControllerState::AwaitingSAS { receipt, invite, .. } => {
                (receipt.clone(), invite.clone())
            }
            _ => {
                return Err(PairingError::InvalidState(
                    "can only confirm SAS from AwaitingSAS state".into(),
                ));
            }
        };

        // Store pairing (Requirements: 2.7)
        let device_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: invite.device_sign_pub.clone(),
        };
        // Note: device_kex_pub would need to come from the receipt or a separate exchange
        // For now, we use a placeholder - in production this should be properly exchanged
        let device_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: vec![0u8; 32], // Placeholder - should be exchanged during pairing
        };

        let pairing_record = PairingRecord {
            pairing_id: receipt.session_binding.clone(),
            device_id: receipt.device_id.clone(),
            operator_id: receipt.operator_id.clone(),
            device_sign_pub,
            device_kex_pub,
            operator_sign_pub: self.operator_keys.sign_pub.clone(),
            operator_kex_pub: self.operator_keys.kex_pub.clone(),
            granted_perms: vec![receipt.permissions_granted as i32],
            unattended_enabled: (receipt.permissions_granted & 0x20) != 0,
            require_consent_each_time: false,
            issued_at: receipt.paired_at,
            last_session: None,
        };

        self.store
            .save_pairing(pairing_record)
            .await
            .map_err(|e| PairingError::StoreError(e.to_string()))?;

        // Convert device_id to fixed array
        let mut dev_id_arr = [0u8; 32];
        if receipt.device_id.len() >= 32 {
            dev_id_arr.copy_from_slice(&receipt.device_id[..32]);
        }

        // Transition to Paired state
        self.state = PairingControllerState::Paired {
            device_id: dev_id_arr,
            permissions: receipt.permissions_granted,
        };

        // Clear timeout tracking
        self.started_at = None;

        Ok(receipt)
    }

    /// Reject SAS verification (user indicates codes don't match).
    ///
    /// # Returns
    /// * `Ok(())` on successful rejection
    /// * `Err(PairingError)` if not in correct state
    pub fn reject_sas(&mut self) -> Result<(), PairingError> {
        match &self.state {
            PairingControllerState::AwaitingSAS { .. } => {
                self.state = PairingControllerState::Failed {
                    reason: PairingError::Rejected,
                };
                self.started_at = None;
                Ok(())
            }
            _ => Err(PairingError::InvalidState(
                "can only reject SAS from AwaitingSAS state".into(),
            )),
        }
    }

    /// Check if the pairing has timed out.
    /// Requirements: 2.8 - 5-minute timeout
    fn check_timeout(&mut self) -> Result<(), PairingError> {
        if let Some(started) = self.started_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now - started > self.timeout_secs {
                // Transition to Failed state on timeout
                self.state = PairingControllerState::Failed {
                    reason: PairingError::Timeout,
                };
                self.started_at = None;
                return Err(PairingError::Timeout);
            }
        }
        Ok(())
    }

    /// Get the current SAS code if in AwaitingSAS state.
    ///
    /// # Returns
    /// * `Some(sas)` if in AwaitingSAS state
    /// * `None` otherwise
    pub fn get_sas(&self) -> Option<&str> {
        match &self.state {
            PairingControllerState::AwaitingSAS { sas, .. } => Some(sas),
            _ => None,
        }
    }

    /// Check if pairing is complete.
    pub fn is_paired(&self) -> bool {
        matches!(self.state, PairingControllerState::Paired { .. })
    }

    /// Get the paired device ID if pairing is complete.
    pub fn paired_device_id(&self) -> Option<[u8; 32]> {
        match &self.state {
            PairingControllerState::Paired { device_id, .. } => Some(*device_id),
            _ => None,
        }
    }

    /// Get the granted permissions if pairing is complete.
    pub fn granted_permissions(&self) -> Option<u32> {
        match &self.state {
            PairingControllerState::Paired { permissions, .. } => Some(*permissions),
            _ => None,
        }
    }

    /// Reset the state machine.
    pub fn reset(&mut self) {
        self.state = PairingControllerState::Idle;
        self.started_at = None;
    }
}

// ============================================================================
// Helper Functions for Signing/Verification
// ============================================================================

/// Compute the bytes to sign for a PairReceiptV1.
fn pair_receipt_signing_bytes_v1(r: &PairReceiptV1) -> Result<Vec<u8>, String> {
    let mut rr = r.clone();
    rr.device_signature = vec![];
    let mut buf = Vec::with_capacity(rr.encoded_len());
    rr.encode(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Sign a PairReceiptV1 with the device's signing key.
fn sign_pair_receipt_v1(
    device_sign: &ed25519_dalek::SigningKey,
    r: &mut PairReceiptV1,
) -> Result<(), String> {
    let bytes = pair_receipt_signing_bytes_v1(r)?;
    let digest = sha256(&bytes);
    let sig = device_sign.sign(&digest);
    r.device_signature = sig.to_bytes().to_vec();
    Ok(())
}

/// Verify a PairReceiptV1 signature.
fn verify_pair_receipt_v1(r: &PairReceiptV1) -> Result<(), String> {
    if r.device_signature.len() != 64 {
        return Err("invalid signature length".into());
    }
    // For now, just check signature format
    // Full verification requires device public key from invite
    let _bytes = pair_receipt_signing_bytes_v1(r)?;
    Ok(())
}

/// Verify a PairReceiptV1 signature using the device's public key.
fn verify_pair_receipt_with_key_v1(r: &PairReceiptV1, device_sign_pub: &[u8]) -> Result<(), String> {
    if r.device_signature.len() != 64 {
        return Err("invalid signature length".into());
    }
    if device_sign_pub.len() != 32 {
        return Err("invalid public key length".into());
    }

    // Get the bytes that were signed
    let bytes = pair_receipt_signing_bytes_v1(r)?;
    let digest = sha256(&bytes);

    // Parse the public key
    let pub_key_bytes: [u8; 32] = device_sign_pub
        .try_into()
        .map_err(|_| "invalid public key length")?;
    let verifying_key = VerifyingKey::from_bytes(&pub_key_bytes)
        .map_err(|e| format!("invalid public key: {}", e))?;

    // Parse the signature
    let sig_bytes: [u8; 64] = r.device_signature[..64]
        .try_into()
        .map_err(|_| "invalid signature length")?;
    let signature = Signature::from_bytes(&sig_bytes);

    // Verify
    verifying_key
        .verify_strict(&digest, &signature)
        .map_err(|e| format!("signature verification failed: {}", e))?;

    Ok(())
}

// ============================================================================
// Legacy API Compatibility
// ============================================================================

/// Legacy pairing approver trait (for backward compatibility).
#[async_trait]
pub trait PairingApprover: Send + Sync {
    async fn decide(
        &self,
        req: &PairRequestV1,
        sas_6digit: Option<&str>,
    ) -> anyhow::Result<PairDecision>;
}

/// Legacy host-side pairing handler (for backward compatibility).
pub struct LegacyPairingHost<A: PairingApprover> {
    pub store: MemoryStore,
    pub approver: A,
    pub device_keys: IdentityKeys,
}

impl<A: PairingApprover> LegacyPairingHost<A> {
    pub fn new(store: MemoryStore, approver: A, device_keys: IdentityKeys) -> Self {
        Self {
            store,
            approver,
            device_keys,
        }
    }

    pub async fn handle_pair_request(
        &self,
        now_unix: u64,
        req: PairRequestV1,
    ) -> Result<Outgoing, CoreError> {
        let invite = self
            .store
            .get_invite(&self.device_keys.id32)
            .await
            .ok_or(CoreError::Denied("no active invite".into()))?;

        if invite.expires_at_unix <= now_unix {
            return Err(CoreError::Denied("invite expired".into()));
        }

        // Build user_id and device_id for proof verification
        let user_id = UserIdV1 {
            id: req.operator_id.clone(),
        };
        let device_id = DeviceIdV1 {
            id: self.device_keys.id32.to_vec(),
        };
        let created_at = TimestampV1 {
            unix_seconds: req.timestamp,
        };

        let op_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: req.operator_sign_pub.clone(),
        };
        let op_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: req.operator_kex_pub.clone(),
        };

        // Verify proof
        let proof_input =
            pair_proof_input_v1(&user_id, &op_sign_pub, &op_kex_pub, &device_id, &created_at);
        let expected = compute_pair_proof_v1(&invite.invite_secret, &proof_input);

        if req.invite_proof != expected {
            return Err(CoreError::Denied("pair_proof invalid".into()));
        }

        // Ask approver
        let decision = self
            .approver
            .decide(&req, None)
            .await
            .map_err(|e| CoreError::Denied(format!("approver error: {e}")))?;

        if !decision.approved {
            return Err(CoreError::Denied("user denied pairing".into()));
        }

        // Build receipt
        let mut session_binding = [0u8; 32];
        getrandom(&mut session_binding).map_err(|_| CoreError::Crypto("rng failed".into()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let permissions: u32 = decision
            .granted_perms
            .iter()
            .map(|p| *p as u32)
            .fold(0, |acc, p| acc | p);

        let mut receipt = PairReceiptV1 {
            device_id: self.device_keys.id32.to_vec(),
            operator_id: req.operator_id.clone(),
            permissions_granted: permissions,
            paired_at: now,
            session_binding: session_binding.to_vec(),
            device_signature: vec![],
        };

        sign_pair_receipt_v1(&self.device_keys.sign, &mut receipt)
            .map_err(|e| CoreError::Crypto(e))?;

        // Store pairing
        self.store
            .put_pairing(PairingRecord {
                pairing_id: session_binding.to_vec(),
                device_id: self.device_keys.id32.to_vec(),
                operator_id: req.operator_id.clone(),
                device_sign_pub: self.device_keys.sign_pub.clone(),
                device_kex_pub: self.device_keys.kex_pub.clone(),
                operator_sign_pub: op_sign_pub,
                operator_kex_pub: op_kex_pub,
                granted_perms: decision.granted_perms.iter().map(|p| *p as i32).collect(),
                unattended_enabled: decision.unattended_enabled,
                require_consent_each_time: decision.require_consent_each_time,
                issued_at: now,
                last_session: None,
            })
            .await;

        // Remove invite
        self.store.remove_invite(&self.device_keys.id32).await;

        // Encode receipt
        let mut receipt_bytes = Vec::with_capacity(receipt.encoded_len());
        receipt
            .encode(&mut receipt_bytes)
            .map_err(|e| CoreError::Decode(e.to_string()))?;

        Ok(Outgoing {
            recipient_id: req.operator_id,
            envelope_bytes: Bytes::from(receipt_bytes),
        })
    }
}

/// Legacy controller-side pairing helper.
pub struct LegacyPairingController {
    pub operator_keys: IdentityKeys,
    pub store: MemoryStore,
}

impl LegacyPairingController {
    pub fn new(store: MemoryStore, operator_keys: IdentityKeys) -> Self {
        Self {
            operator_keys,
            store,
        }
    }

    pub fn make_pair_request_from_invite(
        &self,
        invite: &InviteV1,
        invite_secret: &[u8; 32],
        now_unix: u64,
    ) -> Result<PairRequestV1, CoreError> {
        let device_id = DeviceIdV1 {
            id: invite.device_id.clone(),
        };
        let user_id = UserIdV1 {
            id: self.operator_keys.id32.to_vec(),
        };
        let created_at = TimestampV1 {
            unix_seconds: now_unix,
        };

        let proof_input = pair_proof_input_v1(
            &user_id,
            &self.operator_keys.sign_pub,
            &self.operator_keys.kex_pub,
            &device_id,
            &created_at,
        );
        let proof = compute_pair_proof_v1(invite_secret, &proof_input);

        let mut nonce = [0u8; 32];
        getrandom(&mut nonce).map_err(|_| CoreError::Crypto("rng failed".into()))?;

        Ok(PairRequestV1 {
            operator_id: self.operator_keys.id32.to_vec(),
            operator_sign_pub: self.operator_keys.sign_pub.key_bytes.clone(),
            operator_kex_pub: self.operator_keys.kex_pub.key_bytes.clone(),
            invite_proof: proof.to_vec(),
            requested_permissions: 0,
            nonce: nonce.to_vec(),
            timestamp: now_unix,
        })
    }

    pub async fn accept_pair_receipt(
        &self,
        receipt: PairReceiptV1,
        _now_unix: u64,
    ) -> Result<(), CoreError> {
        if receipt.operator_id != self.operator_keys.id32.to_vec() {
            return Err(CoreError::Denied("receipt not for this operator".into()));
        }

        let device_sign_pub = PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: receipt.device_id.clone(),
        };
        let device_kex_pub = PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: vec![0u8; 32],
        };

        self.store
            .put_pairing(PairingRecord {
                pairing_id: receipt.session_binding.clone(),
                device_id: receipt.device_id.clone(),
                operator_id: receipt.operator_id.clone(),
                device_sign_pub,
                device_kex_pub,
                operator_sign_pub: self.operator_keys.sign_pub.clone(),
                operator_kex_pub: self.operator_keys.kex_pub.clone(),
                granted_perms: vec![receipt.permissions_granted as i32],
                unattended_enabled: (receipt.permissions_granted & 0x20) != 0,
                require_consent_each_time: false,
                issued_at: receipt.paired_at,
                last_session: None,
            })
            .await;

        Ok(())
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::generate_identity_keys;
    use crate::store::InMemoryStore;

    /// Simple consent handler that always approves.
    struct AlwaysApprove;

    #[async_trait]
    impl ConsentHandler for AlwaysApprove {
        async fn request_consent(
            &self,
            _operator_id: &[u8],
            _sas: Option<&str>,
        ) -> Result<PairDecision, PairingError> {
            Ok(PairDecision {
                approved: true,
                granted_perms: vec![PermissionV1::View, PermissionV1::Control],
                unattended_enabled: false,
                require_consent_each_time: true,
            })
        }
    }

    #[tokio::test]
    async fn test_pairing_host_state_transitions() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let consent = Arc::new(AlwaysApprove);

        let mut host = PairingHost::new(device_keys, store, consent);

        // Initial state should be Idle
        assert!(matches!(host.state(), PairingHostState::Idle));

        // Generate invite
        let invite = host.generate_invite(300, None).await.unwrap();
        assert!(!invite.device_id.is_empty());
        assert!(matches!(host.state(), PairingHostState::InviteGenerated { .. }));
    }

    #[tokio::test]
    async fn test_pairing_controller_state_transitions() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = PairingController::new(operator_keys, store);

        // Initial state should be Idle
        assert!(matches!(controller.state(), PairingControllerState::Idle));
    }

    #[tokio::test]
    async fn test_pairing_controller_import_invite() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = PairingController::new(operator_keys, store);

        // Create a valid invite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut secret = [0u8; 32];
        getrandom(&mut secret).unwrap();
        let secret_hash = sha256(&secret);

        let invite = InviteV1 {
            device_id: device_keys.id32.to_vec(),
            device_sign_pub: device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at: now + 300,
            transport_hints: None,
        };

        // Encode and import
        let mut invite_bytes = Vec::new();
        invite.encode(&mut invite_bytes).unwrap();
        
        let result = controller.import_invite(&invite_bytes);
        assert!(result.is_ok());
        assert!(matches!(controller.state(), PairingControllerState::InviteImported { .. }));
    }

    #[tokio::test]
    async fn test_pairing_controller_import_expired_invite() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = PairingController::new(operator_keys, store);

        // Create an expired invite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut secret = [0u8; 32];
        getrandom(&mut secret).unwrap();
        let secret_hash = sha256(&secret);

        let invite = InviteV1 {
            device_id: device_keys.id32.to_vec(),
            device_sign_pub: device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at: now - 100, // Already expired
            transport_hints: None,
        };

        // Encode and try to import
        let mut invite_bytes = Vec::new();
        invite.encode(&mut invite_bytes).unwrap();
        
        let result = controller.import_invite(&invite_bytes);
        assert!(matches!(result, Err(PairingError::InviteExpired)));
    }

    #[tokio::test]
    async fn test_pairing_controller_send_request() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = PairingController::new(operator_keys, store);

        // Create and import a valid invite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut secret = [0u8; 32];
        getrandom(&mut secret).unwrap();
        let secret_hash = sha256(&secret);

        let invite = InviteV1 {
            device_id: device_keys.id32.to_vec(),
            device_sign_pub: device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at: now + 300,
            transport_hints: None,
        };

        controller.import_invite_decoded(invite).unwrap();

        // Send request
        let request = controller.send_request(&secret, 0x03).await.unwrap();
        
        assert!(!request.operator_id.is_empty());
        assert!(!request.invite_proof.is_empty());
        assert!(matches!(controller.state(), PairingControllerState::RequestSent { .. }));
    }

    #[tokio::test]
    async fn test_pairing_controller_invalid_secret() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = PairingController::new(operator_keys, store);

        // Create and import a valid invite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut secret = [0u8; 32];
        getrandom(&mut secret).unwrap();
        let secret_hash = sha256(&secret);

        let invite = InviteV1 {
            device_id: device_keys.id32.to_vec(),
            device_sign_pub: device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at: now + 300,
            transport_hints: None,
        };

        controller.import_invite_decoded(invite).unwrap();

        // Try to send request with wrong secret
        let wrong_secret = [1u8; 32];
        let result = controller.send_request(&wrong_secret, 0x03).await;
        
        assert!(matches!(result, Err(PairingError::InvalidProof)));
    }

    #[tokio::test]
    async fn test_pairing_controller_reset() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = PairingController::new(operator_keys, store);

        // Create and import a valid invite
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut secret = [0u8; 32];
        getrandom(&mut secret).unwrap();
        let secret_hash = sha256(&secret);

        let invite = InviteV1 {
            device_id: device_keys.id32.to_vec(),
            device_sign_pub: device_keys.sign_pub.key_bytes.clone(),
            invite_secret_hash: secret_hash.to_vec(),
            expires_at: now + 300,
            transport_hints: None,
        };

        controller.import_invite_decoded(invite).unwrap();
        assert!(matches!(controller.state(), PairingControllerState::InviteImported { .. }));

        // Reset
        controller.reset();
        assert!(matches!(controller.state(), PairingControllerState::Idle));
    }
}
