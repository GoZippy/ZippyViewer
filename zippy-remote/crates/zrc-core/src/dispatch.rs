//! Message dispatch module for routing incoming envelopes.
//!
//! This module implements centralized message dispatch for routing incoming
//! messages to appropriate handlers based on message type.
//!
//! Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use x25519_dalek::StaticSecret;

use crate::errors::CoreError;
use zrc_crypto::envelope::{envelope_open_v1, EnvelopeError};
use zrc_proto::v1::{EnvelopeV1, MsgTypeV1, PairReceiptV1, SessionInitResponseV1};

// ============================================================================
// Error Types
// ============================================================================

/// Errors specific to dispatch operations.
#[derive(Debug, Clone)]
pub enum DispatchError {
    /// No handler registered for message type
    NoHandler(MsgTypeV1),
    /// Signature verification failed
    SignatureInvalid,
    /// Decryption failed
    DecryptionFailed(String),
    /// Handler returned an error
    HandlerError(String),
    /// Missing required field
    MissingField(String),
    /// Decode error
    DecodeError(String),
    /// Unknown message type
    UnknownMsgType(i32),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchError::NoHandler(t) => write!(f, "no handler registered for {:?}", t),
            DispatchError::SignatureInvalid => write!(f, "signature verification failed"),
            DispatchError::DecryptionFailed(s) => write!(f, "decryption failed: {}", s),
            DispatchError::HandlerError(s) => write!(f, "handler error: {}", s),
            DispatchError::MissingField(s) => write!(f, "missing field: {}", s),
            DispatchError::DecodeError(s) => write!(f, "decode error: {}", s),
            DispatchError::UnknownMsgType(t) => write!(f, "unknown message type: {}", t),
        }
    }
}

impl std::error::Error for DispatchError {}

impl From<EnvelopeError> for DispatchError {
    fn from(e: EnvelopeError) -> Self {
        match e {
            EnvelopeError::BadSignature | EnvelopeError::SenderIdMismatch => {
                DispatchError::SignatureInvalid
            }
            EnvelopeError::DecryptFailed => {
                DispatchError::DecryptionFailed("AEAD decryption failed".into())
            }
            EnvelopeError::Missing(field) => DispatchError::MissingField(field.into()),
            EnvelopeError::InvalidKeyBytes => {
                DispatchError::DecryptionFailed("invalid key bytes".into())
            }
            EnvelopeError::UnsupportedSuite => {
                DispatchError::DecryptionFailed("unsupported cipher suite".into())
            }
            EnvelopeError::EncryptFailed => {
                DispatchError::DecryptionFailed("encryption failed".into())
            }
        }
    }
}

// ============================================================================
// Handler Error Type
// ============================================================================

/// Errors returned by message handlers.
#[derive(Debug, Clone)]
pub enum HandlerError {
    /// Invalid payload format
    InvalidPayload(String),
    /// Processing failed
    ProcessingFailed(String),
    /// Permission denied
    PermissionDenied(String),
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::InvalidPayload(s) => write!(f, "invalid payload: {}", s),
            HandlerError::ProcessingFailed(s) => write!(f, "processing failed: {}", s),
            HandlerError::PermissionDenied(s) => write!(f, "permission denied: {}", s),
            HandlerError::Internal(s) => write!(f, "internal error: {}", s),
        }
    }
}

impl std::error::Error for HandlerError {}

// ============================================================================
// Message Handler Trait
// ============================================================================

/// Trait for handling specific message types.
/// Requirements: 6.2, 6.5
///
/// Handlers are registered with the Dispatcher and invoked when a message
/// of the corresponding type is received.
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message.
    ///
    /// # Arguments
    /// * `sender_id` - The verified sender identifier (32 bytes)
    /// * `payload` - The decrypted message payload bytes
    ///
    /// # Returns
    /// * `Ok(Some(response))` - Response bytes to send back to sender
    /// * `Ok(None)` - No response needed
    /// * `Err(HandlerError)` - Handler failed to process the message
    async fn handle(
        &self,
        sender_id: [u8; 32],
        payload: &[u8],
    ) -> Result<Option<Vec<u8>>, HandlerError>;
}

// ============================================================================
// Dispatch Statistics
// ============================================================================

/// Statistics for message dispatch operations.
/// Requirements: 6.6, 6.7
#[derive(Debug, Default)]
pub struct DispatchStats {
    /// Total messages received
    pub received: AtomicU64,
    /// Messages successfully dispatched to handlers
    pub dispatched: AtomicU64,
    /// Messages dropped (signature failed, no handler, etc.)
    pub dropped: AtomicU64,
    /// Messages dropped due to signature verification failure
    pub signature_failures: AtomicU64,
    /// Messages dropped due to decryption failure
    pub decryption_failures: AtomicU64,
    /// Messages dropped due to unknown message type
    pub unknown_type: AtomicU64,
    /// Messages dropped due to handler errors
    pub handler_errors: AtomicU64,
}

impl DispatchStats {
    /// Create new dispatch statistics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a snapshot of current statistics.
    pub fn snapshot(&self) -> DispatchStatsSnapshot {
        DispatchStatsSnapshot {
            received: self.received.load(Ordering::Relaxed),
            dispatched: self.dispatched.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            signature_failures: self.signature_failures.load(Ordering::Relaxed),
            decryption_failures: self.decryption_failures.load(Ordering::Relaxed),
            unknown_type: self.unknown_type.load(Ordering::Relaxed),
            handler_errors: self.handler_errors.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics to zero.
    pub fn reset(&self) {
        self.received.store(0, Ordering::Relaxed);
        self.dispatched.store(0, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        self.signature_failures.store(0, Ordering::Relaxed);
        self.decryption_failures.store(0, Ordering::Relaxed);
        self.unknown_type.store(0, Ordering::Relaxed);
        self.handler_errors.store(0, Ordering::Relaxed);
    }

    fn inc_received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_dispatched(&self) {
        self.dispatched.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_signature_failures(&self) {
        self.signature_failures.fetch_add(1, Ordering::Relaxed);
        self.inc_dropped();
    }

    fn inc_decryption_failures(&self) {
        self.decryption_failures.fetch_add(1, Ordering::Relaxed);
        self.inc_dropped();
    }

    fn inc_unknown_type(&self) {
        self.unknown_type.fetch_add(1, Ordering::Relaxed);
        self.inc_dropped();
    }

    fn inc_handler_errors(&self) {
        self.handler_errors.fetch_add(1, Ordering::Relaxed);
        self.inc_dropped();
    }
}

/// Snapshot of dispatch statistics at a point in time.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchStatsSnapshot {
    pub received: u64,
    pub dispatched: u64,
    pub dropped: u64,
    pub signature_failures: u64,
    pub decryption_failures: u64,
    pub unknown_type: u64,
    pub handler_errors: u64,
}

// ============================================================================
// Sender Key Resolver Trait
// ============================================================================

/// Trait for resolving sender signing public keys.
///
/// The dispatcher needs to verify envelope signatures, which requires
/// knowing the sender's Ed25519 signing public key. This trait allows
/// different key resolution strategies (e.g., from pairings, directory, etc.)
#[async_trait]
pub trait SenderKeyResolver: Send + Sync {
    /// Resolve the signing public key for a sender.
    ///
    /// # Arguments
    /// * `sender_id` - The sender identifier (32 bytes, derived from sign_pub)
    ///
    /// # Returns
    /// * `Ok(Some(key))` - The 32-byte Ed25519 signing public key
    /// * `Ok(None)` - Sender not found/unknown
    /// * `Err` - Resolution failed
    async fn resolve_sign_pub(&self, sender_id: &[u8]) -> Result<Option<[u8; 32]>, String>;
}

// ============================================================================
// Dispatcher
// ============================================================================

/// Message dispatcher for routing incoming envelopes to handlers.
/// Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7
///
/// The dispatcher:
/// 1. Decodes incoming EnvelopeV1 messages
/// 2. Verifies envelope signatures using the sender's public key
/// 3. Decrypts the payload
/// 4. Routes to the appropriate handler based on msg_type
/// 5. Tracks statistics for observability
pub struct Dispatcher {
    /// Registered message handlers by message type
    handlers: RwLock<HashMap<MsgTypeV1, Arc<dyn MessageHandler>>>,
    /// Recipient's X25519 private key for decryption
    recipient_kex_priv: StaticSecret,
    /// Sender key resolver for signature verification
    key_resolver: Arc<dyn SenderKeyResolver>,
    /// Dispatch statistics
    stats: Arc<DispatchStats>,
}

impl Dispatcher {
    /// Create a new dispatcher.
    ///
    /// # Arguments
    /// * `recipient_kex_priv` - The recipient's X25519 private key for decryption
    /// * `key_resolver` - Resolver for sender signing public keys
    pub fn new(
        recipient_kex_priv: StaticSecret,
        key_resolver: Arc<dyn SenderKeyResolver>,
    ) -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            recipient_kex_priv,
            key_resolver,
            stats: Arc::new(DispatchStats::new()),
        }
    }

    /// Register a handler for a specific message type.
    /// Requirements: 6.2, 6.5
    ///
    /// # Arguments
    /// * `msg_type` - The message type to handle
    /// * `handler` - The handler implementation
    pub async fn register_handler(
        &self,
        msg_type: MsgTypeV1,
        handler: Arc<dyn MessageHandler>,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(msg_type, handler);
        debug!("registered handler for {:?}", msg_type);
    }

    /// Unregister a handler for a message type.
    pub async fn unregister_handler(&self, msg_type: MsgTypeV1) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(&msg_type);
        debug!("unregistered handler for {:?}", msg_type);
    }

    /// Check if a handler is registered for a message type.
    pub async fn has_handler(&self, msg_type: MsgTypeV1) -> bool {
        let handlers = self.handlers.read().await;
        handlers.contains_key(&msg_type)
    }

    /// Get the dispatch statistics.
    pub fn stats(&self) -> &Arc<DispatchStats> {
        &self.stats
    }

    /// Dispatch an incoming envelope.
    /// Requirements: 6.3, 6.4
    ///
    /// This method:
    /// 1. Decodes the envelope
    /// 2. Verifies the signature
    /// 3. Decrypts the payload
    /// 4. Routes to the appropriate handler
    ///
    /// # Arguments
    /// * `envelope` - The incoming EnvelopeV1
    ///
    /// # Returns
    /// * `Ok(Some(response))` - Response bytes from the handler
    /// * `Ok(None)` - No response needed
    /// * `Err(DispatchError)` - Dispatch failed
    pub async fn dispatch(
        &self,
        envelope: EnvelopeV1,
    ) -> Result<Option<Vec<u8>>, DispatchError> {
        self.stats.inc_received();

        // Extract header and message type
        let header = envelope
            .header
            .as_ref()
            .ok_or(DispatchError::MissingField("header".into()))?;

        let msg_type = MsgTypeV1::try_from(header.msg_type)
            .unwrap_or(MsgTypeV1::Unspecified);

        // Check for unknown message type (Requirements: 6.6)
        if msg_type == MsgTypeV1::Unspecified {
            warn!("received unknown message type: {}", header.msg_type);
            self.stats.inc_unknown_type();
            return Err(DispatchError::UnknownMsgType(header.msg_type));
        }

        // Resolve sender's signing public key
        let sender_sign_pub = self
            .key_resolver
            .resolve_sign_pub(&header.sender_id)
            .await
            .map_err(|e| DispatchError::DecryptionFailed(e))?
            .ok_or_else(|| {
                warn!("unknown sender: {}", hex::encode(&header.sender_id));
                self.stats.inc_signature_failures();
                DispatchError::SignatureInvalid
            })?;

        // Verify signature and decrypt (Requirements: 6.3)
        let (plaintext, verified_sender_id) = envelope_open_v1(
            &envelope,
            &self.recipient_kex_priv,
            &sender_sign_pub,
        )
        .map_err(|e| {
            match &e {
                EnvelopeError::BadSignature | EnvelopeError::SenderIdMismatch => {
                    warn!("signature verification failed for sender: {}", 
                          hex::encode(&header.sender_id));
                    self.stats.inc_signature_failures();
                }
                EnvelopeError::DecryptFailed => {
                    warn!("decryption failed for message from: {}", 
                          hex::encode(&header.sender_id));
                    self.stats.inc_decryption_failures();
                }
                _ => {
                    self.stats.inc_dropped();
                }
            }
            DispatchError::from(e)
        })?;

        // Convert sender_id to fixed array
        let mut sender_id_arr = [0u8; 32];
        if verified_sender_id.len() >= 32 {
            sender_id_arr.copy_from_slice(&verified_sender_id[..32]);
        }

        // Get handler for this message type
        let handler = {
            let handlers = self.handlers.read().await;
            handlers.get(&msg_type).cloned()
        };

        let handler = match handler {
            Some(h) => h,
            None => {
                warn!("no handler registered for {:?}", msg_type);
                self.stats.inc_dropped();
                return Err(DispatchError::NoHandler(msg_type));
            }
        };

        // Dispatch to handler (Requirements: 6.4)
        let result = handler.handle(sender_id_arr, &plaintext).await;

        match result {
            Ok(response) => {
                self.stats.inc_dispatched();
                debug!("dispatched {:?} message from {}", 
                       msg_type, hex::encode(&sender_id_arr[..8]));
                Ok(response)
            }
            Err(e) => {
                warn!("handler error for {:?}: {}", msg_type, e);
                self.stats.inc_handler_errors();
                Err(DispatchError::HandlerError(e.to_string()))
            }
        }
    }

    /// Dispatch raw envelope bytes.
    ///
    /// Convenience method that decodes the envelope first.
    pub async fn dispatch_bytes(&self, env_bytes: &[u8]) -> Result<Option<Vec<u8>>, DispatchError> {
        let envelope = EnvelopeV1::decode(env_bytes)
            .map_err(|e| DispatchError::DecodeError(e.to_string()))?;
        self.dispatch(envelope).await
    }
}

// ============================================================================
// Legacy Types (for backward compatibility)
// ============================================================================

#[derive(Debug)]
pub enum ControllerEvent {
    PairReceipt(PairReceiptV1),
    SessionInitResponse(SessionInitResponseV1),
}

/// Decode an envelope and extract the message type.
pub fn decode_envelope(env_bytes: &[u8]) -> Result<(EnvelopeV1, MsgTypeV1), CoreError> {
    let env = EnvelopeV1::decode(env_bytes).map_err(|e| CoreError::Decode(e.to_string()))?;
    let header = env.header.as_ref().ok_or(CoreError::BadRequest("missing envelope header".into()))?;
    let msg_type = MsgTypeV1::try_from(header.msg_type).unwrap_or(MsgTypeV1::Unspecified);
    Ok((env, msg_type))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;
    use x25519_dalek::PublicKey as X25519PublicKey;
    use zrc_crypto::envelope::envelope_seal_v1;
    use zrc_crypto::hash::{derive_id, sha256};

    /// Simple in-memory key resolver for testing.
    struct TestKeyResolver {
        keys: RwLock<HashMap<Vec<u8>, [u8; 32]>>,
    }

    impl TestKeyResolver {
        fn new() -> Self {
            Self {
                keys: RwLock::new(HashMap::new()),
            }
        }

        async fn add_key(&self, sender_id: Vec<u8>, sign_pub: [u8; 32]) {
            let mut keys = self.keys.write().await;
            keys.insert(sender_id, sign_pub);
        }
    }

    #[async_trait]
    impl SenderKeyResolver for TestKeyResolver {
        async fn resolve_sign_pub(&self, sender_id: &[u8]) -> Result<Option<[u8; 32]>, String> {
            let keys = self.keys.read().await;
            Ok(keys.get(sender_id).copied())
        }
    }

    /// Simple echo handler for testing.
    struct EchoHandler;

    #[async_trait]
    impl MessageHandler for EchoHandler {
        async fn handle(
            &self,
            _sender_id: [u8; 32],
            payload: &[u8],
        ) -> Result<Option<Vec<u8>>, HandlerError> {
            Ok(Some(payload.to_vec()))
        }
    }

    /// Handler that always fails.
    struct FailingHandler;

    #[async_trait]
    impl MessageHandler for FailingHandler {
        async fn handle(
            &self,
            _sender_id: [u8; 32],
            _payload: &[u8],
        ) -> Result<Option<Vec<u8>>, HandlerError> {
            Err(HandlerError::ProcessingFailed("intentional failure".into()))
        }
    }

    #[tokio::test]
    async fn test_register_handler() {
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let key_resolver = Arc::new(TestKeyResolver::new());
        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);

        // Initially no handler
        assert!(!dispatcher.has_handler(MsgTypeV1::ControlMsg).await);

        // Register handler
        dispatcher
            .register_handler(MsgTypeV1::ControlMsg, Arc::new(EchoHandler))
            .await;

        // Now has handler
        assert!(dispatcher.has_handler(MsgTypeV1::ControlMsg).await);

        // Unregister
        dispatcher.unregister_handler(MsgTypeV1::ControlMsg).await;
        assert!(!dispatcher.has_handler(MsgTypeV1::ControlMsg).await);
    }

    #[tokio::test]
    async fn test_dispatch_success() {
        // Generate sender keys
        let sender_sign = SigningKey::generate(&mut OsRng);
        let sender_sign_pub = sender_sign.verifying_key().to_bytes();
        let sender_id = derive_id(&sender_sign_pub);

        // Generate recipient keys
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let recipient_kex_pub = X25519PublicKey::from(&recipient_kex_priv);
        let recipient_id = sha256(recipient_kex_pub.as_bytes());

        // Set up key resolver
        let key_resolver = Arc::new(TestKeyResolver::new());
        key_resolver.add_key(sender_id.to_vec(), sender_sign_pub).await;

        // Create dispatcher
        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);
        dispatcher
            .register_handler(MsgTypeV1::ControlMsg, Arc::new(EchoHandler))
            .await;

        // Create envelope
        let plaintext = b"test message";
        let now = 1700000000u64;
        let envelope = envelope_seal_v1(
            &sender_sign,
            &sender_id,
            &recipient_id,
            recipient_kex_pub.as_bytes(),
            MsgTypeV1::ControlMsg,
            plaintext,
            now,
        )
        .unwrap();

        // Dispatch
        let result = dispatcher.dispatch(envelope).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(plaintext.to_vec()));

        // Check stats
        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.dispatched, 1);
        assert_eq!(stats.dropped, 0);
    }

    #[tokio::test]
    async fn test_dispatch_unknown_sender() {
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let recipient_kex_pub = X25519PublicKey::from(&recipient_kex_priv);
        let recipient_id = sha256(recipient_kex_pub.as_bytes());

        // Generate sender keys (but don't add to resolver)
        let sender_sign = SigningKey::generate(&mut OsRng);
        let sender_sign_pub = sender_sign.verifying_key().to_bytes();
        let sender_id = derive_id(&sender_sign_pub);

        // Empty key resolver
        let key_resolver = Arc::new(TestKeyResolver::new());

        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);
        dispatcher
            .register_handler(MsgTypeV1::ControlMsg, Arc::new(EchoHandler))
            .await;

        // Create envelope
        let envelope = envelope_seal_v1(
            &sender_sign,
            &sender_id,
            &recipient_id,
            recipient_kex_pub.as_bytes(),
            MsgTypeV1::ControlMsg,
            b"test",
            1700000000,
        )
        .unwrap();

        // Dispatch should fail - unknown sender
        let result = dispatcher.dispatch(envelope).await;
        assert!(matches!(result, Err(DispatchError::SignatureInvalid)));

        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.signature_failures, 1);
    }

    #[tokio::test]
    async fn test_dispatch_no_handler() {
        // Generate sender keys
        let sender_sign = SigningKey::generate(&mut OsRng);
        let sender_sign_pub = sender_sign.verifying_key().to_bytes();
        let sender_id = derive_id(&sender_sign_pub);

        // Generate recipient keys
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let recipient_kex_pub = X25519PublicKey::from(&recipient_kex_priv);
        let recipient_id = sha256(recipient_kex_pub.as_bytes());

        // Set up key resolver
        let key_resolver = Arc::new(TestKeyResolver::new());
        key_resolver.add_key(sender_id.to_vec(), sender_sign_pub).await;

        // Create dispatcher WITHOUT registering handler
        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);

        // Create envelope
        let envelope = envelope_seal_v1(
            &sender_sign,
            &sender_id,
            &recipient_id,
            recipient_kex_pub.as_bytes(),
            MsgTypeV1::ControlMsg,
            b"test",
            1700000000,
        )
        .unwrap();

        // Dispatch should fail - no handler
        let result = dispatcher.dispatch(envelope).await;
        assert!(matches!(result, Err(DispatchError::NoHandler(_))));

        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.dropped, 1);
    }

    #[tokio::test]
    async fn test_dispatch_handler_error() {
        // Generate sender keys
        let sender_sign = SigningKey::generate(&mut OsRng);
        let sender_sign_pub = sender_sign.verifying_key().to_bytes();
        let sender_id = derive_id(&sender_sign_pub);

        // Generate recipient keys
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let recipient_kex_pub = X25519PublicKey::from(&recipient_kex_priv);
        let recipient_id = sha256(recipient_kex_pub.as_bytes());

        // Set up key resolver
        let key_resolver = Arc::new(TestKeyResolver::new());
        key_resolver.add_key(sender_id.to_vec(), sender_sign_pub).await;

        // Create dispatcher with failing handler
        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);
        dispatcher
            .register_handler(MsgTypeV1::ControlMsg, Arc::new(FailingHandler))
            .await;

        // Create envelope
        let envelope = envelope_seal_v1(
            &sender_sign,
            &sender_id,
            &recipient_id,
            recipient_kex_pub.as_bytes(),
            MsgTypeV1::ControlMsg,
            b"test",
            1700000000,
        )
        .unwrap();

        // Dispatch should fail - handler error
        let result = dispatcher.dispatch(envelope).await;
        assert!(matches!(result, Err(DispatchError::HandlerError(_))));

        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.handler_errors, 1);
        assert_eq!(stats.dropped, 1);
    }

    #[tokio::test]
    async fn test_stats_reset() {
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let key_resolver = Arc::new(TestKeyResolver::new());
        let dispatcher = Dispatcher::new(recipient_kex_priv, key_resolver);

        // Manually increment some stats
        dispatcher.stats.inc_received();
        dispatcher.stats.inc_received();
        dispatcher.stats.inc_dispatched();

        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 2);
        assert_eq!(stats.dispatched, 1);

        // Reset
        dispatcher.stats.reset();

        let stats = dispatcher.stats().snapshot();
        assert_eq!(stats.received, 0);
        assert_eq!(stats.dispatched, 0);
    }
}
