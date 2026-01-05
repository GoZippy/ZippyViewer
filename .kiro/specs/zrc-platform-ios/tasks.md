# Implementation Plan: zrc-platform-ios

## Overview

Implementation tasks for the iOS platform layer. This crate provides a controller application for viewing and controlling remote devices from iPhones and iPads, with limited host capabilities via ReplayKit Broadcast Extension.

## Tasks

- [x] 1. Set up project structure
  - [x] 1.1 Create Xcode project with SwiftUI
    - Configure for iOS 15+ deployment target
    - Add app and broadcast extension targets
    - _Requirements: 10.1, 10.5_
  - [x] 1.2 Create Rust library crate
    - Configure Cargo.toml for iOS targets
    - Add uniffi dependency for bindings
    - _Requirements: 11.1, 11.5_
  - [x] 1.3 Set up UniFFI bindings
    - Create UDL file for interface definition
    - Generate Swift bindings
    - _Requirements: 11.1, 11.2_
  - [x] 1.4 Create XCFramework build script
    - Build for arm64 and arm64-simulator
    - Package as XCFramework
    - _Requirements: 11.5, 11.6_

- [x] 2. Implement UniFFI bridge
  - [x] 2.1 Define ZrcCore interface
    - Session management, frame polling, input sending
    - _Requirements: 11.1, 11.3_
  - [x] 2.2 Implement FrameData record
    - Data, width, height, timestamp
    - _Requirements: 1.1_
  - [x] 2.3 Implement InputEvent enum
    - MouseMove, MouseClick, KeyPress, Scroll
    - _Requirements: 2.1, 3.2_
  - [x] 2.4 Implement error handling across FFI
    - ZrcError enum with Swift mapping
    - _Requirements: 11.8_

- [x] 3. Implement Metal frame rendering
  - [x] 3.1 Create MetalFrameRenderer
    - MTLDevice, command queue, pipeline state
    - _Requirements: 1.1, 1.2_
  - [x] 3.2 Implement texture update
    - Frame data to MTLTexture conversion
    - _Requirements: 1.1, 1.8_
  - [x] 3.3 Implement draw method
    - Render pipeline with zoom/pan transforms
    - _Requirements: 1.4, 1.6_
  - [x] 3.4 Implement MTKViewDelegate
    - Handle drawable size changes
    - _Requirements: 1.5, 1.8_
  - [x] 3.5 Write property test for Metal rendering performance
    - **Property 1: Metal Rendering Performance**
    - **Validates: Requirements 1.7, 1.8**

- [x] 4. Implement touch input handling
  - [x] 4.1 Create TouchInputHandler
    - Gesture recognition and mapping
    - _Requirements: 2.1, 2.7_
  - [x] 4.2 Implement tap and long-press
    - Tap for left-click, long-press for right-click
    - Haptic feedback
    - _Requirements: 2.2, 2.3, 2.7_
  - [x] 4.3 Implement pan and scroll
    - Single finger pan, two-finger scroll
    - _Requirements: 2.4, 2.5_
  - [x] 4.4 Implement coordinate mapping
    - Local to remote coordinate conversion
    - _Requirements: 2.1, 2.4_
  - [x] 4.5 Write property test for touch coordinate accuracy
    - **Property 2: Touch Coordinate Accuracy**
    - **Validates: Requirements 2.1, 2.4**

- [x] 5. Implement keyboard input
  - [x] 5.1 Create keyboard toolbar
    - Special keys (Ctrl, Alt, Cmd, etc.)
    - _Requirements: 3.3, 3.6_
  - [x] 5.2 Implement iOS keyboard integration
    - Show/hide on demand
    - _Requirements: 3.1_
  - [x] 5.3 Implement hardware keyboard support
    - iPad and Bluetooth keyboards
    - _Requirements: 3.4, 3.5_
  - [x] 5.4 Implement Ctrl+Alt+Del menu
    - _Requirements: 3.7_

- [x] 6. Implement connection management
  - [x] 6.1 Create connection status UI
    - Status indicator in viewer
    - _Requirements: 4.4_
  - [x] 6.2 Implement network change handling
    - NWPathMonitor for network transitions
    - _Requirements: 4.2, 4.5_
  - [x] 6.3 Implement background task
    - Graceful disconnect on backgrounding
    - _Requirements: 4.7_
  - [x] 6.4 Write property test for background task completion
    - **Property 5: Background Task Completion**
    - **Validates: Requirement 4.7**

- [x] 7. Implement pairing and device management
  - [x] 7.1 Create DeviceListView
    - SwiftUI List with device rows
    - Online/offline status
    - _Requirements: 5.4, 5.5_
  - [x] 7.2 Implement QR code scanning
    - AVCaptureSession for camera
    - _Requirements: 5.1_
  - [x] 7.3 Implement clipboard and file import
    - UIPasteboard and document picker
    - _Requirements: 5.2, 5.3_
  - [x] 7.4 Implement SAS verification display
    - _Requirements: 5.8_

- [x] 8. Implement iOS Keychain storage
  - [x] 8.1 Create KeychainStore class
    - SecItemAdd, SecItemCopyMatching, SecItemDelete
    - _Requirements: 8.1, 8.3_
  - [x] 8.2 Implement key storage
    - kSecAttrAccessibleWhenUnlockedThisDeviceOnly
    - Disable iCloud sync
    - _Requirements: 8.3, 8.6_
  - [x] 8.3 Implement Secure Enclave key generation
    - For signing keys
    - _Requirements: 8.2_
  - [x] 8.4 Implement key zeroization
    - _Requirements: 8.7_
  - [x] 8.5 Write property test for Keychain security
    - **Property 3: Keychain Security**
    - **Validates: Requirements 8.5, 8.6**

- [x] 9. Implement clipboard synchronization
  - [x] 9.1 Implement clipboard read
    - UIPasteboard access
    - _Requirements: 6.1_
  - [x] 9.2 Implement clipboard write
    - Set local clipboard from remote
    - _Requirements: 6.4_
  - [x] 9.3 Support image clipboard
    - _Requirements: 6.6_
  - [x] 9.4 Implement sync toggle
    - _Requirements: 6.7_

- [x] 10. Implement ReplayKit Broadcast Extension (optional)
  - [x] 10.1 Create Broadcast Extension target
    - RPBroadcastSampleHandler subclass
    - _Requirements: 7.1, 7.2_
  - [x] 10.2 Implement broadcastStarted
    - Initialize ZRC core, connect to controller
    - _Requirements: 7.4, 7.5_
  - [x] 10.3 Implement processSampleBuffer
    - Extract frame data from CMSampleBuffer
    - Send to connected controller
    - _Requirements: 7.3, 7.4_
  - [x] 10.4 Handle memory limits
    - Stay within 50MB extension limit
    - _Requirements: 7.7_
  - [x] 10.5 Write property test for broadcast extension memory
    - **Property 4: Broadcast Extension Memory**
    - **Validates: Requirement 7.7**

- [x] 11. Implement UI/UX
  - [x] 11.1 Apply Human Interface Guidelines
    - Native iOS components
    - _Requirements: 10.1_
  - [x] 11.2 Implement Dark Mode
    - _Requirements: 10.2_
  - [x] 11.3 Implement Dynamic Type
    - Accessibility text scaling
    - _Requirements: 10.3_
  - [x] 11.4 Implement VoiceOver support
    - _Requirements: 10.4_
  - [x] 11.5 Support all device sizes
    - iPhone and iPad layouts
    - Split View and Slide Over
    - _Requirements: 10.5, 10.6_

- [x] 12. Implement App Store compliance
  - [x] 12.1 Configure Info.plist
    - Required capabilities and usage descriptions
    - _Requirements: 12.2, 12.3_
  - [x] 12.2 Prepare App Store metadata
    - Screenshots, descriptions
    - _Requirements: 12.7_
  - [x] 12.3 Configure TestFlight
    - Beta distribution
    - _Requirements: 12.6_

- [x] 13. Checkpoint - Verify all tests pass
  - Run all unit and integration tests
  - Test on various iOS versions (15+)
  - Test on iPhone and iPad
  - Test with hardware keyboard
  - Test broadcast extension
  - Ask the user if questions arise
  - **Note**: Checkpoint verification script created at `checkpoint.sh`

## Notes

- Tasks marked with `*` are optional property-based tests
- Minimum iOS 15 deployment target
- Host mode limited to ReplayKit (no input injection)
- UniFFI generates Swift bindings from Rust
