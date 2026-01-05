# Design Document: zrc-desktop

## Overview

The zrc-desktop crate implements a graphical user interface (GUI) for the Zippy Remote Control (ZRC) system. This desktop application provides a user-friendly experience for viewing and controlling remote devices, managing pairings, and handling file transfers. Built with egui/eframe for cross-platform compatibility.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            zrc-desktop                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         UI Layer (egui)                               │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Main      │  │   Viewer    │  │  Settings   │                  │   │
│  │  │   Window    │  │   Window    │  │   Dialog    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Device     │  │   Pairing   │  │   File      │                  │   │
│  │  │  List       │  │   Wizard    │  │  Transfer   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Application Core                                 │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   App       │  │   Session   │  │   Device    │                  │   │
│  │  │   State     │  │   Manager   │  │   Manager   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Input     │  │  Clipboard  │  │    File     │                  │   │
│  │  │   Handler   │  │   Sync      │  │  Transfer   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Rendering Layer                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Frame     │  │   Texture   │  │    GPU      │                  │   │
│  │  │   Decoder   │  │   Manager   │  │  Renderer   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Network Layer                                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Transport  │  │    QUIC     │  │   Presence  │                  │   │
│  │  │   Client    │  │   Client    │  │   Monitor   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Application State

```rust
/// Main application state
pub struct ZrcDesktopApp {
    /// Operator identity
    identity: Arc<IdentityManager>,
    /// Device management
    device_manager: Arc<DeviceManager>,
    /// Active sessions
    session_manager: Arc<SessionManager>,
    /// Application settings
    settings: Settings,
    /// UI state
    ui_state: UiState,
    /// Background task runtime
    runtime: tokio::runtime::Handle,
}

impl eframe::App for ZrcDesktopApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_background_events();
        self.render_ui(ctx, frame);
    }
}

pub struct UiState {
    /// Current view
    pub current_view: View,
    /// Open viewer windows
    pub viewer_windows: HashMap<SessionId, ViewerWindow>,
    /// Active dialogs
    pub dialogs: Vec<Dialog>,
    /// Notifications
    pub notifications: VecDeque<Notification>,
    /// Search/filter text
    pub search_text: String,
}

pub enum View {
    DeviceList,
    Settings,
    About,
}

pub enum Dialog {
    PairingWizard(PairingWizardState),
    SasVerification(SasVerificationState),
    ConnectionProgress(ConnectionProgressState),
    FileTransfer(FileTransferState),
    Confirmation(ConfirmationState),
}
```

### Device Manager

```rust
/// Manages paired devices and their status
pub struct DeviceManager {
    pairings_store: Arc<PairingsStore>,
    presence_monitor: Arc<PresenceMonitor>,
    devices: RwLock<HashMap<DeviceId, DeviceInfo>>,
    groups: RwLock<Vec<DeviceGroup>>,
}

impl DeviceManager {
    /// Get all devices
    pub fn list_devices(&self) -> Vec<DeviceInfo>;
    
    /// Get devices by group
    pub fn list_by_group(&self, group_id: &GroupId) -> Vec<DeviceInfo>;
    
    /// Search devices
    pub fn search(&self, query: &str) -> Vec<DeviceInfo>;
    
    /// Get device by ID
    pub fn get_device(&self, id: &DeviceId) -> Option<DeviceInfo>;
    
    /// Update device display name
    pub fn set_device_name(&self, id: &DeviceId, name: &str) -> Result<(), DeviceError>;
    
    /// Move device to group
    pub fn move_to_group(&self, id: &DeviceId, group: &GroupId) -> Result<(), DeviceError>;
    
    /// Remove device (revoke pairing)
    pub fn remove_device(&self, id: &DeviceId) -> Result<(), DeviceError>;
    
    /// Import pairing from invite
    pub async fn import_invite(&self, source: InviteSource) -> Result<DeviceId, DeviceError>;
}

pub struct DeviceInfo {
    pub id: DeviceId,
    pub display_name: String,
    pub status: DeviceStatus,
    pub permissions: Permissions,
    pub paired_at: SystemTime,
    pub last_seen: Option<SystemTime>,
    pub group_id: Option<GroupId>,
}

pub enum DeviceStatus {
    Online { latency_ms: u32 },
    Offline { last_seen: SystemTime },
    Connecting,
    Unknown,
}

pub struct DeviceGroup {
    pub id: GroupId,
    pub name: String,
    pub color: Color32,
    pub expanded: bool,
}
```

### Session Manager

```rust
/// Manages active remote sessions
pub struct SessionManager {
    identity: Arc<IdentityManager>,
    transport: Arc<TransportClient>,
    active_sessions: RwLock<HashMap<SessionId, ActiveSession>>,
    event_sender: mpsc::Sender<SessionEvent>,
}

impl SessionManager {
    /// Initiate connection to device
    pub async fn connect(&self, device_id: &DeviceId) -> Result<SessionId, SessionError>;
    
    /// Get active session
    pub fn get_session(&self, id: &SessionId) -> Option<Arc<ActiveSession>>;
    
    /// List active sessions
    pub fn list_sessions(&self) -> Vec<SessionInfo>;
    
    /// Disconnect session
    pub async fn disconnect(&self, id: &SessionId) -> Result<(), SessionError>;
    
    /// Disconnect all sessions
    pub async fn disconnect_all(&self);
    
    /// Subscribe to session events
    pub fn subscribe(&self) -> mpsc::Receiver<SessionEvent>;
}

pub struct ActiveSession {
    pub id: SessionId,
    pub device_id: DeviceId,
    pub capabilities: Capabilities,
    pub started_at: Instant,
    pub quic_connection: QuicConnection,
    pub frame_receiver: FrameReceiver,
    pub control_sender: ControlSender,
    pub stats: SessionStats,
}

pub struct SessionStats {
    pub frames_received: AtomicU64,
    pub bytes_received: AtomicU64,
    pub current_fps: AtomicU32,
    pub latency_ms: AtomicU32,
    pub packet_loss: AtomicF32,
}

pub enum SessionEvent {
    Connected { session_id: SessionId },
    Disconnected { session_id: SessionId, reason: String },
    QualityChanged { session_id: SessionId, quality: ConnectionQuality },
    Error { session_id: SessionId, error: String },
}
```

### Viewer Window

```rust
/// Remote desktop viewer window
pub struct ViewerWindow {
    session_id: SessionId,
    session: Arc<ActiveSession>,
    renderer: FrameRenderer,
    input_handler: InputHandler,
    state: ViewerState,
    toolbar: ViewerToolbar,
}

impl ViewerWindow {
    /// Create new viewer window
    pub fn new(session: Arc<ActiveSession>) -> Self;
    
    /// Render the viewer
    pub fn render(&mut self, ctx: &egui::Context);
    
    /// Handle input events
    pub fn handle_input(&mut self, event: &egui::Event);
    
    /// Toggle fullscreen
    pub fn toggle_fullscreen(&mut self);
    
    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: ZoomLevel);
    
    /// Select monitor
    pub fn select_monitor(&mut self, monitor: MonitorId);
}

pub struct ViewerState {
    pub fullscreen: bool,
    pub zoom: ZoomLevel,
    pub input_mode: InputMode,
    pub selected_monitor: MonitorId,
    pub show_toolbar: bool,
    pub show_stats: bool,
}

pub enum ZoomLevel {
    Fit,
    Actual,
    Custom(f32),
}

pub enum InputMode {
    ViewOnly,
    Control,
}

pub struct ViewerToolbar {
    pub visible: bool,
    pub position: ToolbarPosition,
}
```

### Frame Renderer

```rust
/// GPU-accelerated frame rendering
pub struct FrameRenderer {
    texture_manager: TextureManager,
    current_frame: Option<DecodedFrame>,
    render_stats: RenderStats,
}

impl FrameRenderer {
    /// Create renderer with GPU context
    pub fn new(ctx: &egui::Context) -> Self;
    
    /// Update with new frame
    pub fn update_frame(&mut self, frame: EncodedFrame);
    
    /// Render current frame to UI
    pub fn render(&mut self, ui: &mut egui::Ui, available_size: Vec2) -> Rect;
    
    /// Get render statistics
    pub fn stats(&self) -> &RenderStats;
}

pub struct TextureManager {
    textures: HashMap<TextureId, egui::TextureHandle>,
    format: FrameFormat,
}

impl TextureManager {
    /// Upload frame to GPU texture
    pub fn upload_frame(&mut self, ctx: &egui::Context, frame: &DecodedFrame) -> TextureId;
    
    /// Get texture for rendering
    pub fn get_texture(&self, id: TextureId) -> Option<&egui::TextureHandle>;
    
    /// Release old textures
    pub fn cleanup(&mut self);
}

pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub format: FrameFormat,
    pub data: Vec<u8>,
    pub timestamp: Instant,
}

pub struct RenderStats {
    pub frames_rendered: u64,
    pub dropped_frames: u64,
    pub avg_decode_time_us: u64,
    pub avg_upload_time_us: u64,
}
```

### Input Handler

```rust
/// Captures and transmits input events
pub struct InputHandler {
    control_sender: ControlSender,
    coordinate_mapper: CoordinateMapper,
    state: InputState,
    enabled: bool,
}

impl InputHandler {
    /// Handle egui input event
    pub fn handle_event(&mut self, event: &egui::Event, viewer_rect: Rect);
    
    /// Enable/disable input capture
    pub fn set_enabled(&mut self, enabled: bool);
    
    /// Send special key sequence
    pub fn send_special_sequence(&mut self, seq: SpecialSequence);
    
    /// Flush pending events
    pub fn flush(&mut self);
}

pub struct InputState {
    pub mouse_position: Option<Pos2>,
    pub pressed_keys: HashSet<egui::Key>,
    pub pressed_buttons: HashSet<egui::PointerButton>,
    pub modifiers: egui::Modifiers,
}

pub struct CoordinateMapper {
    viewer_rect: Rect,
    remote_size: Vec2,
}

impl CoordinateMapper {
    /// Map local coordinates to remote
    pub fn map_to_remote(&self, local: Pos2) -> (i32, i32);
    
    /// Check if point is within viewer
    pub fn contains(&self, point: Pos2) -> bool;
}

pub enum SpecialSequence {
    CtrlAltDel,
    AltTab,
    AltF4,
    PrintScreen,
    Custom(Vec<KeyCode>),
}
```

### Clipboard Sync

```rust
/// Bidirectional clipboard synchronization
pub struct ClipboardSync {
    session: Arc<ActiveSession>,
    local_clipboard: arboard::Clipboard,
    last_local_hash: AtomicU64,
    enabled: AtomicBool,
}

impl ClipboardSync {
    /// Start clipboard monitoring
    pub fn start(&self);
    
    /// Stop clipboard monitoring
    pub fn stop(&self);
    
    /// Enable/disable sync
    pub fn set_enabled(&self, enabled: bool);
    
    /// Handle incoming clipboard from remote
    pub fn handle_remote_clipboard(&self, content: ClipboardContent);
    
    /// Check and send local clipboard changes
    pub fn check_local_changes(&self);
}

pub enum ClipboardContent {
    Text(String),
    Image { width: u32, height: u32, data: Vec<u8> },
}
```

### File Transfer

```rust
/// File transfer management
pub struct FileTransferManager {
    session: Arc<ActiveSession>,
    transfers: RwLock<HashMap<TransferId, Transfer>>,
    event_sender: mpsc::Sender<TransferEvent>,
}

impl FileTransferManager {
    /// Upload file to remote
    pub async fn upload(&self, local_path: &Path, remote_path: &str) -> Result<TransferId, TransferError>;
    
    /// Download file from remote
    pub async fn download(&self, remote_path: &str, local_path: &Path) -> Result<TransferId, TransferError>;
    
    /// List transfers
    pub fn list_transfers(&self) -> Vec<TransferInfo>;
    
    /// Pause transfer
    pub fn pause(&self, id: TransferId) -> Result<(), TransferError>;
    
    /// Resume transfer
    pub fn resume(&self, id: TransferId) -> Result<(), TransferError>;
    
    /// Cancel transfer
    pub fn cancel(&self, id: TransferId) -> Result<(), TransferError>;
}

pub struct Transfer {
    pub id: TransferId,
    pub direction: TransferDirection,
    pub local_path: PathBuf,
    pub remote_path: String,
    pub total_bytes: u64,
    pub transferred_bytes: AtomicU64,
    pub state: TransferState,
    pub started_at: Instant,
}

pub enum TransferDirection {
    Upload,
    Download,
}

pub enum TransferState {
    Pending,
    InProgress { speed_bps: u64 },
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

pub struct TransferInfo {
    pub id: TransferId,
    pub filename: String,
    pub direction: TransferDirection,
    pub progress: f32,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub state: TransferState,
}
```

### Presence Monitor

```rust
/// Monitors device online status
pub struct PresenceMonitor {
    transport: Arc<TransportClient>,
    devices: Arc<RwLock<HashMap<DeviceId, PresenceInfo>>>,
    poll_interval: Duration,
}

impl PresenceMonitor {
    /// Start monitoring
    pub fn start(&self);
    
    /// Stop monitoring
    pub fn stop(&self);
    
    /// Get device presence
    pub fn get_presence(&self, device_id: &DeviceId) -> Option<PresenceInfo>;
    
    /// Force refresh
    pub async fn refresh(&self);
}

pub struct PresenceInfo {
    pub online: bool,
    pub last_seen: SystemTime,
    pub latency_ms: Option<u32>,
    pub transport: Option<TransportType>,
}
```

## Data Models

### Settings Schema

```rust
#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub appearance: AppearanceSettings,
    pub input: InputSettings,
    pub transport: TransportSettings,
    pub notifications: NotificationSettings,
    pub shortcuts: ShortcutSettings,
}

#[derive(Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme: Theme,
    pub font_size: f32,
    pub show_device_icons: bool,
    pub compact_mode: bool,
}

#[derive(Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize)]
pub struct InputSettings {
    pub default_mode: InputMode,
    pub scroll_sensitivity: f32,
    pub capture_system_keys: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TransportSettings {
    pub preference: TransportPreference,
    pub rendezvous_urls: Vec<String>,
    pub relay_urls: Vec<String>,
    pub connection_timeout_seconds: u32,
}

#[derive(Serialize, Deserialize)]
pub struct NotificationSettings {
    pub connection_events: bool,
    pub transfer_complete: bool,
    pub sounds_enabled: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ShortcutSettings {
    pub fullscreen: KeyboardShortcut,
    pub disconnect: KeyboardShortcut,
    pub toggle_input: KeyboardShortcut,
    pub send_ctrl_alt_del: KeyboardShortcut,
}
```

### Window State Persistence

```rust
#[derive(Serialize, Deserialize)]
pub struct WindowState {
    pub main_window: WindowGeometry,
    pub viewer_windows: HashMap<String, ViewerWindowState>,
}

#[derive(Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ViewerWindowState {
    pub geometry: WindowGeometry,
    pub fullscreen: bool,
    pub zoom: ZoomLevel,
    pub selected_monitor: Option<MonitorId>,
}
```

## Correctness Properties

### Property 1: Frame Ordering
*For any* sequence of received frames, frames SHALL be displayed in timestamp order, dropping late frames rather than displaying out of order.
**Validates: Requirement 4.4**

### Property 2: Input Coordinate Accuracy
*For any* input event, the mapped remote coordinates SHALL be within ±1 pixel of the mathematically correct mapping.
**Validates: Requirement 5.5**

### Property 3: Session Cleanup
*For any* session termination (normal or abnormal), all associated resources (textures, streams, handlers) SHALL be released within 1 second.
**Validates: Requirement 2.6**

### Property 4: Clipboard Size Enforcement
*For any* clipboard sync operation, content exceeding the size limit SHALL be rejected without partial transfer.
**Validates: Requirement 7.7**

### Property 5: Transfer Integrity
*For any* completed file transfer, the local file hash SHALL match the remote file hash.
**Validates: Requirement 8.6**

### Property 6: Settings Persistence
*For any* settings change, the new value SHALL be persisted and restored on next application launch.
**Validates: Requirement 11.6**

### Property 7: Connection Quality Indication
*For any* active session, the displayed connection quality SHALL reflect actual metrics within 2 seconds of measurement.
**Validates: Requirements 12.1, 12.2, 12.3**

### Property 8: Accessibility Compliance
*For any* UI element, keyboard navigation SHALL be possible and screen reader labels SHALL be present.
**Validates: Requirement 13.6**

## Error Handling

| Error Condition | User Feedback | Recovery |
|-----------------|---------------|----------|
| Connection failed | Toast + dialog with retry | Offer retry, show transport options |
| Session disconnected | Toast notification | Auto-reconnect option |
| Frame decode error | Skip frame, log warning | Continue with next frame |
| Clipboard too large | Toast warning | Truncate or skip |
| Transfer failed | Progress dialog error | Offer retry/resume |
| Settings save failed | Toast error | Retry, use defaults |
| GPU context lost | Error dialog | Restart renderer |
| Out of memory | Error dialog | Close oldest viewer |

## Testing Strategy

### Unit Tests
- Coordinate mapping calculations
- Settings serialization/deserialization
- Device search/filter logic
- Transfer progress calculations
- Shortcut parsing

### Property-Based Tests
- Frame ordering across random arrival patterns
- Coordinate mapping accuracy
- Settings round-trip persistence
- Transfer integrity verification

### Integration Tests
- Full connection flow with mock agent
- Frame rendering pipeline
- Input capture and transmission
- Clipboard sync bidirectional
- File transfer complete flow

### UI Tests
- Device list rendering
- Viewer window interactions
- Dialog workflows
- Keyboard navigation
- Theme switching

### Platform Tests
- Windows: system tray, notifications, DPI
- macOS: menu bar, notifications, Retina
- Linux: various DEs, Wayland/X11
