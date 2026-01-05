# Design Document: zrc-agent

## Overview

The zrc-agent crate implements the host daemon/service for the Zippy Remote Control (ZRC) system. It runs on machines being remotely controlled, handling pairing requests, session management, screen capture, input injection, and connectivity. The agent enforces local security policies and provides consent mechanisms for attended access.

**Architecture Decision: WebRTC-first Hybrid**

The agent uses a WebRTC-first hybrid architecture:
- **Control Plane (Rust)**: Identity keys, pairing, invite-only discovery, session authorization tickets, directory records, audit events
- **Media Plane (WebRTC)**: Video/audio streams via libwebrtc (C++ engine via FFI), DataChannels for control/clipboard/files
- **Fallback Relay**: coturn for TURN relay (self-hostable)

This approach leverages WebRTC's battle-tested NAT traversal (ICE), congestion control, and codec negotiation while keeping the security model in Rust.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              zrc-agent                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Service Layer                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Windows    │  │   Linux     │  │   macOS     │                  │   │
│  │  │  Service    │  │  systemd    │  │  launchd    │                  │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │   │
│  └─────────┼────────────────┼────────────────┼──────────────────────────┘   │
│            └────────────────┼────────────────┘                              │
│                             ▼                                                │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Agent Core (Rust)                                │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Identity   │  │   Pairing   │  │   Session   │                  │   │
│  │  │  Manager    │  │   Manager   │  │   Manager   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Consent   │  │   Policy    │  │   Config    │                  │   │
│  │  │   Handler   │  │   Engine    │  │   Manager   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                             │                                                │
│            ┌────────────────┼────────────────┐                              │
│            ▼                ▼                ▼                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────────┐    │
│  │  Capture    │  │   Input     │  │     Media Transport Layer       │    │
│  │  Engine     │  │  Injector   │  │  ┌───────────┐ ┌─────────────┐ │    │
│  └─────────────┘  └─────────────┘  │  │  WebRTC   │ │  Signaling  │ │    │
│         │                │          │  │ (libwebrtc│ │  (Rust)     │ │    │
│         │                │          │  │   FFI)    │ │             │ │    │
│         │                │          │  └───────────┘ └─────────────┘ │    │
│         │                │          └─────────────────────────────────┘    │
│         ▼                ▼                │                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Platform Abstraction                               │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │ zrc-platform│  │ zrc-platform│  │ zrc-platform│                  │   │
│  │  │    -win     │  │   -linux    │  │    -mac     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         External Services                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│  │  Rendezvous │  │   Dirnode   │  │   coturn    │  │    Mesh     │       │
│  │  (mailbox)  │  │ (directory) │  │   (TURN)    │  │  (optional) │       │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Security: Identity-Bound DTLS (Prevents Signaling MITM)

**Critical Security Requirement:** The pairing layer MUST bind the WebRTC DTLS fingerprint to the device identity.

```rust
/// Identity-bound certificate for WebRTC DTLS
pub struct IdentityBoundCert {
    /// The DTLS certificate
    pub cert: DtlsCertificate,
    /// SHA-256 fingerprint of the certificate
    pub fingerprint: [u8; 32],
    /// Signature of fingerprint by device's Ed25519 identity key
    pub fingerprint_signature: Signature,
    /// Device's signing public key (for verification)
    pub device_sign_pub: [u8; 32],
}

impl IdentityManager {
    /// Generate DTLS cert and sign fingerprint with identity key
    pub fn generate_dtls_cert(&self) -> Result<IdentityBoundCert, IdentityError> {
        let cert = DtlsCertificate::generate()?;
        let fingerprint = cert.fingerprint_sha256();
        let fingerprint_signature = self.sign(&fingerprint);
        
        Ok(IdentityBoundCert {
            cert,
            fingerprint,
            fingerprint_signature,
            device_sign_pub: self.signing_key.public_key(),
        })
    }
    
    /// Verify peer's DTLS cert is bound to their pinned identity
    pub fn verify_peer_cert_binding(
        &self,
        peer_cert_fingerprint: &[u8; 32],
        peer_fingerprint_sig: &Signature,
        pinned_peer_sign_pub: &[u8; 32],
    ) -> Result<(), IdentityError> {
        // Verify signature matches pinned identity from PairReceipt
        verify_signature(pinned_peer_sign_pub, peer_cert_fingerprint, peer_fingerprint_sig)?;
        Ok(())
    }
}
```

**Trust Chain:**
1. Device generates DTLS cert → signs fingerprint with Ed25519 identity key
2. Operator receives fingerprint + signature in session negotiation
3. Operator verifies signature against pinned identity from PairReceipt
4. If fingerprint changes → alert user, require explicit re-approval
5. Directory/rendezvous CANNOT substitute a different cert (MITM blocked)

## Components and Interfaces

### Service Layer

```rust
/// Platform-agnostic service lifecycle trait
pub trait ServiceHost {
    /// Initialize and start the agent service
    async fn start(&mut self, config: AgentConfig) -> Result<(), ServiceError>;
    
    /// Gracefully stop the service
    async fn stop(&mut self) -> Result<(), ServiceError>;
    
    /// Handle service control signals
    async fn handle_signal(&mut self, signal: ServiceSignal) -> Result<(), ServiceError>;
    
    /// Get current service status
    fn status(&self) -> ServiceStatus;
}

pub enum ServiceSignal {
    Stop,
    Reload,      // Reload configuration
    Pause,       // Pause accepting new sessions
    Resume,      // Resume accepting sessions
}

pub enum ServiceStatus {
    Starting,
    Running { sessions: u32, uptime: Duration },
    Stopping,
    Stopped,
    Error(String),
}


### Identity Manager

```rust
/// Manages device cryptographic identity
pub struct IdentityManager {
    device_id: DeviceId,
    signing_key: SigningKeyPair,
    kex_key: KexKeyPair,
    key_store: Box<dyn KeyStore>,
}

impl IdentityManager {
    /// Initialize or load existing identity
    pub async fn init(key_store: Box<dyn KeyStore>) -> Result<Self, IdentityError>;
    
    /// Get device ID (derived from signing public key)
    pub fn device_id(&self) -> &DeviceId;
    
    /// Get public key bundle for sharing
    pub fn public_bundle(&self) -> PublicKeyBundleV1;
    
    /// Sign data with device signing key
    pub fn sign(&self, data: &[u8]) -> Signature;
    
    /// Perform key exchange with peer
    pub fn key_exchange(&self, peer_kex_pub: &[u8; 32]) -> SharedSecret;
    
    /// Rotate keys (generates new keypairs, retains old for verification)
    pub async fn rotate_keys(&mut self) -> Result<(), IdentityError>;
}

/// Platform-specific secure key storage
pub trait KeyStore: Send + Sync {
    fn store_key(&self, name: &str, key: &[u8]) -> Result<(), KeyStoreError>;
    fn load_key(&self, name: &str) -> Result<Vec<u8>, KeyStoreError>;
    fn delete_key(&self, name: &str) -> Result<(), KeyStoreError>;
    fn key_exists(&self, name: &str) -> bool;
}
```

### Pairing Manager

```rust
/// Manages device pairings with operators
pub struct PairingManager {
    identity: Arc<IdentityManager>,
    pairings_db: PairingsDatabase,
    rate_limiter: RateLimiter,
    consent_handler: Arc<dyn ConsentHandler>,
}

impl PairingManager {
    /// Generate a new invite for sharing
    pub fn generate_invite(&self, config: InviteConfig) -> Result<InviteV1, PairingError>;
    
    /// Process incoming pair request
    pub async fn handle_pair_request(
        &self,
        request: PairRequestV1,
        source: TransportSource,
    ) -> Result<PairReceiptV1, PairingError>;
    
    /// List all active pairings
    pub fn list_pairings(&self) -> Vec<PairingInfo>;
    
    /// Revoke a pairing
    pub fn revoke_pairing(&self, operator_id: &OperatorId) -> Result<(), PairingError>;
    
    /// Check if operator is paired
    pub fn is_paired(&self, operator_id: &OperatorId) -> bool;
    
    /// Get permissions for paired operator
    pub fn get_permissions(&self, operator_id: &OperatorId) -> Option<Permissions>;
}

pub struct InviteConfig {
    pub expires_in: Duration,
    pub max_uses: u32,
    pub permissions: Permissions,
    pub transport_hints: EndpointHintsV1,
}

pub struct PairingInfo {
    pub operator_id: OperatorId,
    pub operator_name: Option<String>,
    pub permissions: Permissions,
    pub paired_at: SystemTime,
    pub last_session: Option<SystemTime>,
}
```

### Session Manager

```rust
/// Manages active remote control sessions
pub struct SessionManager {
    identity: Arc<IdentityManager>,
    pairing_manager: Arc<PairingManager>,
    policy_engine: Arc<PolicyEngine>,
    consent_handler: Arc<dyn ConsentHandler>,
    active_sessions: RwLock<HashMap<SessionId, Session>>,
    max_sessions: usize,
}

impl SessionManager {
    /// Handle incoming session init request
    pub async fn handle_session_request(
        &self,
        request: SessionInitRequestV1,
    ) -> Result<SessionInitResponseV1, SessionError>;
    
    /// Get active session by ID
    pub fn get_session(&self, session_id: &SessionId) -> Option<Arc<Session>>;
    
    /// Terminate a session
    pub async fn terminate_session(&self, session_id: &SessionId) -> Result<(), SessionError>;
    
    /// Terminate all sessions (panic button)
    pub async fn terminate_all(&self);
    
    /// List active sessions
    pub fn list_sessions(&self) -> Vec<SessionInfo>;
}

pub struct Session {
    pub id: SessionId,
    pub operator_id: OperatorId,
    pub ticket: SessionTicketV1,
    pub capabilities: Capabilities,
    pub started_at: Instant,
    pub last_activity: AtomicU64,
    pub streams: SessionStreams,
}

pub struct SessionStreams {
    pub control: ControlStream,
    pub frames: FrameStream,
    pub clipboard: Option<ClipboardStream>,
    pub files: Option<FileTransferStream>,
}
```

### Consent Handler

```rust
/// Handles user consent for pairing and sessions
#[async_trait]
pub trait ConsentHandler: Send + Sync {
    /// Request consent for pairing
    async fn request_pairing_consent(
        &self,
        request: &PairRequestV1,
        operator_info: &OperatorInfo,
    ) -> ConsentResult;
    
    /// Request consent for session
    async fn request_session_consent(
        &self,
        request: &SessionInitRequestV1,
        operator_info: &OperatorInfo,
    ) -> ConsentResult;
    
    /// Show active session indicator
    fn show_session_indicator(&self, session: &SessionInfo);
    
    /// Hide session indicator
    fn hide_session_indicator(&self, session_id: &SessionId);
    
    /// Check if user is present (for unattended detection)
    fn is_user_present(&self) -> bool;
}

pub enum ConsentResult {
    Approved { permissions: Permissions },
    Denied { reason: String },
    Timeout,
    UserNotPresent,
}

/// GUI consent handler (system tray + dialogs)
pub struct GuiConsentHandler {
    tray_icon: TrayIcon,
    dialog_sender: mpsc::Sender<ConsentDialog>,
}

/// Headless consent handler (for unattended mode)
pub struct HeadlessConsentHandler {
    policy: UnattendedPolicy,
}
```

### Policy Engine

```rust
/// Evaluates access policies
pub struct PolicyEngine {
    consent_mode: ConsentMode,
    time_restrictions: Option<TimeRestrictions>,
    operator_policies: HashMap<OperatorId, OperatorPolicy>,
    default_policy: DefaultPolicy,
}

impl PolicyEngine {
    /// Evaluate if session should be allowed
    pub fn evaluate_session(
        &self,
        operator_id: &OperatorId,
        requested_caps: Capabilities,
    ) -> PolicyDecision;
    
    /// Check if consent is required
    pub fn requires_consent(&self, operator_id: &OperatorId) -> bool;
    
    /// Get effective permissions for operator
    pub fn effective_permissions(&self, operator_id: &OperatorId) -> Permissions;
}

pub enum ConsentMode {
    AlwaysRequire,
    UnattendedAllowed,
    TrustedOnly { trusted_operators: HashSet<OperatorId> },
}

pub enum PolicyDecision {
    Allow { permissions: Permissions },
    RequireConsent { max_permissions: Permissions },
    Deny { reason: String },
}

pub struct TimeRestrictions {
    pub allowed_days: HashSet<Weekday>,
    pub allowed_hours: Range<u8>,  // 0-23
    pub timezone: Tz,
}
```

### Capture Engine

```rust
/// Manages screen capture
pub struct CaptureEngine {
    platform_capturer: Box<dyn PlatformCapturer>,
    config: CaptureConfig,
    monitors: Vec<MonitorInfo>,
    active_capture: Option<ActiveCapture>,
}

impl CaptureEngine {
    /// Start capturing specified monitor
    pub async fn start_capture(&mut self, monitor: MonitorId) -> Result<FrameReceiver, CaptureError>;
    
    /// Stop capture
    pub async fn stop_capture(&mut self);
    
    /// List available monitors
    pub fn list_monitors(&self) -> &[MonitorInfo];
    
    /// Handle monitor configuration change
    pub async fn handle_display_change(&mut self);
    
    /// Update capture configuration
    pub fn update_config(&mut self, config: CaptureConfig);
}

pub struct CaptureConfig {
    pub target_fps: u32,
    pub max_fps: u32,
    pub scale_factor: f32,  // 0.25 to 1.0
    pub quality: CaptureQuality,
}

pub struct MonitorInfo {
    pub id: MonitorId,
    pub name: String,
    pub bounds: Rect,
    pub is_primary: bool,
    pub scale_factor: f32,
}

/// Platform-specific capture implementation
#[async_trait]
pub trait PlatformCapturer: Send + Sync {
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
    fn supported_formats(&self) -> Vec<FrameFormat>;
    fn set_target_fps(&mut self, fps: u32);
}
```

### Input Injector

```rust
/// Injects input events into the system
pub struct InputInjector {
    platform_injector: Box<dyn PlatformInjector>,
    coordinate_mapper: CoordinateMapper,
    held_keys: HashSet<KeyCode>,
    enabled: AtomicBool,
}

impl InputInjector {
    /// Inject input event
    pub fn inject(&mut self, event: InputEventV1) -> Result<(), InputError>;
    
    /// Release all held keys (session end cleanup)
    pub fn release_all_keys(&mut self);
    
    /// Enable/disable input injection
    pub fn set_enabled(&self, enabled: bool);
    
    /// Update coordinate mapping for display
    pub fn update_display_mapping(&mut self, remote_size: Size, local_bounds: Rect);
}

pub struct CoordinateMapper {
    remote_size: Size,
    local_bounds: Rect,
    scale: f32,
    offset: Point,
}

impl CoordinateMapper {
    /// Map remote coordinates to local display coordinates
    pub fn map(&self, remote_x: i32, remote_y: i32) -> (i32, i32);
    
    /// Clamp coordinates to valid bounds
    pub fn clamp(&self, x: i32, y: i32) -> (i32, i32);
}

/// Platform-specific input injection
pub trait PlatformInjector: Send + Sync {
    fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError>;
    fn inject_mouse_button(&mut self, button: MouseButton, down: bool) -> Result<(), InputError>;
    fn inject_mouse_scroll(&mut self, delta: i32) -> Result<(), InputError>;
    fn inject_key(&mut self, code: KeyCode, down: bool) -> Result<(), InputError>;
    fn inject_text(&mut self, text: &str) -> Result<(), InputError>;
    fn inject_special_sequence(&mut self, seq: SpecialSequence) -> Result<(), InputError>;
}
```

### Transport Layer (WebRTC-first)

```rust
/// Manages media transport connectivity (WebRTC-first)
pub struct MediaTransportLayer {
    /// WebRTC peer connection (via libwebrtc FFI)
    webrtc_peer: WebRtcPeer,
    /// Identity-bound DTLS certificate
    identity_cert: IdentityBoundCert,
    /// ICE configuration (STUN/TURN servers)
    ice_config: IceConfig,
    /// Signaling channel (via rendezvous or mesh)
    signaling: Box<dyn SignalingChannel>,
}

impl MediaTransportLayer {
    /// Start WebRTC peer connection
    pub async fn start(&mut self, config: &TransportConfig) -> Result<(), TransportError>;
    
    /// Stop all transports
    pub async fn stop(&mut self);
    
    /// Create offer for session initiation
    pub async fn create_offer(&mut self) -> Result<SessionDescription, TransportError>;
    
    /// Handle incoming answer
    pub async fn handle_answer(&mut self, answer: SessionDescription) -> Result<(), TransportError>;
    
    /// Add ICE candidate
    pub async fn add_ice_candidate(&mut self, candidate: IceCandidate) -> Result<(), TransportError>;
    
    /// Get video track for sending frames
    pub fn video_track(&self) -> &VideoTrack;
    
    /// Get data channel for control messages
    pub fn control_channel(&self) -> &DataChannel;
    
    /// Get current transport status
    pub fn status(&self) -> TransportStatus;
}

pub struct IceConfig {
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
    pub ice_transport_policy: IceTransportPolicy,
}

pub struct TurnServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

pub enum IceTransportPolicy {
    All,      // Use all available transports (default)
    Relay,    // Force relay (for testing or restrictive networks)
}
```

### Signaling Layer (Rust, your protocol)

```rust
/// Signaling for WebRTC session establishment
/// Uses rendezvous mailbox or mesh for message exchange
pub struct SignalingManager {
    rendezvous_adapter: Option<RendezvousAdapter>,
    mesh_adapter: Option<MeshAdapter>,
    message_router: MessageRouter,
}

impl SignalingManager {
    /// Send signaling message (offer/answer/ICE candidate)
    pub async fn send(&self, operator_id: &OperatorId, msg: SignalingMessage) -> Result<(), SignalingError>;
    
    /// Receive incoming signaling messages
    pub fn incoming(&self) -> mpsc::Receiver<(OperatorId, SignalingMessage)>;
}

pub enum SignalingMessage {
    Offer { sdp: String, cert_binding: CertBindingV1 },
    Answer { sdp: String, cert_binding: CertBindingV1 },
    IceCandidate { candidate: String, sdp_mid: String, sdp_mline_index: u32 },
    SessionEnd { reason: String },
}

/// Certificate binding for identity verification
pub struct CertBindingV1 {
    pub dtls_fingerprint: [u8; 32],
    pub fingerprint_signature: Signature,
    pub signer_sign_pub: [u8; 32],
}
```

### Replay Protection

```rust
/// Replay protection for session AEAD
pub struct ReplayFilter {
    /// Per-stream counter tracking
    stream_counters: HashMap<u32, StreamCounter>,
    /// Sliding window size for out-of-order tolerance
    window_size: u64,
}

pub struct StreamCounter {
    /// Highest seen counter value
    highest_seen: u64,
    /// Bitmap for sliding window (tracks seen packets in window)
    window_bitmap: u64,
}

impl ReplayFilter {
    /// Check if packet is a replay, update state if not
    pub fn check_and_update(&mut self, stream_id: u32, counter: u64) -> Result<(), ReplayError>;
    
    /// Generate deterministic nonce from stream_id and counter
    pub fn generate_nonce(stream_id: u32, counter: u64) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[0..4].copy_from_slice(&stream_id.to_le_bytes());
        nonce[4..12].copy_from_slice(&counter.to_le_bytes());
        nonce
    }
}

pub enum ReplayError {
    DuplicatePacket { stream_id: u32, counter: u64 },
    CounterTooOld { stream_id: u32, counter: u64, window_start: u64 },
}
```

## Data Models

### Configuration Schema

```toml
# Agent configuration (agent.toml)

[identity]
key_store = "os"  # "os" | "file"
key_path = ""     # Only for file-based storage

[consent]
mode = "always_require"  # "always_require" | "unattended_allowed" | "trusted_only"
timeout_seconds = 30
show_indicator = true

[capture]
default_fps = 30
max_fps = 60
default_quality = "balanced"  # "low" | "balanced" | "high"
default_monitor = "primary"

[transport]
# Signaling/control plane
rendezvous_urls = ["https://rendezvous.zippyremote.io"]
directory_urls = ["https://dir.zippyremote.io"]
mesh_enabled = true

# WebRTC ICE configuration
stun_servers = ["stun:stun.l.google.com:19302"]
turn_servers = []  # Self-hosted coturn recommended

[transport.turn]
# Example coturn configuration
# urls = ["turn:turn.example.com:3478"]
# username = "zrc"
# credential = "secret"

[session]
max_concurrent = 1
timeout_hours = 8
idle_timeout_minutes = 30

[security]
# Replay protection
replay_window_size = 1024
# Alert on cert change
alert_on_cert_change = true
# Paranoid mode (for self-hosters)
paranoid_mode = false

[logging]
level = "info"
file = ""  # Empty = system log only
audit_file = "audit.log"
max_size_mb = 100
```

### State Persistence

```rust
/// Pairings database schema
pub struct PairingsDatabase {
    // SQLite with encryption
    conn: Connection,
}

// Schema:
// CREATE TABLE pairings (
//     operator_id BLOB PRIMARY KEY,
//     operator_sign_pub BLOB NOT NULL,
//     operator_kex_pub BLOB NOT NULL,
//     operator_name TEXT,
//     permissions INTEGER NOT NULL,
//     paired_at INTEGER NOT NULL,
//     last_session INTEGER,
//     session_count INTEGER DEFAULT 0
// );
//
// CREATE TABLE audit_log (
//     id INTEGER PRIMARY KEY,
//     timestamp INTEGER NOT NULL,
//     event_type TEXT NOT NULL,
//     operator_id BLOB,
//     session_id BLOB,
//     details TEXT,
//     signature BLOB NOT NULL
// );
```

## Correctness Properties

### Property 1: Consent Enforcement
*For any* session request where consent_mode is ALWAYS_REQUIRE, the session SHALL NOT be established without explicit user approval.
**Validates: Requirements 4.3, 5.1, 5.2**

### Property 2: Permission Boundary
*For any* active session, the effective permissions SHALL be the intersection of: operator's paired permissions, session-requested capabilities, and consent-granted permissions.
**Validates: Requirements 4.4, 5.7**

### Property 3: Session Ticket Validity
*For any* session operation, the session ticket SHALL be valid (not expired, matching session_id, valid signature).
**Validates: Requirements 4.5, 4.7**

### Property 4: Key Release on Session End
*For any* session termination (normal or abnormal), all held keys SHALL be released within 100ms.
**Validates: Requirement 7.6**

### Property 5: Rate Limit Enforcement
*For any* source address, pairing attempts exceeding the rate limit SHALL be rejected without processing.
**Validates: Requirement 3.7**

### Property 6: Audit Log Integrity
*For any* audit log entry, the signature SHALL be verifiable with the device's signing key.
**Validates: Requirement 13.5**

### Property 7: Transport Preference Order
*When* multiple transports are available, the agent SHALL prefer them in order: mesh → WebRTC P2P → TURN relay → rendezvous.
**Validates: Requirement 10.8**

### Property 8: Capture Frame Rate Limiting
*For any* capture configuration, the actual frame rate SHALL NOT exceed max_fps.
**Validates: Requirement 6.3**

### Property 9: Identity-Bound DTLS (CRITICAL)
*For any* WebRTC session, the DTLS certificate fingerprint SHALL be signed by the device's Ed25519 identity key, and the operator SHALL verify this signature against the pinned identity from PairReceipt.
**Validates: Requirements 11.2, 11.3**

### Property 10: Replay Protection
*For any* encrypted packet, replaying a previously-seen packet (same stream_id + counter) SHALL be rejected.
**Validates: Requirements 11a.3, 11a.4**

### Property 11: Nonce Uniqueness
*For any* two distinct packets, the nonces SHALL be different (guaranteed by deterministic nonce = stream_id || counter).
**Validates: Requirement 11a.1**

### Property 12: Cert Change Alert
*For any* session where the DTLS cert fingerprint differs from the previously-seen fingerprint for the same device identity, the agent SHALL alert the operator and require explicit re-approval.
**Validates: Requirement 11.9**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| Key store unavailable | Log error, refuse to start | Retry with file-based fallback |
| Pairing DB corrupted | Log error, backup and recreate | Require re-pairing |
| Consent timeout | Deny request | Operator can retry |
| Capture API failure | Fall back to slower method | Log and notify |
| Transport disconnect | Attempt reconnection | Exponential backoff |
| Session ticket expired | Terminate session | Operator must re-initiate |
| Input injection blocked | Log warning | Skip event, continue |
| Config file invalid | Use defaults, log warnings | Continue with defaults |

## Testing Strategy

### Unit Tests
- Identity generation and key operations
- Pairing request validation and proof verification
- Session ticket generation and validation
- Policy evaluation logic
- Coordinate mapping calculations
- Rate limiter behavior

### Property-Based Tests
- Consent enforcement across all consent modes (1000+ scenarios)
- Permission boundary calculations
- Session ticket validity checks
- Key release timing verification
- Rate limit enforcement under load

### Integration Tests
- Full pairing flow (invite → request → receipt)
- Session establishment and teardown
- Multi-transport failover
- Capture engine with mock platform
- Input injection with mock platform

### Platform Tests
- Windows Service lifecycle
- Linux systemd integration
- macOS launchd integration
- Platform-specific capture APIs
- Platform-specific input injection
