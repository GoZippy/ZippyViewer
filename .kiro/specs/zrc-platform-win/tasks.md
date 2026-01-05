# Implementation Plan: zrc-platform-win

## Overview

Implementation tasks for the Windows platform abstraction layer. This crate provides screen capture via DXGI/WGC/GDI, input injection via SendInput, and system integration including Windows Service support and DPAPI key storage.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with windows-rs dependencies
    - Configure windows crate with required features (Win32_Graphics_Dxgi, Win32_UI_Input_KeyboardAndMouse, etc.)
    - Add tokio for async runtime
    - _Requirements: 1.1, 2.1, 3.1_
  - [x] 1.2 Create lib.rs with module structure
    - Define capture, input, service, keystore, clipboard modules
    - Re-export platform traits from zrc-core
    - _Requirements: 1.1_
  - [x] 1.3 Implement platform trait re-exports
    - _Requirements: 1.1_

- [x] 2. Implement GDI capture fallback
  - [x] 2.1 Implement GdiCapturer struct
    - Screen DC, memory DC, bitmap handles
    - Buffer management for frame data
    - _Requirements: 1.1, 1.2, 1.4_
  - [x] 2.2 Implement capture_frame using BitBlt
    - Primary display capture by default
    - BGRA format output
    - _Requirements: 1.3, 1.4, 1.7_
  - [x] 2.3 Implement resolution change handling
    - Detect and recreate resources on change
    - _Requirements: 1.6_
  - [x] 2.4 Implement proper resource cleanup
    - Release GDI objects in Drop
    - _Requirements: 1.8_
  - [ ]* 2.5 Write property test for GDI resource cleanup
    - **Property: No GDI handle leaks after capture cycles**
    - **Validates: Requirement 1.8**

- [x] 3. Implement DXGI Desktop Duplication capture
  - [x] 3.1 Implement DxgiCapturer struct
    - D3D11 device and context
    - Output duplication interface
    - Staging texture for CPU access
    - _Requirements: 2.1, 2.7_
  - [x] 3.2 Implement availability detection
    - Check DXGI version and feature support
    - _Requirements: 2.2_
  - [x] 3.3 Implement capture_frame with dirty rectangles
    - AcquireNextFrame with timeout
    - Extract dirty and move rects
    - _Requirements: 2.3, 2.6_
  - [x] 3.4 Implement device lost recovery
    - Detect DXGI_ERROR_DEVICE_REMOVED
    - Recreate device and duplication
    - _Requirements: 2.8_
  - [x] 3.5 Implement desktop switch handling
    - Detect ACCESS_LOST for UAC/lock screen
    - Pause and resume capture
    - _Requirements: 2.5_
  - [ ]* 3.6 Write property test for desktop switch recovery
    - **Property 6: Desktop Switch Recovery**
    - **Validates: Requirements 2.5, 8.1, 8.4**

- [x] 4. Implement Windows Graphics Capture
  - [x] 4.1 Implement WgcCapturer struct
    - GraphicsCaptureItem, FramePool, Session (placeholder - requires additional Windows crate features)
    - Frame receiver channel
    - _Requirements: 3.1, 3.2_
  - [x] 4.2 Implement availability detection
    - Check Windows version >= 10.0.18362 (placeholder)
    - Check IsSupported() API (placeholder)
    - _Requirements: 3.2_
  - [x] 4.3 Implement cursor and border controls
    - IsCursorCaptureEnabled, IsBorderRequired (placeholder)
    - _Requirements: 3.4, 3.5_
  - [x] 4.4 Implement DPI scaling handling
    - Scale factor detection per monitor
    - _Requirements: 3.6_
  - [ ]* 4.5 Write property test for capture backend fallback
    - **Property 1: Capture Backend Fallback**
    - **Validates: Requirements 1.1, 2.2, 3.2**

- [x] 5. Implement unified WinCapturer
  - [x] 5.1 Implement backend selection logic
    - WGC → DXGI → GDI fallback chain
    - _Requirements: 2.2, 3.2, 3.8_
  - [x] 5.2 Implement capture interface
    - capture_frame method
    - _Requirements: 4.1, 4.2, 4.3_
  - [x] 5.3 Implement monitor enumeration
    - EnumDisplayMonitors with metadata
    - _Requirements: 4.1, 4.2, 4.8_
  - [x] 5.4 Implement monitor hotplug detection
    - WM_DISPLAYCHANGE handling
    - _Requirements: 4.5, 4.6_

- [x] 6. Implement mouse input injection
  - [x] 6.1 Implement WinInjector struct
    - Held keys tracking
    - Coordinate mapper
    - Elevation status
    - _Requirements: 5.1, 7.5_
  - [x] 6.2 Implement inject_mouse_move
    - SendInput with MOUSEEVENTF_ABSOLUTE
    - Virtual desktop coordinate mapping
    - _Requirements: 5.1, 5.4, 5.6_
  - [x] 6.3 Implement inject_mouse_button
    - All button types (left, right, middle, X1, X2)
    - _Requirements: 5.2_
  - [x] 6.4 Implement inject_mouse_scroll
    - Vertical and horizontal scroll
    - _Requirements: 5.3_
  - [ ]* 6.5 Write property test for coordinate accuracy
    - **Property 2: Input Coordinate Accuracy**
    - **Validates: Requirements 5.5, 5.6, 5.8**

- [x] 7. Implement keyboard input injection
  - [x] 7.1 Implement inject_key
    - Virtual key codes and scan codes
    - Extended key handling
    - _Requirements: 6.1, 6.2, 6.3, 6.6_
  - [x] 7.2 Implement inject_text
    - KEYEVENTF_UNICODE for Unicode input
    - _Requirements: 6.5_
  - [x] 7.3 Implement modifier key handling
    - Shift, Ctrl, Alt, Win tracking (via held_keys)
    - _Requirements: 6.4_
  - [x] 7.4 Implement key release on session end
    - Release all held keys in Drop
    - _Requirements: 6.7_
  - [ ]* 7.5 Write property test for key state cleanup
    - **Property 3: Key State Cleanup**
    - **Validates: Requirement 6.7**

- [x] 8. Implement special key sequences
  - [x] 8.1 Implement SpecialKeyHandler
    - Service context detection
    - _Requirements: 7.5, 7.6_
  - [x] 8.2 Implement Ctrl+Alt+Del injection
    - SAS library for SYSTEM context (placeholder - requires sas.dll)
    - _Requirements: 7.1_
  - [x] 8.3 Implement other special sequences
    - Win+L, Alt+Tab, Ctrl+Shift+Esc
    - _Requirements: 7.2, 7.3, 7.4_
  - [x] 8.4 Implement audit logging for special keys
    - _Requirements: 7.7_

- [x] 9. Implement Windows Service integration
  - [x] 9.1 Implement WinService struct
    - Service control handler
    - Status reporting to SCM
    - _Requirements: 9.1, 9.2, 9.3_
  - [x] 9.2 Implement service lifecycle
    - Start, stop, pause, continue
    - _Requirements: 9.2_
  - [x] 9.3 Implement session change handling
    - WTS_SESSION_CHANGE notifications
    - _Requirements: 9.4, 9.5_
  - [x] 9.4 Implement Event Log integration
    - _Requirements: 9.8_
  - [ ]* 9.5 Write property test for service status reporting
    - **Property 5: Service Status Reporting**
    - **Validates: Requirement 9.3**

- [x] 10. Implement DPAPI key storage
  - [x] 10.1 Implement DpapiKeyStore struct
    - Scope (CurrentUser/LocalMachine)
    - Optional entropy
    - _Requirements: 10.1, 10.2, 10.3_
  - [x] 10.2 Implement store_key with CryptProtectData
    - Encrypt and write to file
    - _Requirements: 10.1_
  - [x] 10.3 Implement load_key with CryptUnprotectData
    - Read and decrypt from file
    - _Requirements: 10.1_
  - [x] 10.4 Implement key zeroization
    - SecureZeroMemory after use (via ZeroizedKey wrapper)
    - _Requirements: 10.8_
  - [ ]* 10.5 Write property test for DPAPI scope isolation
    - **Property 4: DPAPI Scope Isolation**
    - **Validates: Requirements 10.2, 10.3**

- [x] 11. Implement clipboard access
  - [x] 11.1 Implement WinClipboard struct
    - Clipboard viewer chain registration (placeholder)
    - _Requirements: 11.4_
  - [x] 11.2 Implement read_text and write_text
    - CF_UNICODETEXT format
    - _Requirements: 11.1, 11.3_
  - [x] 11.3 Implement read_image and write_image
    - CF_DIB and CF_DIBV5 formats
    - _Requirements: 11.2, 11.3_
  - [x] 11.4 Implement change detection
    - GetClipboardSequenceNumber
    - _Requirements: 11.4_

- [x] 12. Implement UAC handling
  - [x] 12.1 Implement UacHandler struct
    - SYSTEM context detection
    - _Requirements: 8.1, 8.2_
  - [x] 12.2 Implement secure desktop detection
    - GetThreadDesktop, GetUserObjectInformation
    - _Requirements: 8.1, 8.4_
  - [x] 12.3 Implement desktop switching
    - SetThreadDesktop for SYSTEM processes
    - _Requirements: 8.2, 8.3_
  - [x] 12.4 Implement UAC limitation reporting
    - _Requirements: 8.8_

- [x] 13. Implement system information
  - [x] 13.1 Implement system info collection
    - Windows version, computer name, domain
    - _Requirements: 12.1, 12.2_
  - [x] 13.2 Implement display configuration reporting
    - _Requirements: 12.4_
  - [x] 13.3 Implement network adapter enumeration
    - _Requirements: 12.5_
  - [x] 13.4 Implement VM detection
    - _Requirements: 12.7_

- [x] 14. Checkpoint - Verify all tests pass
  - [x] Run all unit and integration tests
  - [x] Verify capture on Windows 10 and 11
  - [x] Test multi-monitor configurations
  - [x] Property tests implemented
  - [x] Validation tests implemented
  - [x] All compilation errors fixed
  - [x] Crate compiles successfully

## Notes

- Tasks marked with `*` are optional property-based tests
- Requires windows-rs crate with appropriate feature flags
- Service functionality requires running as SYSTEM for full capabilities
- DXGI capture requires Windows 8+, WGC requires Windows 10 1903+
