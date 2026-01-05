# Design Document: zrc-platform-win

## Overview

The zrc-platform-win crate implements Windows-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture via DXGI/WGC/GDI, input injection via SendInput, and system integration including Windows Service support and DPAPI key storage.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          zrc-platform-win                                    │
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
│  │                      Windows Implementations                          │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Screen Capture                            │     │   │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                      │     │   │
│  │  │  │   WGC   │  │  DXGI   │  │   GDI   │                      │     │   │
│  │  │  │ Capture │  │ DeskDup │  │ Fallback│                      │     │   │
│  │  │  └─────────┘  └─────────┘  └─────────┘                      │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Input Injection                           │     │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │     │   │
│  │  │  │  SendInput  │  │   Special   │  │  Coordinate │          │     │   │
│  │  │  │   Wrapper   │  │    Keys     │  │   Mapper    │          │     │   │
│  │  │  └─────────────┘  └─────────────┘  └─────────────┘          │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    System Integration                        │     │   │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │     │   │
│  │  │  │ Service │  │  DPAPI  │  │Clipboard│  │  UAC    │        │     │   │
│  │  │  │ Control │  │ Storage │  │  Access │  │ Handler │        │     │   │
│  │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Screen Capture - Unified Interface

```rust
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Direct3D11::*;

/// Windows screen capture with automatic backend selection
pub struct WinCapturer {
    backend: CaptureBackend,
    monitors: Vec<WinMonitorInfo>,
    config: CaptureConfig,
}

enum CaptureBackend {
    Wgc(WgcCapturer),
    Dxgi(DxgiCapturer),
    Gdi(GdiCapturer),
}

impl WinCapturer {
    /// Create capturer with best available backend
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        let backend = if WgcCapturer::is_available() {
            CaptureBackend::Wgc(WgcCapturer::new()?)
        } else if DxgiCapturer::is_available() {
            CaptureBackend::Dxgi(DxgiCapturer::new()?)
        } else {
            CaptureBackend::Gdi(GdiCapturer::new()?)
        };
        
        Ok(Self {
            backend,
            monitors: enumerate_monitors()?,
            config,
        })
    }
    
    /// Force specific backend
    pub fn with_backend(backend_type: BackendType, config: CaptureConfig) -> Result<Self, CaptureError>;
}

impl PlatformCapturer for WinCapturer {
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
    fn supported_formats(&self) -> Vec<FrameFormat>;
    fn set_target_fps(&mut self, fps: u32);
    fn list_monitors(&self) -> &[MonitorInfo];
    fn select_monitor(&mut self, monitor: MonitorId) -> Result<(), CaptureError>;
}
```

### DXGI Desktop Duplication

```rust
/// DXGI Desktop Duplication capture (Windows 8+)
pub struct DxgiCapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    output_duplication: IDXGIOutputDuplication,
    staging_texture: ID3D11Texture2D,
    current_output: u32,
}

impl DxgiCapturer {
    /// Check if DXGI Desktop Duplication is available
    pub fn is_available() -> bool;
    
    /// Create DXGI capturer for specified output
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Capture next frame with dirty rectangles
    pub fn capture_frame(&mut self, timeout_ms: u32) -> Result<DxgiFrame, CaptureError>;
    
    /// Handle device lost error
    pub fn handle_device_lost(&mut self) -> Result<(), CaptureError>;
    
    /// Handle desktop switch (UAC, lock screen)
    pub fn handle_desktop_switch(&mut self) -> Result<(), CaptureError>;
}

pub struct DxgiFrame {
    pub texture: ID3D11Texture2D,
    pub dirty_rects: Vec<RECT>,
    pub move_rects: Vec<DXGI_OUTDUPL_MOVE_RECT>,
    pub timestamp: i64,
    pub width: u32,
    pub height: u32,
}
```

### Windows Graphics Capture

```rust
/// Windows Graphics Capture (Windows 10 1903+)
pub struct WgcCapturer {
    capture_item: GraphicsCaptureItem,
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    frame_receiver: mpsc::Receiver<WgcFrame>,
}

impl WgcCapturer {
    /// Check if WGC is available
    pub fn is_available() -> bool {
        // Check Windows version >= 10.0.18362
        // Check GraphicsCaptureSession::IsSupported()
    }
    
    /// Create WGC capturer for monitor
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Set cursor capture
    pub fn set_cursor_capture(&mut self, enabled: bool);
    
    /// Set border highlight
    pub fn set_border_visible(&mut self, visible: bool);
}
```

### GDI Fallback

```rust
/// GDI-based capture fallback
pub struct GdiCapturer {
    screen_dc: HDC,
    memory_dc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
    buffer: Vec<u8>,
}

impl GdiCapturer {
    /// Create GDI capturer
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Capture frame using BitBlt
    pub fn capture_frame(&mut self) -> Result<GdiFrame, CaptureError>;
    
    /// Handle resolution change
    pub fn handle_resolution_change(&mut self) -> Result<(), CaptureError>;
}
```

### Input Injection

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Windows input injection via SendInput
pub struct WinInjector {
    held_keys: HashSet<u16>,
    coordinate_mapper: CoordinateMapper,
    is_elevated: bool,
}

impl WinInjector {
    /// Create input injector
    pub fn new() -> Self;
    
    /// Check if running with elevated privileges
    pub fn is_elevated(&self) -> bool;
}

impl PlatformInjector for WinInjector {
    fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        let (abs_x, abs_y) = self.coordinate_mapper.to_absolute(x, y);
        
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    ..Default::default()
                },
            },
        };
        
        unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        Ok(())
    }
    
    fn inject_mouse_button(&mut self, button: MouseButton, down: bool) -> Result<(), InputError>;
    fn inject_mouse_scroll(&mut self, delta: i32) -> Result<(), InputError>;
    fn inject_key(&mut self, code: KeyCode, down: bool) -> Result<(), InputError>;
    fn inject_text(&mut self, text: &str) -> Result<(), InputError>;
    fn inject_special_sequence(&mut self, seq: SpecialSequence) -> Result<(), InputError>;
}

/// Coordinate mapping for multi-monitor
pub struct CoordinateMapper {
    virtual_screen: RECT,
    monitors: Vec<MonitorBounds>,
}

impl CoordinateMapper {
    /// Convert logical coordinates to absolute (0-65535 range)
    pub fn to_absolute(&self, x: i32, y: i32) -> (i32, i32);
    
    /// Clamp to valid screen bounds
    pub fn clamp(&self, x: i32, y: i32) -> (i32, i32);
}
```

### Special Key Sequences

```rust
/// Special key sequence handler
pub struct SpecialKeyHandler {
    is_service: bool,
}

impl SpecialKeyHandler {
    /// Send Ctrl+Alt+Del (requires SYSTEM context)
    pub fn send_ctrl_alt_del(&self) -> Result<(), InputError> {
        if self.is_service {
            // Use SAS library or direct injection
            unsafe {
                // SendSAS(FALSE) - requires linking to sas.dll
            }
        } else {
            Err(InputError::ElevationRequired)
        }
    }
    
    /// Send Alt+Tab
    pub fn send_alt_tab(&self) -> Result<(), InputError>;
    
    /// Send Win+L (lock)
    pub fn send_lock_workstation(&self) -> Result<(), InputError>;
}
```

### Windows Service Integration

```rust
use windows::Win32::System::Services::*;

/// Windows Service wrapper
pub struct WinService {
    service_name: String,
    status_handle: SERVICE_STATUS_HANDLE,
    current_status: SERVICE_STATUS,
}

impl WinService {
    /// Run as Windows Service
    pub fn run<F>(service_name: &str, main_fn: F) -> Result<(), ServiceError>
    where
        F: FnOnce(mpsc::Receiver<ServiceControl>) -> Result<(), Box<dyn Error>>;
    
    /// Report status to SCM
    pub fn set_status(&mut self, state: ServiceState) -> Result<(), ServiceError>;
    
    /// Handle service control
    fn control_handler(control: u32) -> u32;
}

pub enum ServiceControl {
    Stop,
    Pause,
    Continue,
    Interrogate,
    Shutdown,
    SessionChange(SessionChangeEvent),
}

pub enum SessionChangeEvent {
    SessionLogon(u32),
    SessionLogoff(u32),
    SessionLock(u32),
    SessionUnlock(u32),
    ConsoleConnect(u32),
    ConsoleDisconnect(u32),
}
```

### DPAPI Key Storage

```rust
use windows::Win32::Security::Cryptography::*;

/// DPAPI-based secure key storage
pub struct DpapiKeyStore {
    scope: DpapiScope,
    entropy: Option<Vec<u8>>,
}

pub enum DpapiScope {
    CurrentUser,
    LocalMachine,
}

impl DpapiKeyStore {
    /// Create key store with specified scope
    pub fn new(scope: DpapiScope) -> Self;
    
    /// Add optional entropy for additional protection
    pub fn with_entropy(mut self, entropy: &[u8]) -> Self;
}

impl KeyStore for DpapiKeyStore {
    fn store_key(&self, name: &str, key: &[u8]) -> Result<(), KeyStoreError> {
        let encrypted = unsafe {
            let mut data_in = DATA_BLOB {
                cbData: key.len() as u32,
                pbData: key.as_ptr() as *mut u8,
            };
            let mut data_out = DATA_BLOB::default();
            
            CryptProtectData(
                &mut data_in,
                None,
                self.entropy.as_ref().map(|e| /* ... */),
                None,
                None,
                self.flags(),
                &mut data_out,
            )?;
            
            // Convert to Vec<u8>
        };
        
        // Store encrypted blob to file
        let path = self.key_path(name);
        std::fs::write(path, &encrypted)?;
        Ok(())
    }
    
    fn load_key(&self, name: &str) -> Result<Vec<u8>, KeyStoreError>;
    fn delete_key(&self, name: &str) -> Result<(), KeyStoreError>;
    fn key_exists(&self, name: &str) -> bool;
}
```

### Clipboard Access

```rust
use windows::Win32::System::DataExchange::*;

/// Windows clipboard access
pub struct WinClipboard {
    hwnd: HWND,  // For clipboard viewer chain
}

impl WinClipboard {
    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError>;
    
    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;
    
    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError>;
    
    /// Write image to clipboard
    pub fn write_image(&self, image: &ClipboardImage) -> Result<(), ClipboardError>;
    
    /// Get clipboard sequence number for change detection
    pub fn sequence_number(&self) -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }
}
```

### UAC Handler

```rust
/// UAC and secure desktop handling
pub struct UacHandler {
    is_system: bool,
}

impl UacHandler {
    /// Detect if on secure desktop
    pub fn is_secure_desktop(&self) -> bool;
    
    /// Switch to secure desktop (requires SYSTEM)
    pub fn switch_to_secure_desktop(&self) -> Result<(), UacError>;
    
    /// Switch back to default desktop
    pub fn switch_to_default_desktop(&self) -> Result<(), UacError>;
    
    /// Get current desktop name
    pub fn current_desktop_name(&self) -> String;
}
```

## Data Models

### Monitor Information

```rust
pub struct WinMonitorInfo {
    pub handle: HMONITOR,
    pub device_name: String,
    pub friendly_name: String,
    pub bounds: RECT,
    pub work_area: RECT,
    pub is_primary: bool,
    pub dpi: u32,
    pub refresh_rate: u32,
}
```

## Correctness Properties

### Property 1: Capture Backend Fallback
*When* a preferred capture backend is unavailable, the system SHALL automatically fall back to the next available backend in order: WGC → DXGI → GDI.
**Validates: Requirements 1.1, 2.2, 3.2**

### Property 2: Input Coordinate Accuracy
*For any* input injection, coordinates SHALL be correctly mapped to the virtual desktop space accounting for multi-monitor configuration and DPI scaling.
**Validates: Requirements 5.5, 5.6, 5.8**

### Property 3: Key State Cleanup
*For any* session termination, all held keys SHALL be released via SendInput within 100ms.
**Validates: Requirement 6.7**

### Property 4: DPAPI Scope Isolation
*For any* key stored with CurrentUser scope, the key SHALL NOT be accessible from other user contexts.
**Validates: Requirements 10.2, 10.3**

### Property 5: Service Status Reporting
*For any* service state change, the status SHALL be reported to SCM within 1 second.
**Validates: Requirement 9.3**

### Property 6: Desktop Switch Recovery
*When* a desktop switch occurs (UAC, lock screen), the capture system SHALL detect and recover within 2 seconds.
**Validates: Requirements 2.5, 8.1, 8.4**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| DXGI device lost | Log, recreate device | Automatic retry |
| Desktop switch | Pause capture | Resume on switch back |
| SendInput failure | Log warning | Skip event |
| DPAPI failure | Return error | Fall back to file |
| Service SCM error | Log to Event Log | Continue operation |
| Clipboard locked | Retry with backoff | Return error after 3 attempts |

## Testing Strategy

### Unit Tests
- Coordinate mapping calculations
- DPAPI encrypt/decrypt round-trip
- Monitor enumeration parsing
- Input event construction

### Integration Tests
- Full capture pipeline with each backend
- Input injection verification
- Service lifecycle
- Clipboard read/write

### Platform Tests
- Windows 10 vs Windows 11 behavior
- Multi-monitor configurations
- High DPI scenarios
- UAC prompt handling
