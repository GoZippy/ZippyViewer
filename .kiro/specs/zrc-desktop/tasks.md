# Implementation Plan: zrc-desktop

## Overview

Implementation tasks for the graphical user interface (GUI) desktop application. This app provides a user-friendly experience for viewing and controlling remote devices, managing pairings, and handling file transfers.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with dependencies
    - zrc-core, zrc-crypto, zrc-proto, zrc-transport
    - egui, eframe, wgpu, tokio, serde
    - _Requirements: 1.1, 3.1, 4.2_
  - [x] 1.2 Create module structure
    - app, ui, viewer, device, session, input, clipboard, transfer, settings
    - _Requirements: 1.1, 2.1, 3.1, 5.1, 7.1, 8.1, 11.1_

- [x] 2. Implement Application Core
  - [x] 2.1 Implement ZrcDesktopApp struct
    - Identity, device manager, session manager, settings
    - _Requirements: 1.1, 2.1_
  - [x] 2.2 Implement eframe::App trait
    - Update loop, event handling
    - _Requirements: 3.1_
  - [x] 2.3 Implement UiState management
    - Views, dialogs, notifications
    - _Requirements: 1.1, 2.1_

- [x] 3. Implement Device Manager
  - [x] 3.1 Implement device list storage
    - Load from pairings store
    - _Requirements: 1.1_
  - [x] 3.2 Implement device status tracking
    - Online/offline, last seen
    - _Requirements: 1.3, 1.4_
  - [x] 3.3 Implement device grouping
    - _Requirements: 1.5_
  - [x] 3.4 Implement search and filter
    - _Requirements: 1.6_

- [x] 4. Implement Device List View
  - [x] 4.1 Implement device list UI
    - Name, ID, status indicator
    - _Requirements: 1.1, 1.2, 1.3_
  - [x] 4.2 Implement context menu
    - Connect, Properties, Remove
    - _Requirements: 1.7_
  - [x] 4.3 Implement double-click to connect
    - _Requirements: 1.8_

- [x] 5. Checkpoint - Verify device list functionality
  - Ensure device list displays correctly
  - Ask the user if questions arise

- [x] 6. Implement Connection Flow
  - [x] 6.1 Implement connection initiation
    - Session request, progress display
    - _Requirements: 2.1, 2.2_
  - [x] 6.2 Implement SAS verification dialog
    - _Requirements: 2.3_
  - [x] 6.3 Implement connection error handling
    - Error display, retry option
    - _Requirements: 2.5, 2.6_
  - [x] 6.4 Implement connection cancellation
    - _Requirements: 2.7_

- [x] 7. Implement Session Manager
  - [x] 7.1 Implement session lifecycle
    - Connect, disconnect, cleanup
    - _Requirements: 2.1, 2.4_
  - [x] 7.2 Implement session events
    - Connected, disconnected, quality changed
    - _Requirements: 2.4, 12.4_
  - [x] 7.3 Implement multi-session support
    - _Requirements: 3.5_
  - [ ]* 7.4 Write property test for session cleanup
    - **Property 3: Session Cleanup**
    - **Validates: Requirement 2.6**

- [x] 8. Implement Viewer Window
  - [x] 8.1 Implement viewer window structure
    - Frame display, toolbar, status bar
    - _Requirements: 3.1, 3.6, 3.7_
  - [x] 8.2 Implement window resizing
    - Aspect ratio preservation
    - _Requirements: 3.2, 3.4_
  - [x] 8.3 Implement fullscreen mode
    - F11 or double-click
    - _Requirements: 3.3_
  - [x] 8.4 Implement zoom controls
    - Fit, 100%, custom
    - _Requirements: 3.8_

- [x] 9. Implement Frame Renderer
  - [x] 9.1 Implement texture manager
    - GPU texture upload
    - _Requirements: 4.2_
  - [x] 9.2 Implement frame decoding
    - Format conversion
    - _Requirements: 4.3_
  - [x] 9.3 Implement frame dropping
    - Drop when behind
    - _Requirements: 4.4_
  - [x] 9.4 Implement resolution change handling
    - _Requirements: 4.7_
  - [ ]* 9.5 Write property test for frame ordering
    - **Property 1: Frame Ordering**
    - **Validates: Requirement 4.4**

- [x] 10. Implement Input Handler
  - [x] 10.1 Implement mouse capture
    - Movement, clicks, scroll
    - _Requirements: 5.1, 5.2, 5.3_
  - [x] 10.2 Implement keyboard capture
    - When viewer focused
    - _Requirements: 5.4_
  - [x] 10.3 Implement coordinate mapping
    - Local to remote coordinates
    - _Requirements: 5.5_
  - [x] 10.4 Implement special key sequences
    - Ctrl+Alt+Del via menu
    - _Requirements: 5.6_
  - [x] 10.5 Implement input mode toggle
    - View-only vs control
    - _Requirements: 5.7, 5.8_
  - [x]* 10.6 Write property test for coordinate accuracy
    - **Property 2: Input Coordinate Accuracy**
    - **Validates: Requirement 5.5**

- [x] 11. Checkpoint - Verify viewer functionality
  - Ensure frame display and input work
  - Ask the user if questions arise

- [x] 12. Implement Multi-Monitor Support
  - [x] 12.1 Implement monitor selector
    - Layout diagram
    - _Requirements: 6.1, 6.2_
  - [x] 12.2 Implement monitor switching
    - _Requirements: 6.3_
  - [x] 12.3 Implement tiled layout
    - _Requirements: 6.4_
  - [x] 12.4 Implement monitor preference persistence
    - _Requirements: 6.5_

- [x] 13. Implement Clipboard Sync
  - [x] 13.1 Implement clipboard monitoring
    - Detect local changes
    - _Requirements: 7.1_
  - [x] 13.2 Implement text and image support
    - _Requirements: 7.2, 7.3_
  - [x] 13.3 Implement sync toggle
    - _Requirements: 7.5_
  - [x] 13.4 Implement size limits
    - _Requirements: 7.7_
  - [x] 13.5 Write property test for clipboard size enforcement
    - **Property 4: Clipboard Size Enforcement**
    - **Validates: Requirement 7.7**

- [x] 14. Implement File Transfer
  - [x] 14.1 Implement drag-and-drop upload
    - _Requirements: 8.1_
  - [x] 14.2 Implement file browser
    - _Requirements: 8.2_
  - [x] 14.3 Implement transfer progress UI
    - Speed, ETA, queue
    - _Requirements: 8.3, 8.4_
  - [x] 14.4 Implement pause/resume/cancel
    - _Requirements: 8.5_
  - [x] 14.5 Implement integrity verification
    - _Requirements: 8.6_
  - [x] 14.6 Write property test for transfer integrity
    - **Property 5: Transfer Integrity**
    - **Validates: Requirement 8.6**

- [x] 15. Implement Session Controls
  - [x] 15.1 Implement toolbar
    - Disconnect, fullscreen, monitor, clipboard
    - _Requirements: 9.1, 9.2_
  - [x] 15.2 Implement session info display
    - Duration, bandwidth
    - _Requirements: 9.4, 9.5_
  - [x] 15.3 Implement quality slider
    - _Requirements: 9.7_
  - [x] 15.4 Implement keyboard shortcuts
    - _Requirements: 9.8_

- [x] 16. Implement Pairing Management
  - [x] 16.1 Implement invite import
    - QR, clipboard, file
    - _Requirements: 10.1, 10.2, 10.3_
  - [x] 16.2 Implement pairing wizard
    - _Requirements: 10.4_
  - [x] 16.3 Implement pairing details view
    - _Requirements: 10.5_
  - [x] 16.4 Implement device name editing
    - _Requirements: 10.6_
  - [x] 16.5 Implement pairing removal
    - _Requirements: 10.7_

- [x] 17. Implement Settings
  - [x] 17.1 Implement settings dialog
    - _Requirements: 11.1_
  - [x] 17.2 Implement appearance settings
    - Theme, font size
    - _Requirements: 11.4_
  - [x] 17.3 Implement input settings
    - _Requirements: 11.2_
  - [x] 17.4 Implement transport settings
    - _Requirements: 11.3_
  - [x] 17.5 Implement settings persistence
    - _Requirements: 11.6_
  - [x] 17.6 Write property test for settings persistence
    - **Property 6: Settings Persistence**
    - **Validates: Requirement 11.6**

- [x] 18. Implement Connection Diagnostics
  - [x] 18.1 Implement latency display
    - _Requirements: 12.1_
  - [x] 18.2 Implement packet loss display
    - _Requirements: 12.2_
  - [x] 18.3 Implement bandwidth display
    - _Requirements: 12.3_
  - [x] 18.4 Implement connection info dialog
    - _Requirements: 12.5_
  - [x] 18.5 Write property test for connection quality indication
    - **Property 7: Connection Quality Indication**
    - **Validates: Requirements 12.1, 12.2, 12.3**

- [x] 19. Implement Platform Integration
  - [x] 19.1 Implement system tray
    - _Requirements: 13.2_
  - [x] 19.2 Implement notifications
    - _Requirements: 13.3_
  - [x] 19.3 Implement theme support
    - _Requirements: 13.4_
  - [x] 19.4 Implement high-DPI support
    - _Requirements: 13.5_
  - [x] 19.5 Implement accessibility
    - Keyboard navigation, screen reader
    - _Requirements: 13.6_
  - [x] 19.6 Write property test for accessibility compliance
    - **Property 8: Accessibility Compliance**
    - **Validates: Requirement 13.6**

- [x] 20. Checkpoint - Verify all tests pass
  - Ensure all property tests pass with 100+ iterations
  - Verify desktop app functionality end-to-end
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- Built with egui/eframe for cross-platform compatibility
- GPU acceleration via wgpu for frame rendering
- Accessibility is critical for enterprise adoption
