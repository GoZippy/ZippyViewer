//! Core ZRC functionality exposed to Swift via UniFFI

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use uniffi::Object;

use crate::error::ZrcError;
use crate::frame::FrameData;
use crate::input::InputEvent;

use zrc_core::keys::generate_identity_keys;
use zrc_core::session::SessionController;
use zrc_core::store::InMemoryStore;
use zrc_core::types::IdentityKeys;

/// Configuration for ZRC core
#[derive(Debug, Clone)]
struct CoreConfig {
    rendezvous_urls: Vec<String>,
    relay_urls: Vec<String>,
    transport_preference: String,
}

impl CoreConfig {
    fn from_json(json: &str) -> Result<Self, ZrcError> {
        let parsed: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| ZrcError::General(format!("Invalid config JSON: {}", e)))?;
        
        Ok(Self {
            rendezvous_urls: parsed
                .get("rendezvous_urls")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            relay_urls: parsed
                .get("relay_urls")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            transport_preference: parsed
                .get("transport_preference")
                .and_then(|v| v.as_str())
                .unwrap_or("auto")
                .to_string(),
        })
    }
}

/// Active session state
struct ActiveSession {
    session_id: u64,
    device_id: Vec<u8>,
    // Frame buffer for polling
    frame_buffer: Arc<RwLock<Option<FrameData>>>,
}

/// Internal core state
struct CoreInner {
    config: CoreConfig,
    identity_keys: IdentityKeys,
    store: Arc<InMemoryStore>,
    session_controller: Option<SessionController<InMemoryStore>>,
    active_sessions: Arc<RwLock<HashMap<u64, ActiveSession>>>,
    next_session_id: Arc<Mutex<u64>>,
}

impl CoreInner {
    fn new(config_json: String) -> Result<Self, ZrcError> {
        let config = CoreConfig::from_json(&config_json)?;
        
        // Generate or load identity keys
        // For now, generate new keys each time (in production, should load from Keychain)
        let identity_keys = generate_identity_keys()
            .map_err(|e| ZrcError::Crypto(format!("Failed to generate identity keys: {}", e)))?;
        
        let store = Arc::new(InMemoryStore::new());
        
        Ok(Self {
            config,
            identity_keys,
            store,
            session_controller: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(1)),
        })
    }

    async fn start_session(&mut self, device_id: Vec<u8>) -> Result<u64, ZrcError> {
        // Validate device_id length
        if device_id.len() != 32 {
            return Err(ZrcError::InvalidInput("device_id must be 32 bytes".to_string()));
        }
        
        // Create session controller if needed
        if self.session_controller.is_none() {
            self.session_controller = Some(SessionController::new(
                self.identity_keys.clone(),
                self.store.clone(),
            ));
        }
        
        let controller = self.session_controller.as_mut().unwrap();
        
        // Requested capabilities: view, control, clipboard (0x07)
        let requested_caps = 0x07;
        
        // Start session request
        let _request = controller
            .start_session(&device_id, requested_caps)
            .await
            .map_err(|e| ZrcError::ConnectionFailed(format!("Session start failed: {}", e)))?;
        
        // Generate session ID for tracking
        let mut next_id = self.next_session_id.lock().await;
        let session_id = *next_id;
        *next_id += 1;
        drop(next_id);
        
        // Create active session
        let session = ActiveSession {
            session_id,
            device_id: device_id.clone(),
            frame_buffer: Arc::new(RwLock::new(None)),
        };
        
        self.active_sessions.write().await.insert(session_id, session);
        
        Ok(session_id)
    }

    async fn end_session(&mut self, session_id: u64) -> Result<(), ZrcError> {
        // Remove session from active sessions
        self.active_sessions.write().await.remove(&session_id);
        
        // Reset session controller if no active sessions
        if self.active_sessions.read().await.is_empty() {
            if let Some(controller) = &mut self.session_controller {
                controller.reset();
            }
        }
        
        Ok(())
    }

    async fn poll_frame(&self, session_id: u64) -> Option<FrameData> {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            // Get frame from buffer
            let frame_buffer = session.frame_buffer.read().await;
            frame_buffer.clone()
        } else {
            None
        }
    }

    async fn send_input(&self, session_id: u64, event: InputEvent) -> Result<(), ZrcError> {
        // Verify session exists
        let sessions = self.active_sessions.read().await;
        if !sessions.contains_key(&session_id) {
            return Err(ZrcError::SessionNotFound);
        }
        drop(sessions);
        
        // Convert InputEvent to zrc-core InputEvent
        let core_event = event.to_core_event();
        
        // TODO: Send input via session transport
        // For now, this is a placeholder - actual implementation requires
        // QUIC transport integration which is complex and platform-specific
        // The structure is in place for proper integration
        
        Ok(())
    }
    
    /// Update frame buffer for a session (called from transport layer)
    pub async fn update_frame(&self, session_id: u64, frame: FrameData) {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            *session.frame_buffer.write().await = Some(frame);
        }
    }
}

/// ZRC Core interface exposed to Swift
#[derive(uniffi::Object)]
pub struct ZrcCore {
    inner: Arc<Mutex<CoreInner>>,
}

#[uniffi::export]
impl ZrcCore {
    /// Create new ZRC core instance
    #[uniffi::constructor]
    pub fn new(config_json: String) -> Result<Arc<Self>, ZrcError> {
        let inner = CoreInner::new(config_json)?;
        Ok(Arc::new(Self {
            inner: Arc::new(Mutex::new(inner)),
        }))
    }

    /// Start a session with a remote device
    pub async fn start_session(&self, device_id: Vec<u8>) -> Result<u64, ZrcError> {
        let mut inner = self.inner.lock().await;
        inner.start_session(device_id).await
    }

    /// End a session
    pub async fn end_session(&self, session_id: u64) -> Result<(), ZrcError> {
        let mut inner = self.inner.lock().await;
        inner.end_session(session_id).await
    }

    /// Poll for a new frame from the session
    pub async fn poll_frame(&self, session_id: u64) -> Option<FrameData> {
        let inner = self.inner.lock().await;
        inner.poll_frame(session_id).await
    }

    /// Send an input event to the remote device
    pub async fn send_input(&self, session_id: u64, event: InputEvent) -> Result<(), ZrcError> {
        let inner = self.inner.lock().await;
        inner.send_input(session_id, event).await
    }
}
