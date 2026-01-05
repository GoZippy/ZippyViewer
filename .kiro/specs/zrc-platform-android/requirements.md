# Requirements Document: zrc-platform-android

## Introduction

The zrc-platform-android crate implements Android-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides the controller application for viewing and controlling remote devices from Android phones and tablets, with optional host capabilities for attended access scenarios.

## Glossary

- **MediaProjection**: Android API for screen capture
- **AccessibilityService**: Android service for input injection (host mode)
- **SurfaceView**: Android view for efficient frame rendering
- **JNI**: Java Native Interface for Rust integration
- **Foreground_Service**: Android service type for persistent operations
- **Intent**: Android mechanism for inter-component communication
- **Keystore**: Android secure key storage system

## Requirements

### Requirement 1: Controller - Frame Rendering

**User Story:** As a mobile operator, I want to view remote screens, so that I can see what's happening on remote devices.

#### Acceptance Criteria

1. THE Android_Controller SHALL render received frames using SurfaceView or TextureView
2. THE Android_Controller SHALL support hardware-accelerated rendering
3. THE Android_Controller SHALL handle frame scaling for different screen sizes
4. THE Android_Controller SHALL support pinch-to-zoom for detailed viewing
5. THE Android_Controller SHALL support landscape and portrait orientations
6. THE Android_Controller SHALL maintain aspect ratio during scaling
7. THE Android_Controller SHALL achieve smooth 30+ fps rendering
8. THE Android_Controller SHALL handle frame format conversion efficiently

### Requirement 2: Controller - Touch Input

**User Story:** As a mobile operator, I want to control remote devices via touch, so that I can interact naturally.

#### Acceptance Criteria

1. THE Android_Controller SHALL map touch events to mouse events
2. THE Android_Controller SHALL support tap for click
3. THE Android_Controller SHALL support long-press for right-click
4. THE Android_Controller SHALL support drag for mouse movement
5. THE Android_Controller SHALL support two-finger scroll
6. THE Android_Controller SHALL support pinch gestures for zoom (local zoom)
7. THE Android_Controller SHALL provide visual feedback for touch actions
8. THE Android_Controller SHALL support configurable touch-to-mouse mapping

### Requirement 3: Controller - Keyboard Input

**User Story:** As a mobile operator, I want to type on remote devices, so that I can enter text and commands.

#### Acceptance Criteria

1. THE Android_Controller SHALL show soft keyboard on demand
2. THE Android_Controller SHALL send keyboard events to remote device
3. THE Android_Controller SHALL support special keys toolbar (Ctrl, Alt, etc.)
4. THE Android_Controller SHALL support hardware keyboard when connected
5. THE Android_Controller SHALL handle IME input correctly
6. THE Android_Controller SHALL support keyboard shortcuts via toolbar
7. THE Android_Controller SHALL support function keys (F1-F12)
8. THE Android_Controller SHALL support Ctrl+Alt+Del via menu

### Requirement 4: Controller - Connection Management

**User Story:** As a mobile operator, I want reliable connections, so that sessions remain stable.

#### Acceptance Criteria

1. THE Android_Controller SHALL maintain QUIC connection in foreground
2. THE Android_Controller SHALL handle network changes (WiFi to cellular)
3. THE Android_Controller SHALL support connection migration
4. THE Android_Controller SHALL show connection status indicator
5. THE Android_Controller SHALL attempt automatic reconnection on disconnect
6. THE Android_Controller SHALL handle app backgrounding gracefully
7. THE Android_Controller SHALL support foreground service for persistent sessions
8. THE Android_Controller SHALL respect battery optimization settings

### Requirement 5: Pairing and Device Management

**User Story:** As a mobile operator, I want to manage device pairings, so that I can access my devices.

#### Acceptance Criteria

1. THE Android_Controller SHALL support QR code scanning for invite import
2. THE Android_Controller SHALL support clipboard paste for invite import
3. THE Android_Controller SHALL display paired device list
4. THE Android_Controller SHALL show device online/offline status
5. THE Android_Controller SHALL support device grouping
6. THE Android_Controller SHALL support pairing removal
7. THE Android_Controller SHALL sync pairings across app reinstalls (optional)
8. THE Android_Controller SHALL support SAS verification display

### Requirement 6: Clipboard Synchronization

**User Story:** As a mobile operator, I want clipboard sync, so that I can copy/paste with remote devices.

#### Acceptance Criteria

1. THE Android_Controller SHALL read local clipboard content
2. THE Android_Controller SHALL send clipboard to remote device
3. THE Android_Controller SHALL receive clipboard from remote device
4. THE Android_Controller SHALL set local clipboard from remote
5. THE Android_Controller SHALL support text clipboard format
6. THE Android_Controller SHALL handle clipboard permission (Android 10+)
7. THE Android_Controller SHALL provide clipboard sync toggle
8. THE Android_Controller SHALL indicate clipboard sync status

### Requirement 7: Host Mode - Screen Capture (Optional)

**User Story:** As a device owner, I want to share my Android screen, so that others can view/assist.

#### Acceptance Criteria

1. THE Android_Host SHALL capture screen using MediaProjection API
2. THE Android_Host SHALL request user permission for screen capture
3. THE Android_Host SHALL run as foreground service during capture
4. THE Android_Host SHALL show persistent notification during sharing
5. THE Android_Host SHALL handle screen rotation
6. THE Android_Host SHALL support configurable capture quality
7. THE Android_Host SHALL stop capture when permission revoked
8. THE Android_Host SHALL exclude sensitive content (optional)

### Requirement 8: Host Mode - Input Injection (Optional)

**User Story:** As a device owner, I want remote control of my Android, so that others can assist me.

#### Acceptance Criteria

1. THE Android_Host SHALL inject input via AccessibilityService
2. THE Android_Host SHALL require explicit user enablement of accessibility
3. THE Android_Host SHALL support tap, swipe, and scroll gestures
4. THE Android_Host SHALL support text input
5. THE Android_Host SHALL handle accessibility permission changes
6. THE Android_Host SHALL provide clear setup instructions
7. THE Android_Host SHALL indicate when input injection is active
8. THE Android_Host SHALL support disabling input (view-only mode)

### Requirement 9: Secure Key Storage

**User Story:** As a user, I want secure key storage, so that my identity is protected.

#### Acceptance Criteria

1. THE Android_Platform SHALL store keys in Android Keystore
2. THE Android_Platform SHALL use hardware-backed keystore when available
3. THE Android_Platform SHALL require user authentication for key access (optional)
4. THE Android_Platform SHALL handle keystore unavailability
5. THE Android_Platform SHALL support key backup exclusion
6. THE Android_Platform SHALL zeroize key material in memory
7. THE Android_Platform SHALL detect rooted devices (warning only)
8. THE Android_Platform SHALL support biometric authentication for key access

### Requirement 10: Network and Transport

**User Story:** As a user, I want reliable networking, so that connections work across network conditions.

#### Acceptance Criteria

1. THE Android_Platform SHALL support QUIC transport
2. THE Android_Platform SHALL handle network type detection (WiFi, cellular)
3. THE Android_Platform SHALL respect data saver settings
4. THE Android_Platform SHALL support VPN compatibility
5. THE Android_Platform SHALL handle IPv4 and IPv6
6. THE Android_Platform SHALL support connection over cellular data
7. THE Android_Platform SHALL provide network quality indicators
8. THE Android_Platform SHALL handle airplane mode transitions

### Requirement 11: UI/UX Requirements

**User Story:** As a user, I want a polished mobile experience, so that the app is pleasant to use.

#### Acceptance Criteria

1. THE Android_App SHALL follow Material Design guidelines
2. THE Android_App SHALL support dark and light themes
3. THE Android_App SHALL support system theme following
4. THE Android_App SHALL be accessible (TalkBack support)
5. THE Android_App SHALL support multiple screen sizes (phone, tablet)
6. THE Android_App SHALL provide haptic feedback for actions
7. THE Android_App SHALL support edge-to-edge display
8. THE Android_App SHALL handle notch/cutout displays

### Requirement 12: JNI Integration

**User Story:** As a developer, I want clean Rust integration, so that core logic is shared.

#### Acceptance Criteria

1. THE Android_Platform SHALL use JNI for Rust-Kotlin bridge
2. THE Android_Platform SHALL minimize JNI boundary crossings
3. THE Android_Platform SHALL handle JNI exceptions properly
4. THE Android_Platform SHALL support async operations across JNI
5. THE Android_Platform SHALL manage native memory correctly
6. THE Android_Platform SHALL provide Kotlin-friendly API wrapper
7. THE Android_Platform SHALL support ProGuard/R8 optimization
8. THE Android_Platform SHALL include native libraries for arm64-v8a and x86_64
