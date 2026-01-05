# Implementation Plan: zrc-platform-android

## Overview

Implementation tasks for the Android platform layer. This crate provides a controller application for viewing and controlling remote devices from Android, with optional host capabilities via MediaProjection and AccessibilityService.

## Tasks

- [x] 1. Set up project structure
  - [x] 1.1 Create Android project with Kotlin
    - Configure Gradle with NDK support
    - Add Rust toolchain integration (cargo-ndk)
    - _Requirements: 12.1, 12.8_
  - [x] 1.2 Create Rust library crate
    - Configure Cargo.toml for Android targets
    - Add jni crate dependency
    - _Requirements: 12.1, 12.2_
  - [x] 1.3 Set up JNI bindings
    - Create ZrcCore.kt wrapper class
    - Implement native method declarations
    - _Requirements: 12.1, 12.6_

- [x] 2. Implement JNI bridge
  - [x] 2.1 Implement core initialization
    - Java_io_zippyremote_core_ZrcCore_init
    - Configuration parsing from JSON
    - _Requirements: 12.1, 12.4_
  - [x] 2.2 Implement session management
    - startSession, endSession JNI methods
    - _Requirements: 4.1, 4.5_
  - [x] 2.3 Implement frame polling
    - pollFrame with ByteArray return
    - _Requirements: 1.1, 1.8_
  - [x] 2.4 Implement input sending
    - sendInput with JSON event encoding
    - _Requirements: 2.1, 3.2_
  - [ ]* 2.5 Write property test for JNI memory safety
    - **Property 1: JNI Memory Safety**
    - **Validates: Requirements 12.5, 12.6**

- [x] 3. Implement frame rendering
  - [x] 3.1 Create ViewerSurfaceView
    - SurfaceView with SurfaceHolder.Callback
    - _Requirements: 1.1, 1.2_
  - [x] 3.2 Implement RenderThread
    - Background thread for frame rendering
    - Frame decoding and drawing
    - _Requirements: 1.7, 1.8_
  - [x] 3.3 Implement zoom and pan
    - ScaleGestureDetector for pinch-to-zoom
    - GestureDetector for pan
    - _Requirements: 1.4, 1.6_
  - [ ]* 3.4 Write property test for frame rendering continuity
    - **Property 2: Frame Rendering Continuity**
    - **Validates: Requirements 1.7, 1.8**

- [x] 4. Implement touch input handling
  - [x] 4.1 Create TouchInputHandler
    - Gesture detection and mapping
    - _Requirements: 2.1, 2.7_
  - [x] 4.2 Implement tap and long-press
    - Single tap for left-click
    - Long-press for right-click with haptic
    - _Requirements: 2.2, 2.3_
  - [x] 4.3 Implement drag and scroll
    - Single finger drag for mouse move
    - Two-finger scroll
    - _Requirements: 2.4, 2.5_
  - [x] 4.4 Implement coordinate mapping
    - Local to remote coordinate conversion
    - _Requirements: 2.1, 2.4_
  - [ ]* 4.5 Write property test for touch coordinate accuracy
    - **Property 3: Touch Coordinate Accuracy**
    - **Validates: Requirements 2.1, 2.4**

- [x] 5. Implement keyboard input
  - [x] 5.1 Create soft keyboard integration
    - Show/hide keyboard on demand
    - _Requirements: 3.1_
  - [x] 5.2 Implement special keys toolbar
    - Ctrl, Alt, Win, function keys
    - _Requirements: 3.3, 3.6, 3.7_
  - [x] 5.3 Implement hardware keyboard support
    - KeyEvent handling
    - _Requirements: 3.4_
  - [x] 5.4 Implement Ctrl+Alt+Del menu
    - _Requirements: 3.8_

- [x] 6. Implement connection management
  - [x] 6.1 Create connection status indicator
    - UI component for status display
    - _Requirements: 4.4_
  - [x] 6.2 Implement network change handling
    - ConnectivityManager listener
    - Connection migration
    - _Requirements: 4.2, 4.3_
  - [x] 6.3 Implement foreground service
    - Persistent notification during session
    - _Requirements: 4.7_
  - [x] 6.4 Implement auto-reconnection
    - Exponential backoff retry
    - _Requirements: 4.5_

- [x] 7. Implement pairing and device management
  - [x] 7.1 Create device list UI
    - RecyclerView with device items
    - Online/offline status
    - _Requirements: 5.3, 5.4_
  - [x] 7.2 Implement QR code scanning
    - CameraX integration
    - Invite parsing
    - _Requirements: 5.1_
  - [x] 7.3 Implement clipboard paste import
    - _Requirements: 5.2_
  - [x] 7.4 Implement SAS verification display
    - _Requirements: 5.8_

- [x] 8. Implement Android Keystore integration
  - [x] 8.1 Create AndroidKeyStore class
    - KeyPairGenerator with AndroidKeyStore provider
    - _Requirements: 9.1, 9.2_
  - [x] 8.2 Implement key generation
    - EC key with StrongBox if available
    - _Requirements: 9.2_
  - [x] 8.3 Implement secret storage
    - EncryptedSharedPreferences
    - _Requirements: 9.1_
  - [x] 8.4 Implement key zeroization
    - _Requirements: 9.6_
  - [ ]* 8.5 Write property test for Keystore security
    - **Property 4: Keystore Security**
    - **Validates: Requirements 9.1, 9.2**

- [x] 9. Implement clipboard synchronization
  - [x] 9.1 Implement clipboard read
    - ClipboardManager access
    - _Requirements: 6.1_
  - [x] 9.2 Implement clipboard write
    - Set local clipboard from remote
    - _Requirements: 6.4_
  - [x] 9.3 Handle clipboard permission (Android 10+)
    - _Requirements: 6.6_
  - [x] 9.4 Implement sync toggle
    - _Requirements: 6.7_

- [x] 10. Implement host mode - MediaProjection (optional)
  - [x] 10.1 Create ScreenCaptureService
    - Foreground service with notification
    - _Requirements: 7.1, 7.3, 7.4_
  - [x] 10.2 Implement MediaProjection capture
    - ImageReader for frame capture
    - _Requirements: 7.1, 7.2_
  - [x] 10.3 Implement frame processing
    - Send frames to connected controller
    - _Requirements: 7.5_
  - [x] 10.4 Handle rotation and quality
    - _Requirements: 7.5, 7.6_
  - [ ]* 10.5 Write property test for service lifecycle
    - **Property 5: Service Lifecycle**
    - **Validates: Requirements 4.7, 7.4**

- [x] 11. Implement host mode - AccessibilityService (optional)
  - [x] 11.1 Create ZrcAccessibilityService
    - AccessibilityService implementation
    - _Requirements: 8.1, 8.2_
  - [x] 11.2 Implement gesture injection
    - Tap, swipe, scroll via dispatchGesture
    - _Requirements: 8.3_
  - [x] 11.3 Implement text input
    - ACTION_SET_TEXT on focused node
    - _Requirements: 8.4_
  - [x] 11.4 Implement setup instructions
    - Guide user to enable accessibility
    - _Requirements: 8.6_

- [x] 12. Implement UI/UX
  - [x] 12.1 Apply Material Design
    - Material 3 components
    - _Requirements: 11.1_
  - [x] 12.2 Implement dark/light themes
    - System theme following
    - _Requirements: 11.2, 11.3_
  - [x] 12.3 Implement accessibility
    - TalkBack support
    - _Requirements: 11.4_
  - [x] 12.4 Support multiple screen sizes
    - Phone and tablet layouts
    - _Requirements: 11.5_

- [x] 13. Checkpoint - Verify all tests pass
  - Run all unit and integration tests
  - Test on various Android versions (API 26+)
  - Test on phone and tablet
  - Test with hardware keyboard
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- Minimum Android API level 26 (Android 8.0)
- Host mode features are optional
- JNI bridge uses cargo-ndk for cross-compilation
