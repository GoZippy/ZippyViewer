//! Session state machines for ZRC.
//!
//! This module implements the session workflow state machines for both
//! host (device) and controller (operator) sides.
//!
//! Requirements: 3.1-3.9, 4.1-4.8

use std::sync::Arc;

use async_trait::async_trait;
use ed25519_dalek::Signer;
use getrandom::getrandom;
use prost::Message;

use crate::{
    policy::{PolicyEngine, PolicyError},
    store::{PairingRecord, Store, StoreError, TicketRecord},
    transport::TransportNegotiator,
    types::IdentityKeys,
};
use zrc_crypto::hash::sha256;
use zrc_proto::v1::{
    SessionInitRequestV1, SessionInitResponseV1, SessionTicketV1, TransportNegotiationV1,
};

// ============================================================================
// Error Types
// ============================================================================

/// Errors specific to session operations.
#[derive(Debug, Clone)]
pub enum SessionError {
    /// Operator is not paired with this device
    NotPaired,
    /// Permission denied by policy
    PermissionDenied(String),
    /// Session ticket has expired
    TicketExpired,
    /// Session ticket is invalid
    TicketInvalid(String),
    /// Invalid state transition
    InvalidState(String),
    /// Consent was denied by user
    ConsentDenied,
    /// Cryptographic operation failed
    CryptoError(String),
    /// Missing required field
    MissingField(String),
    /// Signature verification failed
    SignatureInvalid,
    /// Store operation failed
    StoreError(String),
    /// Policy error
    PolicyError(String),
    /// Transport negotiation failed
    TransportError(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotPaired => write!(f, "operator is not paired with this device"),
            SessionError::PermissionDenied(s) => write!(f, "permission denied: {}", s),
            SessionError::TicketExpired => write!(f, "session ticket has expired"),
            SessionError::TicketInvalid(s) => write!(f, "session ticket is invalid: {}", s),
            SessionError::InvalidState(s) => write!(f, "invalid state: {}", s),
            SessionError::ConsentDenied => write!(f, "consent was denied by user"),
            SessionError::CryptoError(s) => write!(f, "crypto error: {}", s),
            SessionError::MissingField(s) => write!(f, "missing field: {}", s),
            SessionError::SignatureInvalid => write!(f, "signature verification failed"),
            SessionError::StoreError(s) => write!(f, "store error: {}", s),
            SessionError::PolicyError(s) => write!(f, "policy error: {}", s),
            SessionError::TransportError(s) => write!(f, "transport error: {}", s),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<StoreError> for SessionError {
    fn from(e: StoreError) -> Self {
        SessionError::StoreError(e.to_string())
    }
}

impl From<PolicyError> for SessionError {
    fn from(e: PolicyError) -> Self {
        SessionError::PolicyError(e.to_string())
    }
}

// ============================================================================
// Session End Reason
// ============================================================================

/// Reason for session termination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEndReason {
    /// Normal termination requested by operator
    OperatorDisconnect,
    /// Normal termination requested by device
    DeviceDisconnect,
    /// Session ticket expired
    TicketExpired,
    /// Policy violation detected
    PolicyViolation(String),
    /// Transport connection lost
    TransportLost,
    /// Consent revoked during session
    ConsentRevoked,
    /// Error occurred
    Error(String),
}

// ============================================================================
// Session Consent Handler Trait
// ============================================================================

/// Decision returned by the session consent handler.
#[derive(Clone, Debug)]
pub struct SessionConsentDecision {
    /// Whether session was approved
    pub approved: bool,
    /// Granted permissions (if approved)
    pub granted_permissions: u32,
}

/// Trait for handling session consent decisions.
/// Requirements: 3.4
#[async_trait]
pub trait SessionConsentHandler: Send + Sync {
    /// Called when a session request needs user approval.
    async fn request_consent(
        &self,
        operator_id: &[u8],
        requested_permissions: u32,
        paired_permissions: u32,
    ) -> Result<SessionConsentDecision, SessionError>;
}

// ============================================================================
// Session Host State Machine
// ============================================================================

/// State of the session host state machine.
/// Requirements: 3.1
#[derive(Clone, Debug)]
pub enum SessionHostState {
    /// Initial state, ready to receive session requests
    Idle,
    /// Session request received, processing
    RequestReceived {
        /// The received session init request
        request: SessionInitRequestV1,
        /// Operator identifier
        operator_id: Vec<u8>,
        /// Pairing record for this operator
        pairing: PairingRecord,
    },
    /// Awaiting user consent
    AwaitingConsent {
        /// The received session init request
        request: SessionInitRequestV1,
        /// Operator identifier
        operator_id: Vec<u8>,
        /// Pairing record for this operator
        pairing: PairingRecord,
    },
    /// Negotiating transport
    Negotiating {
        /// Session identifier
        session_id: [u8; 32],
        /// Operator identifier
        operator_id: Vec<u8>,
        /// Granted permissions
        permissions: u32,
    },
    /// Session is active
    Active {
        /// Active session data
        session: ActiveSession,
    },
    /// Session has ended
    Ended {
        /// Reason for session termination
        reason: SessionEndReason,
    },
}

/// Active session data.
/// Requirements: 3.8
#[derive(Clone, Debug)]
pub struct ActiveSession {
    /// Session identifier (32 bytes)
    pub session_id: [u8; 32],
    /// Operator identifier (32 bytes)
    pub operator_id: Vec<u8>,
    /// Granted permissions bitmask
    pub permissions: u32,
    /// Session ticket
    pub ticket: SessionTicketV1,
    /// Unix timestamp when session started
    pub started_at: u64,
}

/// Action returned by session operations.
#[derive(Clone, Debug)]
pub enum SessionAction {
    /// Waiting for user consent
    AwaitingConsent {
        operator_id: Vec<u8>,
        requested_permissions: u32,
    },
    /// Session was auto-approved (unattended mode)
    AutoApproved {
        response: SessionInitResponseV1,
    },
    /// Session was rejected
    Rejected {
        reason: String,
    },
}


/// Host-side session state machine.
/// Requirements: 3.1-3.9
pub struct SessionHost<S: Store, C: SessionConsentHandler> {
    /// Current state of the state machine
    state: SessionHostState,
    /// Device identity keys
    device_keys: IdentityKeys,
    /// Persistent storage
    store: Arc<S>,
    /// Policy engine for consent and permissions
    policy: Arc<PolicyEngine>,
    /// Consent handler for user approval
    consent_handler: Arc<C>,
    /// Transport negotiator
    transport_negotiator: TransportNegotiator,
    /// Default ticket TTL in seconds
    ticket_ttl_secs: u64,
}

impl<S: Store, C: SessionConsentHandler> SessionHost<S, C> {
    /// Create a new session host state machine.
    pub fn new(
        device_keys: IdentityKeys,
        store: Arc<S>,
        policy: Arc<PolicyEngine>,
        consent_handler: Arc<C>,
    ) -> Self {
        Self {
            state: SessionHostState::Idle,
            device_keys,
            store,
            policy,
            consent_handler,
            transport_negotiator: TransportNegotiator::default(),
            ticket_ttl_secs: 3600, // 1 hour default
        }
    }

    /// Create a new session host with custom transport negotiator.
    pub fn with_transport_negotiator(
        device_keys: IdentityKeys,
        store: Arc<S>,
        policy: Arc<PolicyEngine>,
        consent_handler: Arc<C>,
        transport_negotiator: TransportNegotiator,
    ) -> Self {
        Self {
            state: SessionHostState::Idle,
            device_keys,
            store,
            policy,
            consent_handler,
            transport_negotiator,
            ticket_ttl_secs: 3600,
        }
    }

    /// Set the default ticket TTL.
    pub fn set_ticket_ttl(&mut self, ttl_secs: u64) {
        self.ticket_ttl_secs = ttl_secs;
    }

    /// Get the current state.
    pub fn state(&self) -> &SessionHostState {
        &self.state
    }

    /// Get the device keys.
    pub fn device_keys(&self) -> &IdentityKeys {
        &self.device_keys
    }

    /// Handle an incoming session init request.
    /// Requirements: 3.2, 3.3, 3.4
    pub async fn handle_request(
        &mut self,
        request: SessionInitRequestV1,
    ) -> Result<SessionAction, SessionError> {
        // Validate state transition
        match &self.state {
            SessionHostState::Idle | SessionHostState::Ended { .. } => {}
            _ => {
                return Err(SessionError::InvalidState(
                    "can only handle request from Idle or Ended state".into(),
                ));
            }
        }

        // Validate required fields
        if request.operator_id.is_empty() {
            return Err(SessionError::MissingField("operator_id".into()));
        }
        if request.session_id.is_empty() {
            return Err(SessionError::MissingField("session_id".into()));
        }

        // Verify operator is paired (Requirements: 3.2, 3.3)
        let pairing = self
            .store
            .load_pairing(&self.device_keys.id32, &request.operator_id)
            .await?
            .ok_or(SessionError::NotPaired)?;

        // Get paired permissions as bitmask
        // The granted_perms are stored as PermissionV1 enum values (1=VIEW, 2=CONTROL, etc.)
        // We need to convert them to a bitmask where each permission is a power of 2
        let paired_permissions: u32 = pairing
            .granted_perms
            .iter()
            .fold(0u32, |acc, p| acc | (1 << (*p as u32)));

        // Validate requested permissions against paired permissions
        let requested = request.requested_capabilities;
        
        // Only validate if requested permissions are non-zero
        if requested != 0 {
            self.policy
                .validate_permissions(&[0u8; 32], requested, paired_permissions)?;
        }

        // Check time-based restrictions
        self.policy.check_time_restrictions()?;

        // Transition to RequestReceived
        self.state = SessionHostState::RequestReceived {
            request: request.clone(),
            operator_id: request.operator_id.clone(),
            pairing: pairing.clone(),
        };

        // Check if consent is required (Requirements: 3.4)
        let mut operator_id_arr = [0u8; 32];
        if request.operator_id.len() >= 32 {
            operator_id_arr.copy_from_slice(&request.operator_id[..32]);
        }

        let requires_consent = self
            .policy
            .requires_consent(&operator_id_arr, pairing.unattended_enabled);

        if requires_consent {
            // Transition to AwaitingConsent
            self.state = SessionHostState::AwaitingConsent {
                request: request.clone(),
                operator_id: request.operator_id.clone(),
                pairing,
            };

            Ok(SessionAction::AwaitingConsent {
                operator_id: request.operator_id,
                requested_permissions: requested,
            })
        } else {
            // Auto-approve for unattended access
            let response = self.create_session_response(requested, paired_permissions).await?;
            Ok(SessionAction::AutoApproved { response })
        }
    }


    /// Approve the session request after user consent.
    /// Requirements: 3.5, 3.6, 3.7
    pub async fn approve(&mut self) -> Result<SessionInitResponseV1, SessionError> {
        // Validate state
        let (request, pairing) = match &self.state {
            SessionHostState::AwaitingConsent {
                request,
                pairing,
                ..
            } => (request.clone(), pairing.clone()),
            _ => {
                return Err(SessionError::InvalidState(
                    "can only approve from AwaitingConsent state".into(),
                ));
            }
        };

        // Get paired permissions as bitmask
        let paired_permissions: u32 = pairing
            .granted_perms
            .iter()
            .fold(0u32, |acc, p| acc | (1 << (*p as u32)));

        let requested = request.requested_capabilities;
        self.create_session_response(requested, paired_permissions).await
    }

    /// Create session response with ticket and transport params.
    /// Requirements: 3.5, 3.6, 3.7
    async fn create_session_response(
        &mut self,
        requested: u32,
        paired_permissions: u32,
    ) -> Result<SessionInitResponseV1, SessionError> {
        let (request, operator_id) = match &self.state {
            SessionHostState::RequestReceived {
                request,
                operator_id,
                ..
            }
            | SessionHostState::AwaitingConsent {
                request,
                operator_id,
                ..
            } => (request.clone(), operator_id.clone()),
            _ => {
                return Err(SessionError::InvalidState(
                    "invalid state for creating response".into(),
                ));
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Calculate effective permissions (intersection of requested and paired)
        let granted_permissions = requested & paired_permissions;

        // Generate session ID from request or create new one
        let mut session_id = [0u8; 32];
        if request.session_id.len() >= 32 {
            session_id.copy_from_slice(&request.session_id[..32]);
        } else {
            getrandom(&mut session_id)
                .map_err(|_| SessionError::CryptoError("RNG failed".into()))?;
        }

        // Generate ticket ID (16 bytes)
        let mut ticket_id = [0u8; 16];
        getrandom(&mut ticket_id)
            .map_err(|_| SessionError::CryptoError("RNG failed".into()))?;

        // Compute session binding
        let binding_input = [
            &session_id[..],
            &operator_id[..],
            &self.device_keys.id32[..],
        ]
        .concat();
        let session_binding = sha256(&binding_input);

        // Create session ticket (Requirements: 3.6)
        let mut ticket = SessionTicketV1 {
            ticket_id: ticket_id.to_vec(),
            session_id: session_id.to_vec(),
            operator_id: operator_id.clone(),
            device_id: self.device_keys.id32.to_vec(),
            permissions: granted_permissions,
            expires_at: now + self.ticket_ttl_secs,
            session_binding: session_binding.to_vec(),
            device_signature: vec![], // Will be filled by signing
            ..Default::default()
        };

        // Sign the ticket
        sign_session_ticket_v1(&self.device_keys.sign, &mut ticket)
            .map_err(|e| SessionError::CryptoError(e))?;

        // Generate transport negotiation params (Requirements: 3.7)
        let _transport_params = self.transport_negotiator.generate_params(None, vec![]);
        let transport_negotiation = TransportNegotiationV1 {
            preferred_transport: 0, // AUTO
            quic_params: None,
            relay_tokens: vec![],
            ice_candidates: vec![],
        };

        // Transition to Negotiating state
        self.state = SessionHostState::Negotiating {
            session_id,
            operator_id: operator_id.clone(),
            permissions: granted_permissions,
        };

        // Build response
        let mut response = SessionInitResponseV1 {
            session_id: session_id.to_vec(),
            granted_capabilities: granted_permissions,
            transport_params: Some(transport_negotiation),
            issued_ticket: Some(ticket.clone()),
            device_signature: vec![], // Will be filled by signing
            device_id: self.device_keys.id32.to_vec(),
            operator_id: operator_id.clone(),
            requires_consent: false,
            ..Default::default()
        };

        // Sign the response
        sign_session_init_response_v1(&self.device_keys.sign, &mut response)
            .map_err(|e| SessionError::CryptoError(e))?;

        // Store the ticket
        let ticket_record = TicketRecord::from(&ticket);
        self.store.save_ticket(ticket_record).await?;

        // Update pairing last session timestamp
        let _ = self
            .store
            .update_pairing_last_session(&self.device_keys.id32, &operator_id, now)
            .await;

        // Transition to Active state
        self.state = SessionHostState::Active {
            session: ActiveSession {
                session_id,
                operator_id,
                permissions: granted_permissions,
                ticket,
                started_at: now,
            },
        };

        Ok(response)
    }


    /// Reject the session request.
    /// Requirements: 3.5
    pub async fn reject(&mut self, reason: &str) -> Result<(), SessionError> {
        match &self.state {
            SessionHostState::AwaitingConsent { .. }
            | SessionHostState::RequestReceived { .. } => {
                self.state = SessionHostState::Ended {
                    reason: SessionEndReason::PolicyViolation(reason.to_string()),
                };
                Ok(())
            }
            _ => Err(SessionError::InvalidState(
                "can only reject from AwaitingConsent or RequestReceived state".into(),
            )),
        }
    }

    /// End an active session.
    /// Requirements: 3.9
    pub async fn end_session(&mut self, reason: SessionEndReason) -> Result<(), SessionError> {
        match &self.state {
            SessionHostState::Active { session } => {
                // Revoke the ticket
                let _ = self.store.revoke_ticket(&session.ticket.ticket_id).await;

                self.state = SessionHostState::Ended {
                    reason,
                };
                Ok(())
            }
            SessionHostState::Negotiating { .. } => {
                self.state = SessionHostState::Ended {
                    reason,
                };
                Ok(())
            }
            _ => Err(SessionError::InvalidState(
                "can only end session from Active or Negotiating state".into(),
            )),
        }
    }

    /// Check if a ticket is valid for this session.
    pub async fn validate_ticket(&self, ticket: &SessionTicketV1) -> Result<bool, SessionError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check expiry
        if ticket.expires_at <= now {
            return Ok(false);
        }

        // Check if ticket is in store and not revoked
        self.store
            .is_ticket_valid(&ticket.ticket_id, now)
            .await
            .map_err(|e| SessionError::StoreError(e.to_string()))
    }

    /// Reset the state machine to Idle.
    pub fn reset(&mut self) {
        self.state = SessionHostState::Idle;
    }

    /// Get the active session if in Active state.
    pub fn active_session(&self) -> Option<&ActiveSession> {
        match &self.state {
            SessionHostState::Active { session } => Some(session),
            _ => None,
        }
    }

    /// Check if session is active.
    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionHostState::Active { .. })
    }
}

// ============================================================================
// Session Controller State Machine
// ============================================================================

/// State of the session controller state machine.
/// Requirements: 4.1
#[derive(Clone, Debug)]
pub enum SessionControllerState {
    /// Initial state, ready to start a session
    Idle,
    /// Session request sent, awaiting response
    RequestSent {
        /// The sent request
        request: SessionInitRequestV1,
        /// Device identifier we're connecting to
        device_id: Vec<u8>,
        /// Timestamp when request was sent (for timeout tracking)
        sent_at: u64,
    },
    /// Ticket received from device, ready to connect
    TicketReceived {
        /// Session identifier
        session_id: [u8; 32],
        /// The received ticket
        ticket: SessionTicketV1,
        /// Transport negotiation params
        transport_params: Option<TransportNegotiationV1>,
        /// Granted permissions
        permissions: u32,
    },
    /// Connecting to device via selected transport
    Connecting {
        /// Session identifier
        session_id: [u8; 32],
        /// The ticket for this session
        ticket: SessionTicketV1,
        /// Granted permissions
        permissions: u32,
    },
    /// Session is active
    Active {
        /// Active session data
        session: ControllerActiveSession,
    },
    /// Session has ended
    Ended {
        /// Reason for session termination
        reason: SessionEndReason,
    },
}

/// Active session data for the controller side.
/// Requirements: 4.6
#[derive(Clone, Debug)]
pub struct ControllerActiveSession {
    /// Session identifier (32 bytes)
    pub session_id: [u8; 32],
    /// Device identifier (32 bytes)
    pub device_id: Vec<u8>,
    /// Granted permissions bitmask
    pub permissions: u32,
    /// Session ticket
    pub ticket: SessionTicketV1,
    /// Unix timestamp when session started
    pub started_at: u64,
    /// Unix timestamp when ticket expires
    pub ticket_expires_at: u64,
}

/// Controller-side session state machine.
/// Requirements: 4.1-4.8
pub struct SessionController<S: Store> {
    /// Current state
    state: SessionControllerState,
    /// Operator identity keys
    operator_keys: IdentityKeys,
    /// Persistent storage
    store: Arc<S>,
    /// Transport negotiator
    transport_negotiator: TransportNegotiator,
    /// Session request timeout in seconds (default: 30)
    request_timeout_secs: u64,
    /// Ticket renewal threshold in seconds (renew when this much time left)
    renewal_threshold_secs: u64,
}

impl<S: Store> SessionController<S> {
    /// Create a new session controller.
    pub fn new(operator_keys: IdentityKeys, store: Arc<S>) -> Self {
        Self {
            state: SessionControllerState::Idle,
            operator_keys,
            store,
            transport_negotiator: TransportNegotiator::default(),
            request_timeout_secs: 30,
            renewal_threshold_secs: 300, // 5 minutes before expiry
        }
    }

    /// Create a new session controller with custom transport negotiator.
    pub fn with_transport_negotiator(
        operator_keys: IdentityKeys,
        store: Arc<S>,
        transport_negotiator: TransportNegotiator,
    ) -> Self {
        Self {
            state: SessionControllerState::Idle,
            operator_keys,
            store,
            transport_negotiator,
            request_timeout_secs: 30,
            renewal_threshold_secs: 300,
        }
    }

    /// Set the request timeout.
    pub fn set_request_timeout(&mut self, timeout_secs: u64) {
        self.request_timeout_secs = timeout_secs;
    }

    /// Set the ticket renewal threshold.
    pub fn set_renewal_threshold(&mut self, threshold_secs: u64) {
        self.renewal_threshold_secs = threshold_secs;
    }

    /// Get the current state.
    pub fn state(&self) -> &SessionControllerState {
        &self.state
    }

    /// Get the operator keys.
    pub fn operator_keys(&self) -> &IdentityKeys {
        &self.operator_keys
    }

    /// Reset the state machine to Idle.
    pub fn reset(&mut self) {
        self.state = SessionControllerState::Idle;
    }

    /// Start a new session with a device.
    /// Requirements: 4.2
    ///
    /// # Arguments
    /// * `device_id` - The device identifier (32 bytes)
    /// * `requested_capabilities` - Bitmask of requested permissions
    ///
    /// # Returns
    /// * `Ok(request)` - The session init request to send to the device
    /// * `Err(SessionError)` - If the operation fails
    pub async fn start_session(
        &mut self,
        device_id: &[u8],
        requested_capabilities: u32,
    ) -> Result<SessionInitRequestV1, SessionError> {
        // Validate state transition
        match &self.state {
            SessionControllerState::Idle | SessionControllerState::Ended { .. } => {}
            _ => {
                return Err(SessionError::InvalidState(
                    "can only start session from Idle or Ended state".into(),
                ));
            }
        }

        // Validate device_id length
        if device_id.len() != 32 {
            return Err(SessionError::MissingField(
                "device_id must be 32 bytes".into(),
            ));
        }

        // Verify we are paired with this device (Requirements: 4.2)
        let pairing = self
            .store
            .load_pairing(device_id, &self.operator_keys.id32)
            .await?
            .ok_or(SessionError::NotPaired)?;

        // Get paired permissions as bitmask
        let paired_permissions: u32 = pairing
            .granted_perms
            .iter()
            .fold(0u32, |acc, p| acc | (1 << (*p as u32)));

        // Validate requested capabilities don't exceed paired permissions
        if requested_capabilities != 0 && (requested_capabilities & !paired_permissions) != 0 {
            return Err(SessionError::PermissionDenied(
                "requested capabilities exceed paired permissions".into(),
            ));
        }

        // Generate session ID (32 bytes)
        let mut session_id = [0u8; 32];
        getrandom(&mut session_id)
            .map_err(|_| SessionError::CryptoError("RNG failed".into()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Build the session init request
        let mut request = SessionInitRequestV1 {
            operator_id: self.operator_keys.id32.to_vec(),
            device_id: device_id.to_vec(),
            session_id: session_id.to_vec(),
            requested_capabilities,
            transport_preference: 0, // AUTO
            operator_signature: vec![], // Will be filled by signing
            ..Default::default()
        };

        // Sign the request
        sign_session_init_request_v1(&self.operator_keys.sign, &mut request)
            .map_err(|e| SessionError::CryptoError(e))?;

        // Transition to RequestSent state
        self.state = SessionControllerState::RequestSent {
            request: request.clone(),
            device_id: device_id.to_vec(),
            sent_at: now,
        };

        Ok(request)
    }

    /// Handle a session init response from the device.
    /// Requirements: 4.3, 4.4, 4.5
    ///
    /// # Arguments
    /// * `response` - The session init response from the device
    /// * `device_sign_pub` - The device's Ed25519 signing public key (for verification)
    ///
    /// # Returns
    /// * `Ok(())` - If the response is valid and we can proceed to connect
    /// * `Err(SessionError)` - If verification fails or state is invalid
    pub async fn handle_response(
        &mut self,
        response: SessionInitResponseV1,
        device_sign_pub: &[u8],
    ) -> Result<(), SessionError> {
        // Validate state transition
        let (expected_device_id, _sent_at) = match &self.state {
            SessionControllerState::RequestSent {
                device_id,
                sent_at,
                ..
            } => (device_id.clone(), *sent_at),
            _ => {
                return Err(SessionError::InvalidState(
                    "can only handle response from RequestSent state".into(),
                ));
            }
        };

        // Validate response fields
        if response.session_id.is_empty() {
            return Err(SessionError::MissingField("session_id".into()));
        }

        // Verify device_id matches (if present in response)
        if !response.device_id.is_empty() && response.device_id != expected_device_id {
            return Err(SessionError::TicketInvalid(
                "device_id mismatch".into(),
            ));
        }

        // Verify device signature (Requirements: 4.3)
        verify_session_init_response_v1(&response, device_sign_pub)
            .map_err(|_| SessionError::SignatureInvalid)?;

        // Extract ticket (Requirements: 4.4)
        let ticket = response
            .issued_ticket
            .ok_or(SessionError::MissingField("issued_ticket".into()))?;

        // Validate ticket
        if ticket.ticket_id.is_empty() {
            return Err(SessionError::TicketInvalid("empty ticket_id".into()));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if ticket.expires_at <= now {
            return Err(SessionError::TicketExpired);
        }

        // Extract session_id
        let mut session_id = [0u8; 32];
        if response.session_id.len() >= 32 {
            session_id.copy_from_slice(&response.session_id[..32]);
        } else {
            return Err(SessionError::MissingField(
                "session_id must be 32 bytes".into(),
            ));
        }

        // Transition to TicketReceived state (Requirements: 4.5)
        self.state = SessionControllerState::TicketReceived {
            session_id,
            ticket,
            transport_params: response.transport_params,
            permissions: response.granted_capabilities,
        };

        Ok(())
    }

    /// Initiate transport connection after receiving ticket.
    /// Requirements: 4.5
    ///
    /// # Returns
    /// * `Ok(transport)` - The selected transport to use for connection
    /// * `Err(SessionError)` - If no compatible transport is available
    pub fn initiate_connection(&mut self) -> Result<crate::transport::SelectedTransport, SessionError> {
        // Validate state
        let (session_id, ticket, transport_params, permissions) = match &self.state {
            SessionControllerState::TicketReceived {
                session_id,
                ticket,
                transport_params,
                permissions,
            } => (*session_id, ticket.clone(), transport_params.clone(), *permissions),
            _ => {
                return Err(SessionError::InvalidState(
                    "can only initiate connection from TicketReceived state".into(),
                ));
            }
        };

        // Convert proto transport params to internal format
        let negotiation = if let Some(params) = transport_params {
            crate::transport::TransportNegotiation {
                quic_params: params.quic_params.map(|q| crate::transport::QuicParams {
                    certificate: q.server_cert_der,
                    alpn_protocols: if q.alpn.is_empty() {
                        vec![]
                    } else {
                        vec![q.alpn]
                    },
                    server_addr: q.endpoints.first().map(|e| format!("{}:{}", e.host, e.port)),
                }),
                relay_tokens: params
                    .relay_tokens
                    .into_iter()
                    .map(|t| crate::transport::RelayToken {
                        // RelayTokenV1 has relay_id, not relay_url - use a placeholder
                        relay_url: String::new(),
                        token: t.signature,
                        expires_at: t.expires_at,
                        bandwidth_limit: if t.bandwidth_limit > 0 {
                            Some(t.bandwidth_limit)
                        } else {
                            None
                        },
                    })
                    .collect(),
                supported_transports: vec![
                    crate::transport::TransportType::Direct,
                    crate::transport::TransportType::Relay,
                ],
                ice_candidates: vec![],
            }
        } else {
            crate::transport::TransportNegotiation::default()
        };

        // Select transport
        let selected = self
            .transport_negotiator
            .select_transport(&negotiation)
            .map_err(|e| SessionError::TransportError(e.to_string()))?;

        // Transition to Connecting state
        self.state = SessionControllerState::Connecting {
            session_id,
            ticket,
            permissions,
        };

        Ok(selected)
    }

    /// Mark the session as active after transport connection is established.
    /// Requirements: 4.6
    pub fn mark_connected(&mut self) -> Result<(), SessionError> {
        let (session_id, ticket, permissions) = match &self.state {
            SessionControllerState::Connecting {
                session_id,
                ticket,
                permissions,
            } => (*session_id, ticket.clone(), *permissions),
            _ => {
                return Err(SessionError::InvalidState(
                    "can only mark connected from Connecting state".into(),
                ));
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.state = SessionControllerState::Active {
            session: ControllerActiveSession {
                session_id,
                device_id: ticket.device_id.clone(),
                permissions,
                ticket_expires_at: ticket.expires_at,
                ticket,
                started_at: now,
            },
        };

        Ok(())
    }

    /// Check if ticket renewal is needed.
    /// Requirements: 4.7
    ///
    /// # Returns
    /// * `true` if ticket should be renewed (within renewal threshold of expiry)
    /// * `false` if ticket is still valid with sufficient time remaining
    pub fn needs_ticket_renewal(&self) -> bool {
        match &self.state {
            SessionControllerState::Active { session } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Check if we're within the renewal threshold
                session.ticket_expires_at <= now + self.renewal_threshold_secs
            }
            _ => false,
        }
    }

    /// Check if the ticket has expired.
    /// Requirements: 4.7
    pub fn is_ticket_expired(&self) -> bool {
        match &self.state {
            SessionControllerState::Active { session } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                session.ticket_expires_at <= now
            }
            SessionControllerState::TicketReceived { ticket, .. }
            | SessionControllerState::Connecting { ticket, .. } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                ticket.expires_at <= now
            }
            _ => false,
        }
    }

    /// Handle transport disconnection.
    /// Requirements: 4.8
    ///
    /// # Arguments
    /// * `can_reconnect` - Whether reconnection should be attempted
    ///
    /// # Returns
    /// * `Ok(true)` - If reconnection is possible (ticket still valid)
    /// * `Ok(false)` - If reconnection is not possible
    /// * `Err(SessionError)` - If state is invalid
    pub fn handle_disconnection(&mut self, can_reconnect: bool) -> Result<bool, SessionError> {
        match &self.state {
            SessionControllerState::Active { session } => {
                if !can_reconnect || self.is_ticket_expired() {
                    // End the session
                    self.state = SessionControllerState::Ended {
                        reason: SessionEndReason::TransportLost,
                    };
                    return Ok(false);
                }

                // Transition back to TicketReceived for reconnection attempt
                self.state = SessionControllerState::TicketReceived {
                    session_id: session.session_id,
                    ticket: session.ticket.clone(),
                    transport_params: None, // Will need to re-negotiate
                    permissions: session.permissions,
                };

                Ok(true)
            }
            SessionControllerState::Connecting { .. } => {
                self.state = SessionControllerState::Ended {
                    reason: SessionEndReason::TransportLost,
                };
                Ok(false)
            }
            _ => Err(SessionError::InvalidState(
                "not in a connected state".into(),
            )),
        }
    }

    /// End the session.
    /// Requirements: 4.8
    pub fn end_session(&mut self, reason: SessionEndReason) -> Result<(), SessionError> {
        match &self.state {
            SessionControllerState::Active { .. }
            | SessionControllerState::Connecting { .. }
            | SessionControllerState::TicketReceived { .. }
            | SessionControllerState::RequestSent { .. } => {
                self.state = SessionControllerState::Ended { reason };
                Ok(())
            }
            SessionControllerState::Idle => Err(SessionError::InvalidState(
                "no session to end".into(),
            )),
            SessionControllerState::Ended { .. } => Ok(()), // Already ended
        }
    }

    /// Get the active session if in Active state.
    pub fn active_session(&self) -> Option<&ControllerActiveSession> {
        match &self.state {
            SessionControllerState::Active { session } => Some(session),
            _ => None,
        }
    }

    /// Check if session is active.
    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionControllerState::Active { .. })
    }

    /// Get the current ticket if available.
    pub fn current_ticket(&self) -> Option<&SessionTicketV1> {
        match &self.state {
            SessionControllerState::TicketReceived { ticket, .. }
            | SessionControllerState::Connecting { ticket, .. } => Some(ticket),
            SessionControllerState::Active { session } => Some(&session.ticket),
            _ => None,
        }
    }
}


// ============================================================================
// Helper Functions for Signing/Verification
// ============================================================================

/// Compute the bytes to sign for a SessionTicketV1.
fn session_ticket_signing_bytes_v1(t: &SessionTicketV1) -> Result<Vec<u8>, String> {
    let mut tt = t.clone();
    tt.device_signature = vec![];
    let mut buf = Vec::with_capacity(tt.encoded_len());
    tt.encode(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Sign a SessionTicketV1 with the device's signing key.
fn sign_session_ticket_v1(
    device_sign: &ed25519_dalek::SigningKey,
    t: &mut SessionTicketV1,
) -> Result<(), String> {
    let bytes = session_ticket_signing_bytes_v1(t)?;
    let digest = sha256(&bytes);
    let sig = device_sign.sign(&digest);
    t.device_signature = sig.to_bytes().to_vec();
    Ok(())
}

/// Compute the bytes to sign for a SessionInitResponseV1.
fn session_init_response_signing_bytes_v1(r: &SessionInitResponseV1) -> Result<Vec<u8>, String> {
    let mut rr = r.clone();
    rr.device_signature = vec![];
    let mut buf = Vec::with_capacity(rr.encoded_len());
    rr.encode(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Sign a SessionInitResponseV1 with the device's signing key.
fn sign_session_init_response_v1(
    device_sign: &ed25519_dalek::SigningKey,
    r: &mut SessionInitResponseV1,
) -> Result<(), String> {
    let bytes = session_init_response_signing_bytes_v1(r)?;
    let digest = sha256(&bytes);
    let sig = device_sign.sign(&digest);
    r.device_signature = sig.to_bytes().to_vec();
    Ok(())
}

/// Compute the bytes to sign for a SessionInitRequestV1.
fn session_init_request_signing_bytes_v1(r: &SessionInitRequestV1) -> Result<Vec<u8>, String> {
    let mut rr = r.clone();
    rr.operator_signature = vec![];
    let mut buf = Vec::with_capacity(rr.encoded_len());
    rr.encode(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Sign a SessionInitRequestV1 with the operator's signing key.
fn sign_session_init_request_v1(
    operator_sign: &ed25519_dalek::SigningKey,
    r: &mut SessionInitRequestV1,
) -> Result<(), String> {
    let bytes = session_init_request_signing_bytes_v1(r)?;
    let digest = sha256(&bytes);
    let sig = operator_sign.sign(&digest);
    r.operator_signature = sig.to_bytes().to_vec();
    Ok(())
}

/// Verify a SessionInitResponseV1 signature.
fn verify_session_init_response_v1(
    r: &SessionInitResponseV1,
    device_sign_pub: &[u8],
) -> Result<(), String> {
    use ed25519_dalek::{Signature, VerifyingKey};

    if device_sign_pub.len() != 32 {
        return Err("device_sign_pub must be 32 bytes".into());
    }
    if r.device_signature.len() != 64 {
        return Err("device_signature must be 64 bytes".into());
    }

    let bytes = session_init_response_signing_bytes_v1(r)?;
    let digest = sha256(&bytes);

    let pub_bytes: [u8; 32] = device_sign_pub
        .try_into()
        .map_err(|_| "invalid public key length")?;
    let verifying_key =
        VerifyingKey::from_bytes(&pub_bytes).map_err(|e| format!("invalid public key: {}", e))?;

    let sig_bytes: [u8; 64] = r
        .device_signature
        .as_slice()
        .try_into()
        .map_err(|_| "invalid signature length")?;
    let signature = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(&digest, &signature)
        .map_err(|e| format!("signature verification failed: {}", e))
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::generate_identity_keys;
    use crate::policy::ConsentMode;
    use crate::store::InMemoryStore;
    use zrc_proto::v1::{KeyTypeV1, PublicKeyV1};

    /// Simple consent handler that always approves.
    struct AlwaysApproveSession;

    #[async_trait]
    impl SessionConsentHandler for AlwaysApproveSession {
        async fn request_consent(
            &self,
            _operator_id: &[u8],
            requested_permissions: u32,
            _paired_permissions: u32,
        ) -> Result<SessionConsentDecision, SessionError> {
            Ok(SessionConsentDecision {
                approved: true,
                granted_permissions: requested_permissions,
            })
        }
    }

    /// Simple consent handler that always denies.
    struct AlwaysDenySession;

    #[async_trait]
    impl SessionConsentHandler for AlwaysDenySession {
        async fn request_consent(
            &self,
            _operator_id: &[u8],
            _requested_permissions: u32,
            _paired_permissions: u32,
        ) -> Result<SessionConsentDecision, SessionError> {
            Ok(SessionConsentDecision {
                approved: false,
                granted_permissions: 0,
            })
        }
    }

    fn make_test_pairing(device_id: &[u8], operator_id: &[u8]) -> PairingRecord {
        PairingRecord {
            pairing_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            device_id: device_id.to_vec(),
            operator_id: operator_id.to_vec(),
            device_sign_pub: PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            device_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            operator_sign_pub: PublicKeyV1 {
                key_type: KeyTypeV1::Ed25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            operator_kex_pub: PublicKeyV1 {
                key_type: KeyTypeV1::X25519 as i32,
                key_bytes: vec![0u8; 32],
            },
            // Store permissions as bitmask values directly (VIEW=0x01, CONTROL=0x02)
            // The granted_perms field stores the raw permission bits
            granted_perms: vec![0, 1], // Bit positions: 1<<0=VIEW, 1<<1=CONTROL -> 0x03
            unattended_enabled: false,
            require_consent_each_time: true,
            issued_at: 1000,
            last_session: None,
        }
    }

    #[tokio::test]
    async fn test_session_host_initial_state() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::default());
        let consent = Arc::new(AlwaysApproveSession);

        let host = SessionHost::new(device_keys, store, policy, consent);

        assert!(matches!(host.state(), SessionHostState::Idle));
    }

    #[tokio::test]
    async fn test_session_host_not_paired() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::default());
        let consent = Arc::new(AlwaysApproveSession);

        let mut host = SessionHost::new(device_keys, store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: vec![1u8; 32],
            device_id: vec![2u8; 32],
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03, // VIEW | CONTROL
            ..Default::default()
        };

        let result = host.handle_request(request).await;
        assert!(matches!(result, Err(SessionError::NotPaired)));
    }

    #[tokio::test]
    async fn test_session_host_requires_consent() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::AlwaysRequire));
        let consent = Arc::new(AlwaysApproveSession);

        // Create pairing
        let operator_id = vec![1u8; 32];
        let pairing = make_test_pairing(&device_keys.id32, &operator_id);
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03, // VIEW | CONTROL
            ..Default::default()
        };

        let result = host.handle_request(request).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            SessionAction::AwaitingConsent { operator_id: op_id, .. } => {
                assert_eq!(op_id, operator_id);
            }
            _ => panic!("Expected AwaitingConsent action"),
        }

        assert!(matches!(host.state(), SessionHostState::AwaitingConsent { .. }));
    }

    #[tokio::test]
    async fn test_session_host_auto_approve_unattended() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::UnattendedAllowed));
        let consent = Arc::new(AlwaysApproveSession);

        // Create pairing with unattended enabled
        let operator_id = vec![1u8; 32];
        let mut pairing = make_test_pairing(&device_keys.id32, &operator_id);
        pairing.unattended_enabled = true;
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03, // VIEW | CONTROL
            ..Default::default()
        };

        let result = host.handle_request(request).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            SessionAction::AutoApproved { response } => {
                assert!(!response.session_id.is_empty());
                assert!(response.issued_ticket.is_some());
            }
            _ => panic!("Expected AutoApproved action"),
        }

        assert!(matches!(host.state(), SessionHostState::Active { .. }));
    }

    #[tokio::test]
    async fn test_session_host_approve_after_consent() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::AlwaysRequire));
        let consent = Arc::new(AlwaysApproveSession);

        // Create pairing
        let operator_id = vec![1u8; 32];
        let pairing = make_test_pairing(&device_keys.id32, &operator_id);
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03,
            ..Default::default()
        };

        // Handle request - should go to AwaitingConsent
        let _ = host.handle_request(request).await.unwrap();
        assert!(matches!(host.state(), SessionHostState::AwaitingConsent { .. }));

        // Approve - should go to Active
        let response = host.approve().await.unwrap();
        assert!(!response.session_id.is_empty());
        assert!(response.issued_ticket.is_some());
        assert!(matches!(host.state(), SessionHostState::Active { .. }));
    }

    #[tokio::test]
    async fn test_session_host_reject() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::AlwaysRequire));
        let consent = Arc::new(AlwaysDenySession);

        // Create pairing
        let operator_id = vec![1u8; 32];
        let pairing = make_test_pairing(&device_keys.id32, &operator_id);
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03,
            ..Default::default()
        };

        // Handle request - should go to AwaitingConsent
        let _ = host.handle_request(request).await.unwrap();

        // Reject
        host.reject("user denied").await.unwrap();
        assert!(matches!(host.state(), SessionHostState::Ended { .. }));
    }

    #[tokio::test]
    async fn test_session_host_end_session() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::UnattendedAllowed));
        let consent = Arc::new(AlwaysApproveSession);

        // Create pairing with unattended enabled
        let operator_id = vec![1u8; 32];
        let mut pairing = make_test_pairing(&device_keys.id32, &operator_id);
        pairing.unattended_enabled = true;
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store.clone(), policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03,
            ..Default::default()
        };

        // Start session
        let _ = host.handle_request(request).await.unwrap();
        assert!(matches!(host.state(), SessionHostState::Active { .. }));

        // End session
        host.end_session(SessionEndReason::OperatorDisconnect).await.unwrap();
        assert!(matches!(host.state(), SessionHostState::Ended { reason: SessionEndReason::OperatorDisconnect }));
    }

    #[tokio::test]
    async fn test_session_host_reset() {
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());
        let policy = Arc::new(PolicyEngine::new(ConsentMode::AlwaysRequire));
        let consent = Arc::new(AlwaysApproveSession);

        // Create pairing
        let operator_id = vec![1u8; 32];
        let pairing = make_test_pairing(&device_keys.id32, &operator_id);
        store.save_pairing(pairing).await.unwrap();

        let mut host = SessionHost::new(device_keys.clone(), store, policy, consent);

        let request = SessionInitRequestV1 {
            operator_id: operator_id.clone(),
            device_id: device_keys.id32.to_vec(),
            session_id: vec![3u8; 32],
            requested_capabilities: 0x03,
            ..Default::default()
        };

        // Handle request
        let _ = host.handle_request(request).await.unwrap();
        assert!(matches!(host.state(), SessionHostState::AwaitingConsent { .. }));

        // Reset
        host.reset();
        assert!(matches!(host.state(), SessionHostState::Idle));
    }

    // =========================================================================
    // Session Controller Tests
    // =========================================================================

    #[tokio::test]
    async fn test_session_controller_initial_state() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        assert!(matches!(controller.state(), SessionControllerState::Idle));
    }

    #[tokio::test]
    async fn test_session_controller_start_session_not_paired() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = SessionController::new(operator_keys, store);

        let device_id = vec![1u8; 32];
        let result = controller.start_session(&device_id, 0x03).await;

        assert!(matches!(result, Err(SessionError::NotPaired)));
    }

    #[tokio::test]
    async fn test_session_controller_start_session_success() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        // Create pairing (from device's perspective, so device_id is device, operator_id is operator)
        let pairing = make_test_pairing(&device_keys.id32, &operator_keys.id32);
        store.save_pairing(pairing).await.unwrap();

        let mut controller = SessionController::new(operator_keys.clone(), store);

        let result = controller.start_session(&device_keys.id32, 0x03).await;
        assert!(result.is_ok());

        let request = result.unwrap();
        assert_eq!(request.operator_id, operator_keys.id32.to_vec());
        assert_eq!(request.device_id, device_keys.id32.to_vec());
        assert!(!request.session_id.is_empty());
        assert!(!request.operator_signature.is_empty());

        assert!(matches!(controller.state(), SessionControllerState::RequestSent { .. }));
    }

    #[tokio::test]
    async fn test_session_controller_start_session_invalid_device_id() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = SessionController::new(operator_keys, store);

        // Invalid device_id (not 32 bytes)
        let device_id = vec![1u8; 16];
        let result = controller.start_session(&device_id, 0x03).await;

        assert!(matches!(result, Err(SessionError::MissingField(_))));
    }

    #[tokio::test]
    async fn test_session_controller_start_session_exceeds_permissions() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        // Create pairing with limited permissions (only VIEW = bit 0)
        let mut pairing = make_test_pairing(&device_keys.id32, &operator_keys.id32);
        pairing.granted_perms = vec![0]; // Only VIEW (bit position 0)
        store.save_pairing(pairing).await.unwrap();

        let mut controller = SessionController::new(operator_keys.clone(), store);

        // Request CONTROL (bit 1) which exceeds paired permissions
        let result = controller.start_session(&device_keys.id32, 0x02).await;

        assert!(matches!(result, Err(SessionError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_session_controller_handle_response_invalid_state() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = SessionController::new(operator_keys, store);

        let response = SessionInitResponseV1::default();
        let result = controller.handle_response(response, &[0u8; 32]).await;

        assert!(matches!(result, Err(SessionError::InvalidState(_))));
    }

    #[tokio::test]
    async fn test_session_controller_end_session() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        // Create pairing
        let pairing = make_test_pairing(&device_keys.id32, &operator_keys.id32);
        store.save_pairing(pairing).await.unwrap();

        let mut controller = SessionController::new(operator_keys.clone(), store);

        // Start session
        let _ = controller.start_session(&device_keys.id32, 0x03).await.unwrap();
        assert!(matches!(controller.state(), SessionControllerState::RequestSent { .. }));

        // End session
        controller.end_session(SessionEndReason::OperatorDisconnect).unwrap();
        assert!(matches!(controller.state(), SessionControllerState::Ended { .. }));
    }

    #[tokio::test]
    async fn test_session_controller_reset() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        // Create pairing
        let pairing = make_test_pairing(&device_keys.id32, &operator_keys.id32);
        store.save_pairing(pairing).await.unwrap();

        let mut controller = SessionController::new(operator_keys.clone(), store);

        // Start session
        let _ = controller.start_session(&device_keys.id32, 0x03).await.unwrap();
        assert!(matches!(controller.state(), SessionControllerState::RequestSent { .. }));

        // Reset
        controller.reset();
        assert!(matches!(controller.state(), SessionControllerState::Idle));
    }

    #[tokio::test]
    async fn test_session_controller_cannot_start_from_request_sent() {
        let operator_keys = generate_identity_keys();
        let device_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        // Create pairing
        let pairing = make_test_pairing(&device_keys.id32, &operator_keys.id32);
        store.save_pairing(pairing).await.unwrap();

        let mut controller = SessionController::new(operator_keys.clone(), store);

        // Start session
        let _ = controller.start_session(&device_keys.id32, 0x03).await.unwrap();

        // Try to start another session - should fail
        let result = controller.start_session(&device_keys.id32, 0x03).await;
        assert!(matches!(result, Err(SessionError::InvalidState(_))));
    }

    #[tokio::test]
    async fn test_session_controller_needs_ticket_renewal_not_active() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        // Not in Active state, should return false
        assert!(!controller.needs_ticket_renewal());
    }

    #[tokio::test]
    async fn test_session_controller_is_ticket_expired_not_active() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        // Not in a state with a ticket, should return false
        assert!(!controller.is_ticket_expired());
    }

    #[tokio::test]
    async fn test_session_controller_handle_disconnection_invalid_state() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = SessionController::new(operator_keys, store);

        // Not in a connected state
        let result = controller.handle_disconnection(true);
        assert!(matches!(result, Err(SessionError::InvalidState(_))));
    }

    #[tokio::test]
    async fn test_session_controller_end_session_from_idle() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let mut controller = SessionController::new(operator_keys, store);

        // Cannot end session from Idle state
        let result = controller.end_session(SessionEndReason::OperatorDisconnect);
        assert!(matches!(result, Err(SessionError::InvalidState(_))));
    }

    #[tokio::test]
    async fn test_session_controller_current_ticket_none() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        // No ticket in Idle state
        assert!(controller.current_ticket().is_none());
    }

    #[tokio::test]
    async fn test_session_controller_is_active() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        // Not active in Idle state
        assert!(!controller.is_active());
    }

    #[tokio::test]
    async fn test_session_controller_active_session_none() {
        let operator_keys = generate_identity_keys();
        let store = Arc::new(InMemoryStore::new());

        let controller = SessionController::new(operator_keys, store);

        // No active session in Idle state
        assert!(controller.active_session().is_none());
    }
}
