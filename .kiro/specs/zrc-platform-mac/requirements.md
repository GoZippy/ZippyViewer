# Requirements Document: zrc-platform-mac

## Introduction

The zrc-platform-mac crate implements macOS-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture, input injection, and system integration for macOS platforms. It handles the unique permission model and security requirements of macOS.

## Glossary

- **ScreenCaptureKit**: Modern macOS API for screen capture (macOS 12.3+)
- **CGDisplayStream**: Legacy Core Graphics API for screen capture
- **CGEvent**: Core Graphics event API for input injection
- **Accessibility_API**: macOS API requiring user permission for input control
- **Keychain**: macOS secure storage for cryptographic keys
- **LaunchAgent**: User-level background process mechanism
- **LaunchDaemon**: System-level background process mechanism
- **Notarization**: Apple's code signing verification for distribution

## Requirements

### Requirement 1: Screen Capture - ScreenCaptureKit

**User Story:** As a developer, I want ScreenCaptureKit capture, so that screen capture is efficient on modern macOS.

#### Acceptance Criteria

1. THE Mac_Capture SHALL implement ScreenCaptureKit-based capture (macOS 12.3+)
2. THE Mac_Capture SHALL detect ScreenCaptureKit availability via runtime check
3. THE Mac_Capture SHALL request screen recording permission from user
4. THE Mac_Capture SHALL handle permission denial gracefully with user guidance
5. THE Mac_Capture SHALL support capturing specific displays
6. THE Mac_Capture SHALL support capturing at native or scaled resolution
7. THE Mac_Capture SHALL achieve 60 fps capture on supported hardware
8. THE Mac_Capture SHALL handle Retina display scaling correctly

### Requirement 2: Screen Capture - CGDisplayStream Fallback

**User Story:** As a developer, I want CGDisplayStream fallback, so that capture works on older macOS versions.

#### Acceptance Criteria

1. THE Mac_Capture SHALL implement CGDisplayStream-based capture as fallback
2. THE Mac_Capture SHALL use CGDisplayStream on macOS < 12.3
3. THE Mac_Capture SHALL request screen recording permission
4. THE Mac_Capture SHALL return frames in compatible pixel format
5. THE Mac_Capture SHALL handle display configuration changes
6. THE Mac_Capture SHALL achieve minimum 30 fps on supported hardware
7. THE Mac_Capture SHALL properly release Core Graphics resources
8. THE Mac_Capture SHALL handle display sleep/wake events

### Requirement 3: Multi-Display Support

**User Story:** As a developer, I want multi-display capture, so that all connected displays can be accessed.

#### Acceptance Criteria

1. THE Mac_Capture SHALL enumerate all connected displays via CGGetActiveDisplayList
2. THE Mac_Capture SHALL provide display metadata: name, resolution, position, main flag
3. THE Mac_Capture SHALL support capturing individual displays
4. THE Mac_Capture SHALL handle display arrangement (Spaces, Mission Control)
5. THE Mac_Capture SHALL detect display add/remove events
6. THE Mac_Capture SHALL handle resolution and scaling changes
7. THE Mac_Capture SHALL support external displays and AirPlay displays
8. THE Mac_Capture SHALL handle display mirroring configuration

### Requirement 4: Input Injection - Mouse

**User Story:** As a developer, I want mouse input injection, so that remote operators can control the mouse.

#### Acceptance Criteria

1. THE Mac_Input SHALL inject mouse events using CGEventPost
2. THE Mac_Input SHALL support mouse move, click, and scroll events
3. THE Mac_Input SHALL support all mouse buttons (left, right, other)
4. THE Mac_Input SHALL handle coordinate system (origin at bottom-left)
5. THE Mac_Input SHALL support multi-touch trackpad gestures (future)
6. THE Mac_Input SHALL handle Retina display coordinate scaling
7. THE Mac_Input SHALL clamp coordinates to valid screen bounds
8. THE Mac_Input SHALL support smooth scrolling events

### Requirement 5: Input Injection - Keyboard

**User Story:** As a developer, I want keyboard input injection, so that remote operators can type and use shortcuts.

#### Acceptance Criteria

1. THE Mac_Input SHALL inject keyboard events using CGEventPost
2. THE Mac_Input SHALL support key down and key up events
3. THE Mac_Input SHALL support modifier keys (Shift, Control, Option, Command)
4. THE Mac_Input SHALL support Unicode character input
5. THE Mac_Input SHALL handle keyboard layout detection
6. THE Mac_Input SHALL implement key release on session end
7. THE Mac_Input SHALL support function keys and media keys
8. THE Mac_Input SHALL handle dead keys and input methods

### Requirement 6: Accessibility Permission

**User Story:** As a developer, I want accessibility permission handling, so that input injection is properly authorized.

#### Acceptance Criteria

1. THE Mac_Platform SHALL check Accessibility permission status
2. THE Mac_Platform SHALL request Accessibility permission via system prompt
3. THE Mac_Platform SHALL guide user to System Preferences if permission denied
4. THE Mac_Platform SHALL detect permission changes at runtime
5. THE Mac_Platform SHALL disable input injection when permission not granted
6. THE Mac_Platform SHALL provide clear error messages for permission issues
7. THE Mac_Platform SHALL support programmatic permission check via AXIsProcessTrusted
8. THE Mac_Platform SHALL handle permission revocation gracefully

### Requirement 7: Screen Recording Permission

**User Story:** As a developer, I want screen recording permission handling, so that capture is properly authorized.

#### Acceptance Criteria

1. THE Mac_Platform SHALL check Screen Recording permission status
2. THE Mac_Platform SHALL trigger permission prompt on first capture attempt
3. THE Mac_Platform SHALL guide user to System Preferences if permission denied
4. THE Mac_Platform SHALL detect permission changes (requires app restart on older macOS)
5. THE Mac_Platform SHALL provide clear status indication for permission state
6. THE Mac_Platform SHALL handle permission in sandboxed and non-sandboxed contexts
7. THE Mac_Platform SHALL support permission pre-flight check
8. THE Mac_Platform SHALL log permission status for troubleshooting

### Requirement 8: Keychain Integration

**User Story:** As a developer, I want Keychain storage, so that cryptographic keys are securely stored.

#### Acceptance Criteria

1. THE Mac_Platform SHALL store private keys in macOS Keychain
2. THE Mac_Platform SHALL use appropriate access control (kSecAttrAccessible)
3. THE Mac_Platform SHALL support Keychain access prompts
4. THE Mac_Platform SHALL handle Keychain locked state
5. THE Mac_Platform SHALL support key export with password protection
6. THE Mac_Platform SHALL implement key access audit via Keychain Access
7. THE Mac_Platform SHALL handle iCloud Keychain sync (disable for device keys)
8. THE Mac_Platform SHALL zeroize key material after retrieval

### Requirement 9: LaunchAgent/Daemon Integration

**User Story:** As a developer, I want launch service integration, so that the agent runs reliably in background.

#### Acceptance Criteria

1. THE Mac_Platform SHALL support running as LaunchAgent (user context)
2. THE Mac_Platform SHALL support running as LaunchDaemon (system context)
3. THE Mac_Platform SHALL provide plist templates for both modes
4. THE Mac_Platform SHALL handle service start, stop, and restart
5. THE Mac_Platform SHALL support KeepAlive and automatic restart
6. THE Mac_Platform SHALL handle user login/logout events
7. THE Mac_Platform SHALL support running at login
8. THE Mac_Platform SHALL log to system log (os_log)

### Requirement 10: Clipboard Access

**User Story:** As a developer, I want clipboard access, so that clipboard sync works on macOS.

#### Acceptance Criteria

1. THE Mac_Platform SHALL read clipboard via NSPasteboard
2. THE Mac_Platform SHALL support text (NSPasteboardTypeString)
3. THE Mac_Platform SHALL support images (NSPasteboardTypePNG, TIFF)
4. THE Mac_Platform SHALL write clipboard content
5. THE Mac_Platform SHALL detect clipboard changes via change count
6. THE Mac_Platform SHALL handle clipboard access in sandboxed apps
7. THE Mac_Platform SHALL support rich text format (optional)
8. THE Mac_Platform SHALL enforce clipboard size limits

### Requirement 11: System Information

**User Story:** As a developer, I want system information access, so that device details can be reported.

#### Acceptance Criteria

1. THE Mac_Platform SHALL report macOS version and build
2. THE Mac_Platform SHALL report computer name
3. THE Mac_Platform SHALL report logged-in user
4. THE Mac_Platform SHALL report display configuration
5. THE Mac_Platform SHALL report hardware model identifier
6. THE Mac_Platform SHALL detect Apple Silicon vs Intel
7. THE Mac_Platform SHALL report network interface information
8. THE Mac_Platform SHALL detect virtual machine environment

### Requirement 12: Code Signing and Notarization

**User Story:** As a developer, I want proper code signing, so that the app runs without Gatekeeper issues.

#### Acceptance Criteria

1. THE Mac_Platform SHALL support Developer ID code signing
2. THE Mac_Platform SHALL support hardened runtime
3. THE Mac_Platform SHALL declare required entitlements
4. THE Mac_Platform SHALL support notarization workflow
5. THE Mac_Platform SHALL handle quarantine attribute
6. THE Mac_Platform SHALL support stapled notarization ticket
7. THE Mac_Platform SHALL provide signing configuration documentation
8. THE Mac_Platform SHALL validate signature at runtime (optional)
