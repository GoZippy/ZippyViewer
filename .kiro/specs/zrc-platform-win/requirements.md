# Requirements Document: zrc-platform-win

## Introduction

The zrc-platform-win crate implements Windows-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture, input injection, and system integration for Windows platforms. It serves as the platform abstraction layer used by the agent on Windows hosts.

## Glossary

- **DXGI**: DirectX Graphics Infrastructure, used for efficient screen capture
- **WGC**: Windows Graphics Capture, modern capture API for Windows 10+
- **GDI**: Graphics Device Interface, legacy capture fallback
- **SendInput**: Windows API for injecting mouse and keyboard input
- **Desktop_Duplication**: DXGI API for capturing desktop frames
- **UAC**: User Account Control, Windows elevation mechanism
- **Secure_Desktop**: Special desktop for UAC prompts and login screen
- **DPAPI**: Data Protection API for secure key storage

## Requirements

### Requirement 1: Screen Capture - GDI Fallback

**User Story:** As a developer, I want GDI capture as fallback, so that screen capture works on all Windows versions.

#### Acceptance Criteria

1. THE Win_Capture SHALL implement GDI-based screen capture using BitBlt
2. THE Win_Capture SHALL capture the primary display by default
3. THE Win_Capture SHALL support capturing specific monitors by index
4. THE Win_Capture SHALL return frames in BGRA format
5. THE Win_Capture SHALL support configurable capture resolution (native or scaled)
6. THE Win_Capture SHALL handle display resolution changes
7. THE Win_Capture SHALL achieve minimum 15 fps on modern hardware
8. THE Win_Capture SHALL release GDI resources properly to prevent leaks

### Requirement 2: Screen Capture - DXGI Desktop Duplication

**User Story:** As a developer, I want DXGI capture, so that screen capture is efficient and low-latency.

#### Acceptance Criteria

1. THE Win_Capture SHALL implement DXGI Desktop Duplication API capture
2. THE Win_Capture SHALL detect DXGI availability and fall back to GDI if unavailable
3. THE Win_Capture SHALL capture dirty rectangles for efficient encoding
4. THE Win_Capture SHALL support GPU-side frame access for zero-copy encoding
5. THE Win_Capture SHALL handle desktop switch events (lock screen, UAC)
6. THE Win_Capture SHALL achieve 60 fps capture on supported hardware
7. THE Win_Capture SHALL handle adapter/output enumeration for multi-GPU systems
8. THE Win_Capture SHALL recover from device lost errors

### Requirement 3: Screen Capture - Windows Graphics Capture

**User Story:** As a developer, I want WGC capture, so that modern Windows 10/11 features are supported.

#### Acceptance Criteria

1. THE Win_Capture SHALL implement Windows Graphics Capture API (Windows 10 1903+)
2. THE Win_Capture SHALL detect WGC availability via feature check
3. THE Win_Capture SHALL support capturing specific windows (future feature)
4. THE Win_Capture SHALL support cursor capture toggle
5. THE Win_Capture SHALL support border highlight toggle
6. THE Win_Capture SHALL handle DPI scaling correctly
7. THE Win_Capture SHALL support HDR content capture where available
8. THE Win_Capture SHALL prefer WGC over DXGI when both available (configurable)

### Requirement 4: Multi-Monitor Support

**User Story:** As a developer, I want multi-monitor capture, so that all displays can be accessed remotely.

#### Acceptance Criteria

1. THE Win_Capture SHALL enumerate all connected monitors
2. THE Win_Capture SHALL provide monitor metadata: name, resolution, position, primary flag
3. THE Win_Capture SHALL support capturing individual monitors
4. THE Win_Capture SHALL support capturing virtual desktop (all monitors combined)
5. THE Win_Capture SHALL handle monitor add/remove events
6. THE Win_Capture SHALL handle resolution and DPI changes per monitor
7. THE Win_Capture SHALL map coordinates correctly across monitor boundaries
8. THE Win_Capture SHALL detect and report monitor configuration changes

### Requirement 5: Input Injection - Mouse

**User Story:** As a developer, I want mouse input injection, so that remote operators can control the mouse.

#### Acceptance Criteria

1. THE Win_Input SHALL inject mouse move events using SendInput
2. THE Win_Input SHALL inject mouse button events (left, right, middle, X1, X2)
3. THE Win_Input SHALL inject mouse scroll events (vertical and horizontal)
4. THE Win_Input SHALL support absolute positioning mode
5. THE Win_Input SHALL clamp coordinates to valid screen bounds
6. THE Win_Input SHALL handle multi-monitor coordinate mapping
7. THE Win_Input SHALL support high-precision mouse movement
8. THE Win_Input SHALL handle DPI scaling for coordinate translation

### Requirement 6: Input Injection - Keyboard

**User Story:** As a developer, I want keyboard input injection, so that remote operators can type and use shortcuts.

#### Acceptance Criteria

1. THE Win_Input SHALL inject key down and key up events using SendInput
2. THE Win_Input SHALL support virtual key codes (VK_*)
3. THE Win_Input SHALL support scan codes for hardware-level injection
4. THE Win_Input SHALL support modifier keys (Shift, Ctrl, Alt, Win)
5. THE Win_Input SHALL support Unicode character input via KEYEVENTF_UNICODE
6. THE Win_Input SHALL handle extended keys correctly (arrows, numpad, etc.)
7. THE Win_Input SHALL implement key release on session end (prevent stuck keys)
8. THE Win_Input SHALL support keyboard layout detection

### Requirement 7: Special Key Sequences

**User Story:** As a developer, I want special key handling, so that secure attention sequences work remotely.

#### Acceptance Criteria

1. THE Win_Input SHALL support Ctrl+Alt+Del injection (requires service context)
2. THE Win_Input SHALL support Win+L (lock workstation) handling
3. THE Win_Input SHALL support Alt+Tab injection
4. THE Win_Input SHALL support Ctrl+Shift+Esc (Task Manager)
5. THE Win_Input SHALL detect when running in service context for elevated injection
6. THE Win_Input SHALL fall back to simulated sequences when not elevated
7. THE Win_Input SHALL log special key sequence attempts for audit
8. THE Win_Input SHALL support configurable special key behavior

### Requirement 8: UAC and Elevation Handling

**User Story:** As a developer, I want UAC handling, so that remote sessions can interact with elevated prompts.

#### Acceptance Criteria

1. THE Win_Platform SHALL detect UAC prompt display (secure desktop switch)
2. THE Win_Platform SHALL support capturing secure desktop when running as SYSTEM
3. THE Win_Platform SHALL support input injection on secure desktop when elevated
4. THE Win_Platform SHALL notify session of desktop switch events
5. THE Win_Platform SHALL handle consent.exe interaction
6. THE Win_Platform SHALL support "over-the-shoulder" elevation scenarios
7. THE Win_Platform SHALL log UAC interaction attempts for audit
8. IF not running elevated, THEN THE Win_Platform SHALL indicate UAC limitation to operator

### Requirement 9: Windows Service Integration

**User Story:** As a developer, I want service integration, so that the agent runs reliably as a Windows service.

#### Acceptance Criteria

1. THE Win_Platform SHALL implement Windows Service control handler
2. THE Win_Platform SHALL support service start, stop, pause, continue
3. THE Win_Platform SHALL report service status to SCM
4. THE Win_Platform SHALL handle session 0 isolation
5. THE Win_Platform SHALL support interactive service detection
6. THE Win_Platform SHALL implement service recovery options
7. THE Win_Platform SHALL support delayed auto-start
8. THE Win_Platform SHALL log service lifecycle events to Event Log

### Requirement 10: Secure Key Storage

**User Story:** As a developer, I want secure key storage, so that cryptographic keys are protected.

#### Acceptance Criteria

1. THE Win_Platform SHALL store private keys using DPAPI
2. THE Win_Platform SHALL use machine-scope protection for service keys
3. THE Win_Platform SHALL use user-scope protection for user keys
4. THE Win_Platform SHALL support key export with password protection
5. THE Win_Platform SHALL implement key access audit logging
6. THE Win_Platform SHALL handle key migration on user profile changes
7. THE Win_Platform SHALL support Windows Credential Manager integration
8. THE Win_Platform SHALL zeroize key material after use

### Requirement 11: Clipboard Access

**User Story:** As a developer, I want clipboard access, so that clipboard sync works on Windows.

#### Acceptance Criteria

1. THE Win_Platform SHALL read clipboard content in text format (CF_UNICODETEXT)
2. THE Win_Platform SHALL read clipboard content in image format (CF_DIB, CF_DIBV5)
3. THE Win_Platform SHALL write clipboard content in supported formats
4. THE Win_Platform SHALL detect clipboard changes via clipboard viewer chain
5. THE Win_Platform SHALL handle clipboard access failures gracefully
6. THE Win_Platform SHALL support clipboard format conversion
7. THE Win_Platform SHALL enforce clipboard size limits
8. THE Win_Platform SHALL handle clipboard owner changes

### Requirement 12: System Information

**User Story:** As a developer, I want system information access, so that device details can be reported.

#### Acceptance Criteria

1. THE Win_Platform SHALL report Windows version and build number
2. THE Win_Platform SHALL report computer name and domain
3. THE Win_Platform SHALL report logged-in user (if any)
4. THE Win_Platform SHALL report display configuration
5. THE Win_Platform SHALL report network adapter information
6. THE Win_Platform SHALL detect remote desktop session status
7. THE Win_Platform SHALL detect virtual machine environment
8. THE Win_Platform SHALL report system uptime and idle time
