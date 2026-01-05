# Requirements Document: zrc-platform-ios

## Introduction

The zrc-platform-ios crate implements iOS-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides the controller application for viewing and controlling remote devices from iPhones and iPads. Due to iOS platform restrictions, host/screen sharing capabilities are limited to ReplayKit broadcast extensions.

## Glossary

- **ReplayKit**: iOS framework for screen recording and broadcast
- **Metal**: Apple's low-level graphics API for efficient rendering
- **UniFFI**: Mozilla's tool for generating Rust FFI bindings
- **Keychain**: iOS secure storage for cryptographic keys
- **App_Extension**: iOS mechanism for extending app functionality
- **Network_Extension**: iOS framework for VPN and network features
- **Broadcast_Extension**: ReplayKit extension for screen sharing

## Requirements

### Requirement 1: Controller - Frame Rendering

**User Story:** As a mobile operator, I want to view remote screens, so that I can see remote devices clearly.

#### Acceptance Criteria

1. THE iOS_Controller SHALL render frames using Metal for GPU acceleration
2. THE iOS_Controller SHALL support MTKView for efficient frame display
3. THE iOS_Controller SHALL handle frame scaling for different iOS devices
4. THE iOS_Controller SHALL support pinch-to-zoom for detailed viewing
5. THE iOS_Controller SHALL support landscape and portrait orientations
6. THE iOS_Controller SHALL maintain aspect ratio during scaling
7. THE iOS_Controller SHALL achieve smooth 60 fps rendering on modern devices
8. THE iOS_Controller SHALL handle ProMotion displays (120Hz)

### Requirement 2: Controller - Touch Input

**User Story:** As a mobile operator, I want intuitive touch controls, so that I can control remote devices naturally.

#### Acceptance Criteria

1. THE iOS_Controller SHALL map touch events to mouse events
2. THE iOS_Controller SHALL support tap for left-click
3. THE iOS_Controller SHALL support long-press for right-click
4. THE iOS_Controller SHALL support drag for mouse movement
5. THE iOS_Controller SHALL support two-finger scroll
6. THE iOS_Controller SHALL support three-finger drag (optional)
7. THE iOS_Controller SHALL provide haptic feedback for actions
8. THE iOS_Controller SHALL support Apple Pencil for precise input

### Requirement 3: Controller - Keyboard Input

**User Story:** As a mobile operator, I want keyboard input, so that I can type on remote devices.

#### Acceptance Criteria

1. THE iOS_Controller SHALL show iOS keyboard on demand
2. THE iOS_Controller SHALL send keyboard events to remote device
3. THE iOS_Controller SHALL provide special keys toolbar (Ctrl, Alt, Cmd, etc.)
4. THE iOS_Controller SHALL support hardware keyboard (iPad, Bluetooth)
5. THE iOS_Controller SHALL handle keyboard shortcuts from hardware keyboard
6. THE iOS_Controller SHALL support function keys via toolbar
7. THE iOS_Controller SHALL support Ctrl+Alt+Del via menu
8. THE iOS_Controller SHALL handle keyboard language switching

### Requirement 4: Controller - Connection Management

**User Story:** As a mobile operator, I want stable connections, so that sessions don't drop unexpectedly.

#### Acceptance Criteria

1. THE iOS_Controller SHALL maintain QUIC connection while app is active
2. THE iOS_Controller SHALL handle network transitions (WiFi to cellular)
3. THE iOS_Controller SHALL support connection migration
4. THE iOS_Controller SHALL show connection status in UI
5. THE iOS_Controller SHALL attempt reconnection on network change
6. THE iOS_Controller SHALL handle app backgrounding (limited by iOS)
7. THE iOS_Controller SHALL use background task for graceful disconnect
8. THE iOS_Controller SHALL support Picture-in-Picture mode (iPad)

### Requirement 5: Pairing and Device Management

**User Story:** As a mobile operator, I want to manage my devices, so that I can connect to them easily.

#### Acceptance Criteria

1. THE iOS_Controller SHALL support QR code scanning for invite import
2. THE iOS_Controller SHALL support clipboard paste for invite import
3. THE iOS_Controller SHALL support file import for invites
4. THE iOS_Controller SHALL display paired device list
5. THE iOS_Controller SHALL show device online/offline status
6. THE iOS_Controller SHALL support device grouping and favorites
7. THE iOS_Controller SHALL support pairing removal with confirmation
8. THE iOS_Controller SHALL display SAS verification code

### Requirement 6: Clipboard Synchronization

**User Story:** As a mobile operator, I want clipboard sync, so that I can share text between devices.

#### Acceptance Criteria

1. THE iOS_Controller SHALL read local clipboard (UIPasteboard)
2. THE iOS_Controller SHALL send clipboard to remote device
3. THE iOS_Controller SHALL receive clipboard from remote device
4. THE iOS_Controller SHALL set local clipboard from remote
5. THE iOS_Controller SHALL support text clipboard format
6. THE iOS_Controller SHALL support image clipboard format
7. THE iOS_Controller SHALL provide clipboard sync toggle
8. THE iOS_Controller SHALL handle Universal Clipboard (Handoff)

### Requirement 7: Screen Sharing - Broadcast Extension (Limited Host)

**User Story:** As an iOS user, I want to share my screen, so that others can see what I'm doing.

#### Acceptance Criteria

1. THE iOS_Host SHALL implement ReplayKit Broadcast Extension
2. THE iOS_Host SHALL appear in iOS screen broadcast picker
3. THE iOS_Host SHALL capture screen via RPScreenRecorder
4. THE iOS_Host SHALL stream captured frames to connected controller
5. THE iOS_Host SHALL handle broadcast start/stop lifecycle
6. THE iOS_Host SHALL show broadcast indicator (iOS system UI)
7. THE iOS_Host SHALL handle app extension memory limits
8. THE iOS_Host SHALL NOT support remote input injection (iOS limitation)

### Requirement 8: Secure Key Storage

**User Story:** As a user, I want secure key storage, so that my identity is protected.

#### Acceptance Criteria

1. THE iOS_Platform SHALL store keys in iOS Keychain
2. THE iOS_Platform SHALL use Secure Enclave when available
3. THE iOS_Platform SHALL set appropriate access control (kSecAttrAccessible)
4. THE iOS_Platform SHALL support biometric authentication for key access
5. THE iOS_Platform SHALL handle Keychain access errors
6. THE iOS_Platform SHALL exclude keys from iCloud Keychain sync
7. THE iOS_Platform SHALL zeroize key material in memory
8. THE iOS_Platform SHALL support key migration on device restore

### Requirement 9: Network and Transport

**User Story:** As a user, I want reliable networking, so that connections work on iOS.

#### Acceptance Criteria

1. THE iOS_Platform SHALL support QUIC transport via Network.framework
2. THE iOS_Platform SHALL handle network path changes
3. THE iOS_Platform SHALL support WiFi and cellular connections
4. THE iOS_Platform SHALL respect Low Data Mode settings
5. THE iOS_Platform SHALL support IPv4 and IPv6
6. THE iOS_Platform SHALL handle VPN configurations
7. THE iOS_Platform SHALL provide network quality indicators
8. THE iOS_Platform SHALL handle airplane mode transitions

### Requirement 10: UI/UX Requirements

**User Story:** As a user, I want a native iOS experience, so that the app feels at home.

#### Acceptance Criteria

1. THE iOS_App SHALL follow Human Interface Guidelines
2. THE iOS_App SHALL support Dark Mode
3. THE iOS_App SHALL support Dynamic Type for accessibility
4. THE iOS_App SHALL support VoiceOver
5. THE iOS_App SHALL support all iPhone and iPad sizes
6. THE iOS_App SHALL support Split View and Slide Over (iPad)
7. THE iOS_App SHALL use SF Symbols for icons
8. THE iOS_App SHALL support Stage Manager (iPadOS 16+)

### Requirement 11: Rust Integration

**User Story:** As a developer, I want clean Rust integration, so that core logic is shared.

#### Acceptance Criteria

1. THE iOS_Platform SHALL use UniFFI or C-ABI for Rust-Swift bridge
2. THE iOS_Platform SHALL generate Swift-friendly API bindings
3. THE iOS_Platform SHALL handle async operations across FFI
4. THE iOS_Platform SHALL manage memory correctly across boundary
5. THE iOS_Platform SHALL support both arm64 and arm64-simulator
6. THE iOS_Platform SHALL provide XCFramework for distribution
7. THE iOS_Platform SHALL support Swift Package Manager integration
8. THE iOS_Platform SHALL handle errors across FFI boundary

### Requirement 12: App Store Compliance

**User Story:** As a developer, I want App Store compliance, so that the app can be distributed.

#### Acceptance Criteria

1. THE iOS_App SHALL comply with App Store Review Guidelines
2. THE iOS_App SHALL declare required capabilities in Info.plist
3. THE iOS_App SHALL provide privacy policy and usage descriptions
4. THE iOS_App SHALL support App Tracking Transparency if needed
5. THE iOS_App SHALL handle in-app purchases if monetized
6. THE iOS_App SHALL support TestFlight for beta distribution
7. THE iOS_App SHALL provide App Store screenshots and metadata
8. THE iOS_App SHALL handle app review rejection feedback
