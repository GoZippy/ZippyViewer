# Design Document: zrc-platform-linux

## Overview

The zrc-platform-linux crate implements Linux-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture via X11/PipeWire, input injection via XTest/uinput, and system integration including systemd service support and libsecret key storage.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          zrc-platform-linux                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Platform Traits (from zrc-core)                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │ Platform    │  │ Platform    │  │ Platform    │                  │   │
│  │  │ Capturer    │  │ Injector    │  │ KeyStore    │                  │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │   │
│  └─────────┼────────────────┼────────────────┼──────────────────────────┘   │
│            │                │                │                              │
│            ▼                ▼                ▼                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Linux Implementations                            │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Screen Capture                            │     │   │
│  │  │  ┌───────────┐  ┌───────────┐  ┌───────────┐                │     │   │
│  │  │  │ PipeWire  │  │   X11     │  │  XWayland │                │     │   │
│  │  │  │  Portal   │  │   SHM     │  │  Fallback │                │     │   │
│  │  │  └───────────┘  └───────────┘  └───────────┘                │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Input Injection                           │     │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │     │   │
│  │  │  │   XTest    │  │   uinput   │  │   libei    │          │     │   │
│  │  │  │ Extension  │  │   Device   │  │  (future)  │          │     │   │
│  │  │  └─────────────┘  └─────────────┘  └─────────────┘          │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    System Integration                        │     │   │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │     │   │
│  │  │  │ systemd │  │libsecret│  │Clipboard│  │   DE    │        │     │   │
│  │  │  │ Service │  │ Storage │  │  X11/Wl │  │ Detect  │        │     │   │
│  │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```


## Components and Interfaces

### Screen Capture - Unified Interface

```rust
/// Linux screen capture with automatic backend selection
pub struct LinuxCapturer {
    backend: CaptureBackend,
    monitors: Vec<LinuxMonitorInfo>,
    config: CaptureConfig,
    session_type: SessionType,
}

enum CaptureBackend {
    PipeWire(PipeWireCapturer),
    X11Shm(X11ShmCapturer),
    X11Basic(X11BasicCapturer),
}

#[derive(Clone, Copy, PartialEq)]
pub enum SessionType {
    X11,
    Wayland,
    XWayland,
    Headless,
}

impl LinuxCapturer {
    /// Create capturer with best available backend
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        let session_type = detect_session_type();
        
        let backend = match session_type {
            SessionType::Wayland => {
                CaptureBackend::PipeWire(PipeWireCapturer::new()?)
            }
            SessionType::X11 | SessionType::XWayland => {
                if X11ShmCapturer::is_available() {
                    CaptureBackend::X11Shm(X11ShmCapturer::new()?)
                } else {
                    CaptureBackend::X11Basic(X11BasicCapturer::new()?)
                }
            }
            SessionType::Headless => {
                return Err(CaptureError::NoDisplayServer);
            }
        };
        
        Ok(Self {
            backend,
            monitors: enumerate_monitors(&session_type)?,
            config,
            session_type,
        })
    }
}

impl PlatformCapturer for LinuxCapturer {
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
    fn supported_formats(&self) -> Vec<FrameFormat>;
    fn set_target_fps(&mut self, fps: u32);
    fn list_monitors(&self) -> &[MonitorInfo];
    fn select_monitor(&mut self, monitor: MonitorId) -> Result<(), CaptureError>;
}

/// Detect display server type
fn detect_session_type() -> SessionType {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if std::env::var("DISPLAY").is_ok() {
            SessionType::XWayland
        } else {
            SessionType::Wayland
        }
    } else if std::env::var("DISPLAY").is_ok() {
        SessionType::X11
    } else {
        SessionType::Headless
    }
}
```

### PipeWire/Portal Capture (Wayland)

```rust
use ashpd::desktop::screencast::*;
use pipewire::*;

/// PipeWire-based capture via xdg-desktop-portal
pub struct PipeWireCapturer {
    portal: ScreenCast,
    session: Session,
    stream: Option<PipeWireStream>,
    frame_receiver: mpsc::Receiver<PwFrame>,
}

impl PipeWireCapturer {
    /// Create PipeWire capturer
    pub async fn new() -> Result<Self, CaptureError> {
        let portal = ScreenCast::new().await?;
        
        // Request screen capture permission via portal
        let session = portal.create_session().await?;
        
        // Select sources (shows user dialog)
        portal.select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor | SourceType::Window,
            false, // multiple
            None,  // restore token
        ).await?;
        
        // Start capture
        let response = portal.start(&session, None).await?;
        
        Ok(Self {
            portal,
            session,
            stream: None,
            frame_receiver: /* ... */,
        })
    }
    
    /// Connect to PipeWire stream
    pub fn connect_stream(&mut self, node_id: u32) -> Result<(), CaptureError>;
    
    /// Handle stream state changes
    pub fn handle_state_change(&mut self, state: StreamState);
}

pub struct PwFrame {
    pub buffer: DmaBuf,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub timestamp: u64,
}
```

### X11 SHM Capture

```rust
use x11rb::connection::Connection;
use x11rb::protocol::shm::*;

/// X11 capture using MIT-SHM extension
pub struct X11ShmCapturer {
    conn: RustConnection,
    screen_num: usize,
    shm_seg: Seg,
    shm_id: i32,
    shm_addr: *mut u8,
    width: u16,
    height: u16,
}

impl X11ShmCapturer {
    /// Check if SHM extension is available
    pub fn is_available() -> bool {
        // Query SHM extension
    }
    
    /// Create SHM capturer
    pub fn new() -> Result<Self, CaptureError> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        
        // Create shared memory segment
        let shm_id = unsafe {
            libc::shmget(libc::IPC_PRIVATE, /* size */, libc::IPC_CREAT | 0o600)
        };
        
        let shm_addr = unsafe {
            libc::shmat(shm_id, std::ptr::null(), 0) as *mut u8
        };
        
        // Attach to X server
        let shm_seg = conn.generate_id()?;
        conn.shm_attach(shm_seg, shm_id as u32, false)?;
        
        Ok(Self { /* ... */ })
    }
    
    /// Capture frame using ShmGetImage
    pub fn capture_frame(&mut self) -> Result<X11Frame, CaptureError>;
}
```

### X11 Basic Capture (Fallback)

```rust
/// X11 capture using XGetImage (slower fallback)
pub struct X11BasicCapturer {
    conn: RustConnection,
    screen_num: usize,
    root_window: Window,
}

impl X11BasicCapturer {
    /// Create basic capturer
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Capture frame using GetImage
    pub fn capture_frame(&mut self) -> Result<X11Frame, CaptureError> {
        let reply = self.conn.get_image(
            ImageFormat::Z_PIXMAP,
            self.root_window,
            0, 0,
            self.width, self.height,
            !0, // plane_mask
        )?.reply()?;
        
        Ok(X11Frame {
            data: reply.data,
            width: self.width,
            height: self.height,
            depth: reply.depth,
        })
    }
}
```

### Input Injection - XTest

```rust
use x11rb::protocol::xtest::*;

/// X11 input injection via XTest extension
pub struct XTestInjector {
    conn: RustConnection,
    held_keys: HashSet<Keycode>,
    keymap: KeyboardMapping,
}

impl XTestInjector {
    /// Check if XTest is available
    pub fn is_available() -> bool;
    
    /// Create XTest injector
    pub fn new() -> Result<Self, InputError>;
    
    /// Get keycode for keysym
    fn keysym_to_keycode(&self, keysym: Keysym) -> Option<Keycode>;
}

impl PlatformInjector for XTestInjector {
    fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        self.conn.xtest_fake_input(
            MOTION_NOTIFY,
            0,
            x11rb::CURRENT_TIME,
            self.root_window,
            x as i16,
            y as i16,
            0,
        )?;
        self.conn.flush()?;
        Ok(())
    }
    
    fn inject_mouse_button(&mut self, button: MouseButton, down: bool) -> Result<(), InputError>;
    fn inject_mouse_scroll(&mut self, delta: i32) -> Result<(), InputError>;
    fn inject_key(&mut self, code: KeyCode, down: bool) -> Result<(), InputError>;
    fn inject_text(&mut self, text: &str) -> Result<(), InputError>;
    fn inject_special_sequence(&mut self, seq: SpecialSequence) -> Result<(), InputError>;
}
```

### Input Injection - uinput

```rust
use uinput::Device;

/// uinput-based input injection (requires privileges)
pub struct UinputInjector {
    mouse_device: Device,
    keyboard_device: Device,
    held_keys: HashSet<u16>,
}

impl UinputInjector {
    /// Check if uinput is available
    pub fn is_available() -> bool {
        std::path::Path::new("/dev/uinput").exists()
    }
    
    /// Create uinput injector
    pub fn new() -> Result<Self, InputError> {
        let mouse_device = uinput::default()?
            .name("ZRC Virtual Mouse")?
            .event(uinput::event::relative::Position::X)?
            .event(uinput::event::relative::Position::Y)?
            .event(uinput::event::controller::Mouse::Left)?
            .event(uinput::event::controller::Mouse::Right)?
            .event(uinput::event::controller::Mouse::Middle)?
            .create()?;
        
        let keyboard_device = uinput::default()?
            .name("ZRC Virtual Keyboard")?
            .event(uinput::event::Keyboard::All)?
            .create()?;
        
        Ok(Self {
            mouse_device,
            keyboard_device,
            held_keys: HashSet::new(),
        })
    }
}

impl PlatformInjector for UinputInjector {
    fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        // uinput uses relative movement by default
        // For absolute, need to set up absolute axes
        self.mouse_device.position(&Position::X, x)?;
        self.mouse_device.position(&Position::Y, y)?;
        self.mouse_device.synchronize()?;
        Ok(())
    }
    
    // ... other methods
}

impl Drop for UinputInjector {
    fn drop(&mut self) {
        // Release all held keys
        for key in self.held_keys.drain() {
            let _ = self.keyboard_device.release(&key);
        }
        let _ = self.keyboard_device.synchronize();
    }
}
```

### Wayland Input Limitations

```rust
/// Wayland input capability detection
pub struct WaylandInputStatus {
    pub xwayland_available: bool,
    pub libei_available: bool,
    pub compositor: WaylandCompositor,
}

#[derive(Clone, Copy)]
pub enum WaylandCompositor {
    Gnome,
    Kde,
    Wlroots,
    Other(String),
}

impl WaylandInputStatus {
    /// Detect Wayland input capabilities
    pub fn detect() -> Self;
    
    /// Get recommended input method
    pub fn recommended_method(&self) -> InputMethod;
    
    /// Get user-facing limitation message
    pub fn limitation_message(&self) -> Option<String>;
}

pub enum InputMethod {
    XWayland,  // Use XTest via XWayland
    LibEi,     // Use libei (if available)
    None,      // No input injection possible
}
```

### Secret Storage

```rust
use secret_service::SecretService;

/// Linux secret storage via libsecret/Secret Service
pub struct SecretStore {
    service: SecretService,
    collection: Collection,
}

impl SecretStore {
    /// Create secret store
    pub async fn new() -> Result<Self, KeyStoreError> {
        let service = SecretService::connect(EncryptionType::Dh).await?;
        let collection = service.get_default_collection().await?;
        
        // Unlock if needed
        if collection.is_locked().await? {
            collection.unlock().await?;
        }
        
        Ok(Self { service, collection })
    }
}

impl KeyStore for SecretStore {
    fn store_key(&self, name: &str, key: &[u8]) -> Result<(), KeyStoreError> {
        let attributes = HashMap::from([
            ("application", "zrc-agent"),
            ("key-name", name),
        ]);
        
        self.collection.create_item(
            &format!("ZRC Key: {}", name),
            attributes,
            key,
            true, // replace
            "application/octet-stream",
        ).await?;
        
        Ok(())
    }
    
    fn load_key(&self, name: &str) -> Result<Vec<u8>, KeyStoreError>;
    fn delete_key(&self, name: &str) -> Result<(), KeyStoreError>;
    fn key_exists(&self, name: &str) -> bool;
}

/// Fallback encrypted file storage
pub struct FileKeyStore {
    path: PathBuf,
    encryption_key: [u8; 32],
}

impl FileKeyStore {
    /// Create file-based key store
    pub fn new(path: PathBuf) -> Result<Self, KeyStoreError>;
}
```

### systemd Integration

```rust
use libsystemd::daemon::*;

/// systemd service integration
pub struct SystemdService {
    unit_name: String,
}

impl SystemdService {
    /// Notify systemd of startup completion
    pub fn notify_ready(&self) -> Result<(), ServiceError> {
        notify(false, &[NotifyState::Ready])?;
        Ok(())
    }
    
    /// Notify systemd of status
    pub fn notify_status(&self, status: &str) -> Result<(), ServiceError> {
        notify(false, &[NotifyState::Status(status)])?;
        Ok(())
    }
    
    /// Notify systemd watchdog
    pub fn notify_watchdog(&self) -> Result<(), ServiceError> {
        notify(false, &[NotifyState::Watchdog])?;
        Ok(())
    }
    
    /// Generate systemd unit file
    pub fn generate_unit_file(&self, config: &UnitConfig) -> String;
}

pub struct UnitConfig {
    pub description: String,
    pub exec_start: String,
    pub user: Option<String>,
    pub restart: RestartPolicy,
    pub watchdog_sec: Option<u32>,
}
```

### Clipboard Access

```rust
/// Linux clipboard access (X11 and Wayland)
pub struct LinuxClipboard {
    backend: ClipboardBackend,
}

enum ClipboardBackend {
    X11(X11Clipboard),
    Wayland(WaylandClipboard),
}

impl LinuxClipboard {
    /// Create clipboard accessor
    pub fn new() -> Result<Self, ClipboardError> {
        let session_type = detect_session_type();
        
        let backend = match session_type {
            SessionType::Wayland => {
                ClipboardBackend::Wayland(WaylandClipboard::new()?)
            }
            _ => {
                ClipboardBackend::X11(X11Clipboard::new()?)
            }
        };
        
        Ok(Self { backend })
    }
    
    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError>;
    
    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError>;
    
    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;
}

/// X11 clipboard via CLIPBOARD and PRIMARY selections
struct X11Clipboard {
    conn: RustConnection,
    window: Window,
    clipboard_atom: Atom,
    primary_atom: Atom,
}

/// Wayland clipboard via portal
struct WaylandClipboard {
    // Use wl-clipboard or portal
}
```

### Desktop Environment Detection

```rust
/// Desktop environment detection
pub struct DesktopEnvironment {
    pub name: String,
    pub session_type: SessionType,
    pub compositor: Option<String>,
}

impl DesktopEnvironment {
    /// Detect current desktop environment
    pub fn detect() -> Self {
        let name = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("DESKTOP_SESSION"))
            .unwrap_or_else(|_| "unknown".to_string());
        
        let session_type = detect_session_type();
        
        let compositor = if session_type == SessionType::Wayland {
            std::env::var("XDG_SESSION_DESKTOP").ok()
        } else {
            None
        };
        
        Self { name, session_type, compositor }
    }
    
    /// Get capabilities based on DE
    pub fn capabilities(&self) -> Capabilities;
}
```

## Data Models

### Monitor Information

```rust
pub struct LinuxMonitorInfo {
    pub name: String,
    pub connector: String,  // e.g., "HDMI-1", "eDP-1"
    pub bounds: Rect,
    pub is_primary: bool,
    pub scale_factor: f64,
    pub refresh_rate: f64,
    pub output_id: u32,  // RandR output or Wayland output
}
```

### systemd Unit Template

```ini
[Unit]
Description=ZRC Remote Control Agent
After=network.target graphical-session.target

[Service]
Type=notify
ExecStart=/usr/bin/zrc-agent
Restart=on-failure
RestartSec=5
WatchdogSec=30

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=default.target
```

## Correctness Properties

### Property 1: Session Type Detection
*For any* Linux system, the session type SHALL be correctly detected based on WAYLAND_DISPLAY and DISPLAY environment variables.
**Validates: Requirements 1.2, 2.7, 10.2**

### Property 2: Capture Backend Selection
*When* on Wayland, the system SHALL use PipeWire/portal; when on X11, SHALL use SHM if available, otherwise basic capture.
**Validates: Requirements 1.1, 2.1, 2.8**

### Property 3: Input Method Fallback
*When* XTest is unavailable and uinput requires elevation, the system SHALL clearly report input limitations.
**Validates: Requirements 4.8, 5.8, 6.7**

### Property 4: Secret Service Fallback
*When* Secret Service (GNOME Keyring/KWallet) is unavailable, the system SHALL fall back to encrypted file storage.
**Validates: Requirements 7.3, 7.6**

### Property 5: uinput Cleanup
*For any* uinput session termination, all virtual devices SHALL be properly destroyed and held keys released.
**Validates: Requirement 5.7**

### Property 6: Portal Permission Flow
*For any* PipeWire capture, the user SHALL be prompted via xdg-desktop-portal before capture begins.
**Validates: Requirements 2.2, 2.3, 2.4**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| No display server | Return NoDisplayServer | Inform user |
| Portal denied | Return PermissionDenied | Guide user to retry |
| SHM unavailable | Fall back to basic capture | Automatic |
| uinput permission denied | Fall back to XTest | Automatic |
| Secret Service unavailable | Fall back to file storage | Automatic |
| Wayland input blocked | Report limitation | Suggest X11 session |

## Testing Strategy

### Unit Tests
- Session type detection
- Coordinate calculations
- Keycode mapping
- Unit file generation

### Integration Tests
- X11 capture pipeline
- PipeWire capture (requires portal)
- XTest input injection
- Secret Service operations

### Platform Tests
- GNOME vs KDE vs other DEs
- X11 vs Wayland vs XWayland
- Various distributions (Ubuntu, Fedora, Arch)
- Container environments
