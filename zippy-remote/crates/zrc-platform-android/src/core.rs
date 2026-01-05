//! Core ZRC functionality for Android

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use zeroize::Zeroize;

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
            .map_err(|e| ZrcError::Config(format!("Invalid config JSON: {}", e)))?;
        
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
    // Frame buffer for polling (last received frame)
    frame_buffer: Arc<RwLock<Option<FrameData>>>,
    // Connection status
    connection_status: Arc<RwLock<String>>,
}

/// Internal core state
struct CoreInner {
    config: CoreConfig,
    identity_keys: IdentityKeys,
    store: Arc<InMemoryStore>,
    session_controller: Option<SessionController<InMemoryStore>>,
    active_sessions: Arc<RwLock<HashMap<u64, ActiveSession>>>,
    next_session_id: Arc<Mutex<u64>>,
    // Runtime handle for async operations
    runtime: Option<tokio::runtime::Handle>,
}

impl CoreInner {
    fn new(config_json: String) -> Result<Self, ZrcError> {
        let config = CoreConfig::from_json(&config_json)?;
        
        // Generate or load identity keys
        // For now, generate new keys each time (in production, should load from Android Keystore)
        let identity_keys = generate_identity_keys()
            .map_err(|e| ZrcError::Core(format!("Failed to generate identity keys: {}", e)))?;
        
        let store = Arc::new(InMemoryStore::new());
        
        Ok(Self {
            config,
            identity_keys,
            store,
            session_controller: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(1)),
            runtime: None,
        })
    }

    fn set_runtime(&mut self, handle: tokio::runtime::Handle) {
        self.runtime = Some(handle);
    }

    async fn start_session(&mut self, device_id: Vec<u8>) -> Result<u64, ZrcError> {
        // Validate device_id length
        if device_id.len() != 32 {
            return Err(ZrcError::InvalidParameter("device_id must be 32 bytes".to_string()));
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
            .map_err(|e| ZrcError::Session(format!("Session start failed: {}", e)))?;
        
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
            connection_status: Arc::new(RwLock::new("connecting".to_string())),
        };
        
        self.active_sessions.write().await.insert(session_id, session);
        
        // TODO: Complete connection flow:
        // 1. Send session request via transport
        // 2. Wait for response
        // 3. Establish QUIC connection
        // 4. Start frame receiver loop
        // For now, mark as connected after a delay (placeholder)
        let sessions = self.active_sessions.clone();
        let session_id_clone = session_id;
        if let Some(rt) = &self.runtime {
            rt.spawn(async move {
                // Simulate connection establishment
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if let Some(session) = sessions.read().await.get(&session_id_clone) {
                    *session.connection_status.write().await = "connected".to_string();
                }
            });
        }
        
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
            // Get frame from buffer and clear it (consume-once semantics)
            let mut frame_buffer = session.frame_buffer.write().await;
            frame_buffer.take()
        } else {
            None
        }
    }

    async fn send_input(&self, session_id: u64, event: InputEvent) -> Result<(), ZrcError> {
        // Verify session exists
        let sessions = self.active_sessions.read().await;
        if !sessions.contains_key(&session_id) {
            return Err(ZrcError::Session(format!("Session {} not found", session_id)));
        }
        drop(sessions);
        
        // Convert InputEvent to zrc-proto InputEventV1
        use zrc_proto::v1::{InputEventV1, InputEventTypeV1};
        
        let proto_event = InputEventV1 {
            event_type: match event.event_type {
                crate::input::InputEventType::MouseMove => InputEventTypeV1::MouseMove as i32,
                crate::input::InputEventType::MouseDown => InputEventTypeV1::MouseDown as i32,
                crate::input::InputEventType::MouseUp => InputEventTypeV1::MouseUp as i32,
                crate::input::InputEventType::Scroll => InputEventTypeV1::Scroll as i32,
                crate::input::InputEventType::KeyDown => InputEventTypeV1::KeyDown as i32,
                crate::input::InputEventType::KeyUp => InputEventTypeV1::KeyUp as i32,
                crate::input::InputEventType::KeyChar => InputEventTypeV1::KeyChar as i32,
            },
            mouse_x: event.mouse_x,
            mouse_y: event.mouse_y,
            button: event.button,
            key_code: event.key_code,
            modifiers: event.modifiers,
            text: event.text,
            scroll_delta_x: event.scroll_delta_x,
            scroll_delta_y: event.scroll_delta_y,
        };
        
        // TODO: Send input via session transport
        // This requires QUIC transport integration which is complex
        // The structure is in place for proper integration
        
        Ok(())
    }
    
    async fn get_connection_status(&self, session_id: u64) -> String {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            session.connection_status.read().await.clone()
        } else {
            "disconnected".to_string()
        }
    }
    
    /// Update frame buffer for a session (called from transport layer)
    pub async fn update_frame(&self, session_id: u64, frame: FrameData) {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            *session.frame_buffer.write().await = Some(frame);
        }
    }
}

/// Core ZRC instance for Android
pub struct ZrcCore {
    inner: Arc<Mutex<CoreInner>>,
}

impl ZrcCore {
    /// Create a new ZRC core instance
    pub fn new(config_json: &str) -> Result<Self, ZrcError> {
        let inner = CoreInner::new(config_json.to_string())?;
        
        // Try to get current runtime handle
        let handle = tokio::runtime::Handle::try_current();
        let mut inner_mutex = Mutex::new(inner);
        if let Ok(handle) = handle {
            // We can't set runtime here because we need &mut, but we can spawn a task
            // For now, store it in the inner struct
            // Actually, we need to set it after creation
        }
        
        Ok(Self {
            inner: Arc::new(inner_mutex),
        })
    }
    
    /// Set the runtime handle for async operations
    pub fn set_runtime(&self, handle: tokio::runtime::Handle) {
        // This is tricky with Arc<Mutex<>> - we'd need to restructure
        // For now, async operations will use Handle::current() or spawn on the runtime
    }

    /// Get a reference to the inner state
    pub fn inner(&self) -> Arc<Mutex<CoreInner>> {
        self.inner.clone()
    }
    
    /// Start a session (async)
    pub async fn start_session(&self, device_id: Vec<u8>) -> Result<u64, ZrcError> {
        let mut inner = self.inner.lock().await;
        inner.start_session(device_id).await
    }
    
    /// End a session (async)
    pub async fn end_session(&self, session_id: u64) -> Result<(), ZrcError> {
        let mut inner = self.inner.lock().await;
        inner.end_session(session_id).await
    }
    
    /// Poll for a frame (async)
    pub async fn poll_frame(&self, session_id: u64) -> Option<FrameData> {
        let inner = self.inner.lock().await;
        inner.poll_frame(session_id).await
    }
    
    /// Send input event (async)
    pub async fn send_input(&self, session_id: u64, event: InputEvent) -> Result<(), ZrcError> {
        let inner = self.inner.lock().await;
        inner.send_input(session_id, event).await
    }
    
    /// Get connection status (async)
    pub async fn get_connection_status(&self, session_id: u64) -> String {
        let inner = self.inner.lock().await;
        inner.get_connection_status(session_id).await
    }
}

impl Drop for ZrcCore {
    fn drop(&mut self) {
        // Zeroize sensitive data if needed
    }
}
