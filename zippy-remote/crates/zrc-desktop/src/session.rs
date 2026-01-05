use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use prost::Message; // For encode/decode

use zrc_core::types::IdentityKeys; // Correct import
use zrc_core::session::SessionController;
use zrc_core::store::{InMemoryStore, Store};
use zrc_core::transport::SelectedTransport; // Added
use zrc_proto::v1::{EnvelopeV1, MsgTypeV1, SessionInitResponseV1, ControlMsgV1, control_msg_v1};
use zrc_transport::{ControlPlaneTransport, MediaOpenParams, MediaSession, MediaTransport, RouteHint};

use crate::transport::{HttpControlTransport, QuicMediaTransport};

// Hardcoded for MVP
const RENDEZVOUS_URL: &str = "https://zrc.dev/api"; 
const REQUESTED_CAPS: u32 = 0x07;

pub struct SessionManager {
    identity_keys: IdentityKeys,
    store: Arc<InMemoryStore>,
    active_sessions: Arc<RwLock<HashMap<SessionId, Arc<ActiveSession>>>>,
    event_sender: Option<mpsc::Sender<SessionEvent>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    media_transport: QuicMediaTransport,
}

impl SessionManager {
    pub fn new(identity_keys: IdentityKeys, store: Arc<InMemoryStore>) -> Self {
        Self {
            identity_keys,
            store,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            event_sender: None,
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            media_transport: QuicMediaTransport,
        }
    }

    pub fn set_event_sender(&mut self, sender: mpsc::Sender<SessionEvent>) {
        self.event_sender = Some(sender);
    }
    
    pub fn get_active_session(&self, id: &SessionId) -> Option<Arc<ActiveSession>> {
        self.active_sessions.read().unwrap().get(id).cloned()
    }

    /// Initiate connection to device
    pub async fn connect(&self, device_id_hex: &str) -> Result<SessionId, SessionError> {
        let device_id = hex::decode(device_id_hex)
            .map_err(|_| SessionError::ConnectionFailed("Invalid hex device ID".into()))?;

        // ... (steps 1-7 same) ...
        // 1. Fetch Pairing Record
        let pairing = self.store.load_pairing(&device_id, &self.identity_keys.id32).await
            .map_err(|_| SessionError::ConnectionFailed("Device not paired".into()))?
            .ok_or(SessionError::ConnectionFailed("Device not paired (not found)".into()))?;

        let device_kex_pub_bytes: [u8; 32] = pairing.device_kex_pub.key_bytes.clone().try_into()
            .map_err(|_| SessionError::ConnectionFailed("Invalid device KEX key length".into()))?;
            
        let device_sign_pub_bytes: [u8; 32] = pairing.device_sign_pub.key_bytes.clone().try_into()
            .map_err(|_| SessionError::ConnectionFailed("Invalid device Sign key length".into()))?;

        // 2-3 same
        let mut controller = SessionController::new(self.identity_keys.clone(), self.store.clone());

        let request = controller
            .start_session(&device_id, REQUESTED_CAPS)
            .await
            .map_err(|e| SessionError::ConnectionFailed(e.to_string()))?;

        // 4 same
        let control_transport = HttpControlTransport::new(RENDEZVOUS_URL, self.identity_keys.id32)
            .map_err(|e| SessionError::ConnectionFailed(format!("Transport init failed: {}", e)))?;

        let mut req_bytes = Vec::new();
        request.encode(&mut req_bytes)
            .map_err(|e| SessionError::ConnectionFailed(format!("Encode error: {}", e)))?;
            
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let envelope = zrc_crypto::envelope::envelope_seal_v1(
            &self.identity_keys.sign,
            &self.identity_keys.id32,
            &device_id,
            &device_kex_pub_bytes,
            MsgTypeV1::SessionInitRequest,
            &req_bytes,
            now
        ).map_err(|e| SessionError::ConnectionFailed(format!("Encryption failed: {}", e)))?;
        
        let mut env_bytes_proto = Vec::new();
        envelope.encode(&mut env_bytes_proto)
             .map_err(|e| SessionError::ConnectionFailed(format!("Envelope encode error: {}", e)))?;
        
        let mut device_id_arr = [0u8; 32];
        device_id_arr.copy_from_slice(&device_id);
        
        control_transport.send(&device_id_arr, &env_bytes_proto)
            .await
            .map_err(|e| SessionError::ConnectionFailed(format!("Failed to send request: {}", e)))?;

        // 5 same
        let response = loop {
             let (_sender_id, incoming_env_bytes) = control_transport.recv()
                 .await
                 .map_err(|e| SessionError::ConnectionFailed(format!("Failed to receive response: {}", e)))?;
             
             let incoming_env = EnvelopeV1::decode(&incoming_env_bytes[..])
                 .map_err(|_| SessionError::ConnectionFailed("Bad envelope format".into()))?;

             let (plaintext, sender_id) = zrc_crypto::envelope::envelope_open_v1(
                 &incoming_env,
                 &self.identity_keys.kex_priv,
                 &device_sign_pub_bytes
             ).map_err(|e| SessionError::ConnectionFailed(format!("Decryption failed: {}", e)))?;

             if sender_id != device_id { continue; }
             
             let header = incoming_env.header.as_ref().unwrap();
             if header.msg_type != (MsgTypeV1::SessionInitResponse as i32) { continue; }

             let resp = SessionInitResponseV1::decode(plaintext)
                  .map_err(|_| SessionError::ConnectionFailed("Bad response proto".into()))?;
             break resp;
        };

        // 6 same
        controller.handle_response(response.clone(), &device_sign_pub_bytes).await
             .map_err(|e| SessionError::ConnectionFailed(format!("Invalid response: {}", e)))?;

        // 7 same
        let selected_transport = controller.initiate_connection()
             .map_err(|e| SessionError::ConnectionFailed(format!("Negotiation failed: {}", e)))?;

        // 8 Connect Media Transport
        let (quic_params, _relay_token_opt) = match selected_transport {
            SelectedTransport::Quic { params } => (params, None),
            SelectedTransport::Relay { token, params } => (params, Some(token)),
            _ => return Err(SessionError::ConnectionFailed("Unsupported transport selected".into())),
        };

        let addr_str = quic_params.server_addr.clone()
            .ok_or(SessionError::ConnectionFailed("No server address provided".into()))?;

        let (host, port) = if let Some((h, p)) = addr_str.rsplit_once(':') {
            (h.to_string(), p.parse::<u16>().map_err(|_| SessionError::ConnectionFailed("Invalid port".into()))?)
        } else {
             return Err(SessionError::ConnectionFailed("Invalid address format".into()));
        };

        let media_params = MediaOpenParams {
            peer_id: device_id_arr,
            route: RouteHint::DirectIp { host, port },
            alpn: quic_params.alpn_protocols.first().cloned(),
            relay_token: Some(bytes::Bytes::from(quic_params.certificate)), 
        };

        let media_session_box = self.media_transport.open(media_params).await
            .map_err(|e| SessionError::ConnectionFailed(e.to_string()))?;
            
        // Convert to Arc for sharing tasks
        let media_session: Arc<dyn MediaSession> = Arc::from(media_session_box);

        // 8b Setup Control Loops & File Transfer
        let file_transfer = Arc::new(crate::transfer::FileTransferManager::new());
        let (control_tx, mut control_rx) = mpsc::channel::<ControlMsgV1>(100);

        // Setup Clipboard
        let clipboard_manager = Arc::new(
            crate::clipboard::ClipboardManager::new(control_tx.clone())
                .map_err(|e| SessionError::ConnectionFailed(format!("Clipboard init failed: {}", e)))?
        );
        clipboard_manager.start_monitoring();

        // Spawn Control Sender
        let ms_tx = media_session.clone();
        tokio::spawn(async move {
            while let Some(msg) = control_rx.recv().await {
                 let payload = msg.encode_to_vec();
                 let _ = ms_tx.send_control(bytes::Bytes::from(payload)).await;
            }
        });
        
        // Spawn Control Receiver
        let ms_rx = media_session.clone();
        let ft_rx = file_transfer.clone();
        let clip_rx = clipboard_manager.clone();
        tokio::spawn(async move {
            loop {
                // TODO: Handle disconnect/errors properly (propagate to SessionManager?)
                match ms_rx.recv_control().await {
                    Ok(bytes) => {
                         if let Ok(msg) = ControlMsgV1::decode(bytes) {
                             if let Some(payload) = msg.payload {
                                 match payload {
                                      control_msg_v1::Payload::FileControl(fc) => {
                                          ft_rx.handle_message(fc).await;
                                      },
                                      control_msg_v1::Payload::Clipboard(cb) => {
                                          clip_rx.apply_remote_update(cb);
                                      },
                                      _ => {}
                                 }
                             }
                         }
                    }
                    Err(_) => break, 
                }
            }
        });

        // 9 same
        controller.mark_connected()
             .map_err(|e| SessionError::ConnectionFailed(e.to_string()))?;

        // Success! Create ActiveSession
        let ui_id = SessionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        
        let session = Arc::new(ActiveSession {
            id: ui_id,
            device_id: device_id_hex.to_string(),
            core_id: controller.active_session().unwrap().session_id,
            controller: Arc::new(Mutex::new(controller)),
            media_session,
            file_transfer,
            control_tx,
            clipboard_manager,
            capabilities: Capabilities::default(),
            started_at: Instant::now(),
            stats: RwLock::new(SessionStats::default()),
            diagnostics: crate::diagnostics::ConnectionDiagnostics::new(),
        });

        {
            let mut sessions = self.active_sessions.write().unwrap();
            sessions.insert(ui_id, session.clone());
        }

        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SessionEvent::Connected { session_id: ui_id }).await;
        }

        Ok(ui_id)
    }

    /// List all active sessions
    pub fn list_active_sessions(&self) -> Vec<SessionId> {
        self.active_sessions.read().unwrap().keys().copied().collect()
    }
    
    /// Get store reference (for device manager reload)
    pub fn get_store(&self) -> Arc<InMemoryStore> {
        self.store.clone()
    }
    
    /// Get operator ID (for device manager reload)
    pub fn get_operator_id(&self) -> [u8; 32] {
        self.identity_keys.id32
    }
    
    /// Import a pairing invite
    /// This will decode the invite and initiate the pairing process
    pub async fn import_pairing_invite(&self, invite_bytes: &[u8]) -> Result<String, SessionError> {
        use zrc_core::pairing::PairingController;
        use zrc_proto::v1::InviteV1;
        use prost::Message;
        
        // Decode invite
        let invite = InviteV1::decode(invite_bytes)
            .map_err(|e| SessionError::ConnectionFailed(format!("Failed to decode invite: {}", e)))?;
        
        // Create pairing controller
        let mut controller = PairingController::new(self.identity_keys.clone(), self.store.clone());
        
        // Import invite
        controller.import_invite(invite_bytes)
            .map_err(|e| SessionError::ConnectionFailed(format!("Failed to import invite: {}", e)))?;
        
        // Return device ID as hex
        Ok(hex::encode(&invite.device_id))
    }

    /// Disconnect a session
    pub async fn disconnect(&self, session_id: SessionId) -> Result<(), SessionError> {
        let session = self.active_sessions.write().unwrap().remove(&session_id)
            .ok_or(SessionError::NotFound(session_id))?;
        
        // Session will be dropped and cleanup will happen automatically
        // Send disconnect event
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SessionEvent::Disconnected {
                session_id,
                reason: "User disconnected".to_string(),
            }).await;
        }
        
        Ok(())
    }

    /// Disconnect all sessions
    pub async fn disconnect_all(&self) {
        let ids = self.list_active_sessions();
        for id in ids {
            let _ = self.disconnect(id).await;
        }
    }

}

/// Session identifier (UI Handle)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Active session
pub struct ActiveSession {
    pub id: SessionId,
    pub core_id: [u8; 32],
    pub device_id: String,
    pub capabilities: Capabilities,
    pub started_at: Instant,
    
    // Core state
    pub controller: Arc<Mutex<SessionController<InMemoryStore>>>,
    pub media_session: Arc<dyn MediaSession>, // Box -> Arc
    pub file_transfer: Arc<crate::transfer::FileTransferManager>,
    pub control_tx: mpsc::Sender<ControlMsgV1>,
    pub clipboard_manager: Arc<crate::clipboard::ClipboardManager>,
    
    pub stats: RwLock<SessionStats>,
    pub diagnostics: crate::diagnostics::ConnectionDiagnostics,
}

/// Session capabilities
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub view: bool,
    pub control: bool,
    pub clipboard: bool,
    pub file_transfer: bool,
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub frames_received: Arc<AtomicU64>,
    pub bytes_received: Arc<AtomicU64>,
    pub current_fps: Arc<AtomicU32>,
    pub latency_ms: Arc<AtomicU32>,
    pub packet_loss: Arc<AtomicU32>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self {
            frames_received: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            current_fps: Arc::new(AtomicU32::new(0)),
            latency_ms: Arc::new(AtomicU32::new(0)),
            packet_loss: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        }
    }
}

impl SessionStats {
    // Return a snapshot with new atomics (detached from live stats)
    // Used for UI Transfer objects
    pub fn clone(&self) -> Self {
        Self {
            frames_received: Arc::new(AtomicU64::new(self.frames_received.load(Ordering::Relaxed))),
            bytes_received: Arc::new(AtomicU64::new(self.bytes_received.load(Ordering::Relaxed))),
            current_fps: Arc::new(AtomicU32::new(self.current_fps.load(Ordering::Relaxed))),
            latency_ms: Arc::new(AtomicU32::new(self.latency_ms.load(Ordering::Relaxed))),
            packet_loss: Arc::new(AtomicU32::new(self.packet_loss.load(Ordering::Relaxed))),
        }
    }
}

/// Session information for UI
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: SessionId,
    pub device_id: String,
    pub started_at: Instant,
    pub stats: SessionStats,
}

/// Session events
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Connected { session_id: SessionId },
    Disconnected { session_id: SessionId, reason: String },
    QualityChanged { session_id: SessionId, quality: ConnectionQuality },
    Error { session_id: SessionId, error: String },
}

/// Connection quality
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
}

/// Session errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found: {0:?}")]
    NotFound(SessionId),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Session error: {0}")]
    Other(String),
}
