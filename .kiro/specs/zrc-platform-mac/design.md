# Design Document: zrc-platform-mac

## Overview

The zrc-platform-mac crate implements macOS-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture via ScreenCaptureKit/CGDisplayStream, input injection via CGEvent, and system integration including Keychain storage and launchd service support.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          zrc-platform-mac                                    │
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
│  │                      macOS Implementations                            │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Screen Capture                            │     │   │
│  │  │  ┌───────────────┐  ┌───────────────┐                       │     │   │
│  │  │  │ScreenCapture  │  │ CGDisplay     │                       │     │   │
│  │  │  │    Kit        │  │   Stream      │                       │     │   │
│  │  │  └───────────────┘  └───────────────┘                       │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    Input Injection                           │     │   │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │     │   │
│  │  │  │  CGEvent    │  │ Accessibility│  │  Coordinate │          │     │   │
│  │  │  │   Post      │  │  Permission  │  │   Mapper    │          │     │   │
│  │  │  └─────────────┘  └─────────────┘  └─────────────┘          │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  │                                                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐     │   │
│  │  │                    System Integration                        │     │   │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │     │   │
│  │  │  │ launchd │  │Keychain │  │Clipboard│  │Permission│        │     │   │
│  │  │  │ Service │  │ Storage │  │  Access │  │ Manager  │        │     │   │
│  │  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │     │   │
│  │  └─────────────────────────────────────────────────────────────┘     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```


## Components and Interfaces

### Screen Capture - Unified Interface

```rust
use screencapturekit::*;
use core_graphics::display::*;

/// macOS screen capture with automatic backend selection
pub struct MacCapturer {
    backend: CaptureBackend,
    displays: Vec<MacDisplayInfo>,
    config: CaptureConfig,
    permission_status: PermissionStatus,
}

enum CaptureBackend {
    ScreenCaptureKit(SckCapturer),
    CGDisplayStream(CgCapturer),
}

impl MacCapturer {
    /// Create capturer with best available backend
    pub fn new(config: CaptureConfig) -> Result<Self, CaptureError> {
        // Check screen recording permission first
        let permission_status = check_screen_recording_permission();
        
        let backend = if SckCapturer::is_available() {
            CaptureBackend::ScreenCaptureKit(SckCapturer::new()?)
        } else {
            CaptureBackend::CGDisplayStream(CgCapturer::new()?)
        };
        
        Ok(Self {
            backend,
            displays: enumerate_displays()?,
            config,
            permission_status,
        })
    }
    
    /// Check and request screen recording permission
    pub fn request_permission(&self) -> PermissionStatus;
}

impl PlatformCapturer for MacCapturer {
    async fn capture_frame(&mut self) -> Result<CapturedFrame, CaptureError>;
    fn supported_formats(&self) -> Vec<FrameFormat>;
    fn set_target_fps(&mut self, fps: u32);
    fn list_monitors(&self) -> &[MonitorInfo];
    fn select_monitor(&mut self, monitor: MonitorId) -> Result<(), CaptureError>;
}
```

### ScreenCaptureKit Capture (macOS 12.3+)

```rust
/// ScreenCaptureKit-based capture
pub struct SckCapturer {
    stream: SCStream,
    stream_config: SCStreamConfiguration,
    content_filter: SCContentFilter,
    frame_receiver: mpsc::Receiver<SckFrame>,
}

impl SckCapturer {
    /// Check if ScreenCaptureKit is available
    pub fn is_available() -> bool {
        // Check macOS version >= 12.3
        if #[cfg(target_os = "macos")] {
            let version = macos_version();
            version >= (12, 3, 0)
        } else {
            false
        }
    }
    
    /// Create SCK capturer for display
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Configure capture parameters
    pub fn configure(&mut self, config: &CaptureConfig) -> Result<(), CaptureError>;
    
    /// Start capture stream
    pub fn start(&mut self) -> Result<(), CaptureError>;
    
    /// Stop capture stream
    pub fn stop(&mut self);
    
    /// Set cursor visibility
    pub fn set_cursor_visible(&mut self, visible: bool);
}

pub struct SckFrame {
    pub surface: IOSurface,
    pub timestamp: CMTime,
    pub display_time: u64,
    pub content_rect: CGRect,
    pub scale_factor: f64,
}
```

### CGDisplayStream Fallback

```rust
/// CGDisplayStream-based capture (legacy)
pub struct CgCapturer {
    display_id: CGDirectDisplayID,
    stream: CGDisplayStream,
    frame_receiver: mpsc::Receiver<CgFrame>,
}

impl CgCapturer {
    /// Create CGDisplayStream capturer
    pub fn new() -> Result<Self, CaptureError>;
    
    /// Start capture
    pub fn start(&mut self) -> Result<(), CaptureError>;
    
    /// Stop capture
    pub fn stop(&mut self);
    
    /// Handle display reconfiguration
    pub fn handle_display_change(&mut self, display_id: CGDirectDisplayID);
}
```

### Input Injection

```rust
use core_graphics::event::*;

/// macOS input injection via CGEvent
pub struct MacInjector {
    event_source: CGEventSource,
    held_keys: HashSet<CGKeyCode>,
    coordinate_mapper: CoordinateMapper,
    accessibility_enabled: bool,
}

impl MacInjector {
    /// Create input injector
    pub fn new() -> Result<Self, InputError> {
        let accessibility_enabled = check_accessibility_permission();
        
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .ok_or(InputError::EventSourceCreationFailed)?;
        
        Ok(Self {
            event_source,
            held_keys: HashSet::new(),
            coordinate_mapper: CoordinateMapper::new()?,
            accessibility_enabled,
        })
    }
    
    /// Check accessibility permission
    pub fn check_permission(&self) -> bool {
        unsafe { AXIsProcessTrusted() }
    }
    
    /// Request accessibility permission
    pub fn request_permission(&self) {
        let options = NSDictionary::dictionaryWithObject_forKey_(
            kCFBooleanTrue,
            kAXTrustedCheckOptionPrompt,
        );
        unsafe { AXIsProcessTrustedWithOptions(options) };
    }
}

impl PlatformInjector for MacInjector {
    fn inject_mouse_move(&mut self, x: i32, y: i32) -> Result<(), InputError> {
        if !self.accessibility_enabled {
            return Err(InputError::PermissionDenied);
        }
        
        let point = self.coordinate_mapper.map(x, y);
        let event = CGEvent::new_mouse_event(
            self.event_source.clone(),
            CGEventType::MouseMoved,
            point,
            CGMouseButton::Left,
        ).ok_or(InputError::EventCreationFailed)?;
        
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
    
    fn inject_mouse_button(&mut self, button: MouseButton, down: bool) -> Result<(), InputError>;
    fn inject_mouse_scroll(&mut self, delta: i32) -> Result<(), InputError>;
    fn inject_key(&mut self, code: KeyCode, down: bool) -> Result<(), InputError>;
    fn inject_text(&mut self, text: &str) -> Result<(), InputError>;
    fn inject_special_sequence(&mut self, seq: SpecialSequence) -> Result<(), InputError>;
}
```

### Coordinate Mapper

```rust
/// Coordinate mapping for macOS (origin at bottom-left)
pub struct CoordinateMapper {
    displays: Vec<DisplayBounds>,
    main_display_height: f64,
}

impl CoordinateMapper {
    /// Map coordinates from top-left origin to macOS bottom-left origin
    pub fn map(&self, x: i32, y: i32) -> CGPoint {
        // macOS uses bottom-left origin, flip Y coordinate
        CGPoint::new(
            x as f64,
            self.main_display_height - y as f64,
        )
    }
    
    /// Handle Retina scaling
    pub fn scale_for_display(&self, display_id: CGDirectDisplayID) -> f64;
}
```

### Permission Manager

```rust
/// macOS permission management
pub struct PermissionManager;

impl PermissionManager {
    /// Check screen recording permission
    pub fn check_screen_recording() -> PermissionStatus {
        // Use CGPreflightScreenCaptureAccess on macOS 10.15+
        // or attempt capture and check result
    }
    
    /// Request screen recording permission
    pub fn request_screen_recording() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }
    
    /// Check accessibility permission
    pub fn check_accessibility() -> bool {
        unsafe { AXIsProcessTrusted() }
    }
    
    /// Request accessibility permission with prompt
    pub fn request_accessibility() -> bool {
        let options = /* ... */;
        unsafe { AXIsProcessTrustedWithOptions(options) }
    }
    
    /// Open System Preferences to relevant pane
    pub fn open_preferences(pane: PreferencePane);
}

pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
}

pub enum PreferencePane {
    ScreenRecording,
    Accessibility,
    Security,
}
```

### Keychain Storage

```rust
use security_framework::keychain::*;

/// macOS Keychain-based key storage
pub struct KeychainStore {
    service_name: String,
    access_group: Option<String>,
}

impl KeychainStore {
    /// Create keychain store
    pub fn new(service_name: &str) -> Self;
    
    /// Set access group for sharing between apps
    pub fn with_access_group(mut self, group: &str) -> Self;
}

impl KeyStore for KeychainStore {
    fn store_key(&self, name: &str, key: &[u8]) -> Result<(), KeyStoreError> {
        let mut item = SecKeychainItem::new();
        item.set_service(&self.service_name);
        item.set_account(name);
        item.set_data(key);
        item.set_accessible(SecAccessible::WhenUnlockedThisDeviceOnly);
        
        // Disable iCloud sync for device keys
        item.set_synchronizable(false);
        
        item.add()?;
        Ok(())
    }
    
    fn load_key(&self, name: &str) -> Result<Vec<u8>, KeyStoreError>;
    fn delete_key(&self, name: &str) -> Result<(), KeyStoreError>;
    fn key_exists(&self, name: &str) -> bool;
}
```

### launchd Integration

```rust
/// launchd service integration
pub struct LaunchdService {
    label: String,
    plist_path: PathBuf,
}

impl LaunchdService {
    /// Generate LaunchAgent plist
    pub fn generate_agent_plist(&self, config: &AgentConfig) -> String;
    
    /// Generate LaunchDaemon plist
    pub fn generate_daemon_plist(&self, config: &DaemonConfig) -> String;
    
    /// Install service
    pub fn install(&self, mode: ServiceMode) -> Result<(), ServiceError>;
    
    /// Uninstall service
    pub fn uninstall(&self) -> Result<(), ServiceError>;
    
    /// Start service
    pub fn start(&self) -> Result<(), ServiceError>;
    
    /// Stop service
    pub fn stop(&self) -> Result<(), ServiceError>;
    
    /// Check if service is running
    pub fn is_running(&self) -> bool;
}

pub enum ServiceMode {
    Agent,  // User context, ~/Library/LaunchAgents
    Daemon, // System context, /Library/LaunchDaemons
}
```

### Clipboard Access

```rust
use cocoa::appkit::NSPasteboard;

/// macOS clipboard access
pub struct MacClipboard {
    pasteboard: NSPasteboard,
    last_change_count: i64,
}

impl MacClipboard {
    /// Read text from clipboard
    pub fn read_text(&self) -> Result<Option<String>, ClipboardError>;
    
    /// Read image from clipboard
    pub fn read_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;
    
    /// Write text to clipboard
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError>;
    
    /// Write image to clipboard
    pub fn write_image(&self, image: &ClipboardImage) -> Result<(), ClipboardError>;
    
    /// Check for clipboard changes
    pub fn has_changed(&mut self) -> bool {
        let current = self.pasteboard.changeCount();
        if current != self.last_change_count {
            self.last_change_count = current;
            true
        } else {
            false
        }
    }
}
```

## Data Models

### Display Information

```rust
pub struct MacDisplayInfo {
    pub display_id: CGDirectDisplayID,
    pub name: String,
    pub bounds: CGRect,
    pub is_main: bool,
    pub is_builtin: bool,
    pub scale_factor: f64,  // Retina scaling
    pub refresh_rate: f64,
    pub color_space: CGColorSpace,
}
```

### LaunchAgent Plist Template

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.zippyremote.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/zrc-agent</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/zrc-agent.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/zrc-agent.error.log</string>
</dict>
</plist>
```

## Correctness Properties

### Property 1: Permission Check Before Capture
*For any* capture attempt, the system SHALL verify screen recording permission is granted before proceeding.
**Validates: Requirements 1.3, 1.4, 2.3**

### Property 2: Accessibility Permission for Input
*For any* input injection attempt, the system SHALL verify accessibility permission is granted and return PermissionDenied if not.
**Validates: Requirements 6.1, 6.2**

### Property 3: Coordinate System Conversion
*For any* input coordinate, the Y-axis SHALL be correctly flipped from top-left origin to macOS bottom-left origin.
**Validates: Requirement 4.4**

### Property 4: Retina Scaling Accuracy
*For any* display with scale_factor > 1.0, coordinates and frame dimensions SHALL be correctly scaled.
**Validates: Requirements 1.8, 4.6**

### Property 5: Keychain Isolation
*For any* key stored with synchronizable=false, the key SHALL NOT sync to iCloud Keychain.
**Validates: Requirement 8.7**

### Property 6: Backend Fallback
*When* ScreenCaptureKit is unavailable (macOS < 12.3), the system SHALL fall back to CGDisplayStream.
**Validates: Requirements 1.2, 2.1**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| Screen recording denied | Return PermissionDenied | Guide to System Preferences |
| Accessibility denied | Disable input, return error | Guide to System Preferences |
| Keychain locked | Prompt for unlock | Retry after unlock |
| Display disconnected | Update display list | Continue with remaining |
| CGEvent creation failed | Log error | Skip event |
| launchd communication failed | Log error | Manual restart required |

## Testing Strategy

### Unit Tests
- Coordinate mapping calculations
- Permission status parsing
- Keychain operations (mock)
- Plist generation

### Integration Tests
- Full capture pipeline
- Input injection (requires accessibility)
- Clipboard read/write
- launchd service lifecycle

### Platform Tests
- macOS 12 vs 13 vs 14 behavior
- Intel vs Apple Silicon
- Retina vs non-Retina displays
- Multiple display configurations
