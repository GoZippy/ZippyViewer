# Requirements Document: zrc-desktop

## Introduction

The zrc-desktop crate implements a graphical user interface (GUI) for the Zippy Remote Control (ZRC) system. This desktop application provides a user-friendly experience for viewing and controlling remote devices, managing pairings, and handling file transfers. The application targets Windows, macOS, and Linux platforms.

## Glossary

- **Viewer**: The window displaying the remote device's screen
- **Device_List**: UI showing paired devices and their online status
- **Session_Controls**: UI elements for managing active sessions
- **Input_Capture**: Capturing local mouse/keyboard for remote transmission
- **Frame_Renderer**: Component displaying received screen frames
- **Connection_Status**: Visual indicator of connection health
- **Toolbar**: Quick access controls for common actions

## Requirements

### Requirement 1: Device List View

**User Story:** As an operator, I want to see my paired devices, so that I can quickly connect to the device I need.

#### Acceptance Criteria

1. THE Desktop_App SHALL display a list of all paired devices
2. THE Desktop_App SHALL show device name, device_id (truncated), and online status for each device
3. THE Desktop_App SHALL indicate online/offline status with visual indicator (green/gray dot)
4. THE Desktop_App SHALL show last seen timestamp for offline devices
5. THE Desktop_App SHALL support device grouping and folders
6. THE Desktop_App SHALL support device search/filter by name or ID
7. THE Desktop_App SHALL support right-click context menu: Connect, Properties, Remove
8. THE Desktop_App SHALL double-click to initiate connection

### Requirement 2: Connection Initiation

**User Story:** As an operator, I want to connect to devices easily, so that I can start remote sessions quickly.

#### Acceptance Criteria

1. THE Desktop_App SHALL initiate session when user selects Connect
2. THE Desktop_App SHALL display connection progress with status messages
3. THE Desktop_App SHALL show SAS verification dialog when required
4. WHEN connection succeeds, THE Desktop_App SHALL open viewer window
5. WHEN connection fails, THE Desktop_App SHALL display error with retry option
6. THE Desktop_App SHALL support connection timeout configuration
7. THE Desktop_App SHALL support canceling connection in progress
8. THE Desktop_App SHALL remember last used connection settings per device

### Requirement 3: Viewer Window

**User Story:** As an operator, I want to see the remote screen, so that I can view and interact with the remote device.

#### Acceptance Criteria

1. THE Desktop_App SHALL display remote screen frames in a dedicated window
2. THE Desktop_App SHALL support window resizing with frame scaling
3. THE Desktop_App SHALL support fullscreen mode (F11 or double-click)
4. THE Desktop_App SHALL maintain aspect ratio when scaling
5. THE Desktop_App SHALL support multiple viewer windows for multiple sessions
6. THE Desktop_App SHALL display connection quality indicator
7. THE Desktop_App SHALL display current resolution and frame rate
8. THE Desktop_App SHALL support zoom controls (fit, 100%, custom)

### Requirement 4: Frame Rendering

**User Story:** As an operator, I want smooth frame display, so that the remote screen appears responsive.

#### Acceptance Criteria

1. THE Desktop_App SHALL render frames at display refresh rate when possible
2. THE Desktop_App SHALL use GPU acceleration for frame rendering (wgpu/Metal/DirectX)
3. THE Desktop_App SHALL handle frame format conversion (BGRA to RGBA)
4. THE Desktop_App SHALL drop frames when rendering falls behind
5. THE Desktop_App SHALL display frame statistics in debug mode
6. THE Desktop_App SHALL support color profile handling
7. THE Desktop_App SHALL handle resolution changes without restart
8. THE Desktop_App SHALL minimize input-to-display latency

### Requirement 5: Input Capture and Transmission

**User Story:** As an operator, I want to control the remote device, so that I can perform tasks remotely.

#### Acceptance Criteria

1. THE Desktop_App SHALL capture mouse movement within viewer window
2. THE Desktop_App SHALL capture mouse clicks (left, right, middle)
3. THE Desktop_App SHALL capture mouse scroll events
4. THE Desktop_App SHALL capture keyboard input when viewer is focused
5. THE Desktop_App SHALL map local coordinates to remote display coordinates
6. THE Desktop_App SHALL support special key combinations (Ctrl+Alt+Del via menu)
7. THE Desktop_App SHALL support input mode toggle (view-only vs control)
8. THE Desktop_App SHALL indicate when input is being captured

### Requirement 6: Multi-Monitor Support

**User Story:** As an operator, I want to access multiple monitors, so that I can view any screen on the remote device.

#### Acceptance Criteria

1. THE Desktop_App SHALL display monitor selector when remote has multiple monitors
2. THE Desktop_App SHALL show monitor layout diagram
3. THE Desktop_App SHALL support switching between monitors
4. THE Desktop_App SHALL support viewing all monitors in tiled layout
5. THE Desktop_App SHALL remember monitor preference per device
6. THE Desktop_App SHALL handle monitor configuration changes during session
7. THE Desktop_App SHALL display monitor names/numbers for identification
8. THE Desktop_App SHALL support primary monitor quick-select

### Requirement 7: Clipboard Synchronization

**User Story:** As an operator, I want clipboard sync, so that I can copy/paste between local and remote.

#### Acceptance Criteria

1. THE Desktop_App SHALL sync clipboard when enabled
2. THE Desktop_App SHALL support text clipboard content
3. THE Desktop_App SHALL support image clipboard content
4. THE Desktop_App SHALL indicate clipboard sync direction
5. THE Desktop_App SHALL support clipboard sync toggle in toolbar
6. THE Desktop_App SHALL respect clipboard permission from session
7. THE Desktop_App SHALL handle large clipboard content gracefully
8. THE Desktop_App SHALL show clipboard sync status in status bar

### Requirement 8: File Transfer

**User Story:** As an operator, I want to transfer files, so that I can move files between local and remote machines.

#### Acceptance Criteria

1. THE Desktop_App SHALL support drag-and-drop file upload to remote
2. THE Desktop_App SHALL support file browser for remote file system
3. THE Desktop_App SHALL display transfer progress with speed and ETA
4. THE Desktop_App SHALL support transfer queue for multiple files
5. THE Desktop_App SHALL support transfer pause/resume/cancel
6. THE Desktop_App SHALL verify file integrity after transfer
7. THE Desktop_App SHALL respect file transfer permission from session
8. THE Desktop_App SHALL support download from remote to local

### Requirement 9: Session Controls

**User Story:** As an operator, I want session controls, so that I can manage the active connection.

#### Acceptance Criteria

1. THE Desktop_App SHALL provide toolbar with common actions
2. THE Desktop_App SHALL support: Disconnect, Fullscreen, Monitor Select, Clipboard Toggle
3. THE Desktop_App SHALL support: File Transfer, Chat, Settings
4. THE Desktop_App SHALL display session duration
5. THE Desktop_App SHALL display bandwidth usage
6. THE Desktop_App SHALL support session recording toggle (if permitted)
7. THE Desktop_App SHALL support quality/performance slider
8. THE Desktop_App SHALL support keyboard shortcut customization

### Requirement 10: Pairing Management

**User Story:** As an operator, I want to manage pairings, so that I can add new devices and remove old ones.

#### Acceptance Criteria

1. THE Desktop_App SHALL support adding devices via invite import
2. THE Desktop_App SHALL support QR code scanning for invites (via webcam or image)
3. THE Desktop_App SHALL support paste invite from clipboard
4. THE Desktop_App SHALL display pairing wizard with progress steps
5. THE Desktop_App SHALL support viewing pairing details (permissions, paired date)
6. THE Desktop_App SHALL support editing device display name
7. THE Desktop_App SHALL support removing pairings with confirmation
8. THE Desktop_App SHALL support exporting/importing pairing data

### Requirement 11: Settings and Preferences

**User Story:** As an operator, I want configurable settings, so that I can customize the application behavior.

#### Acceptance Criteria

1. THE Desktop_App SHALL provide settings dialog
2. THE Desktop_App SHALL support configuring: default quality, input behavior, shortcuts
3. THE Desktop_App SHALL support configuring: transport preferences, server URLs
4. THE Desktop_App SHALL support configuring: appearance (theme, font size)
5. THE Desktop_App SHALL support configuring: notifications, sounds
6. THE Desktop_App SHALL persist settings across restarts
7. THE Desktop_App SHALL support settings import/export
8. THE Desktop_App SHALL support reset to defaults

### Requirement 12: Connection Health and Diagnostics

**User Story:** As an operator, I want connection diagnostics, so that I can troubleshoot connectivity issues.

#### Acceptance Criteria

1. THE Desktop_App SHALL display connection latency
2. THE Desktop_App SHALL display packet loss percentage
3. THE Desktop_App SHALL display bandwidth utilization
4. THE Desktop_App SHALL show connection type (direct, relay, mesh)
5. THE Desktop_App SHALL provide detailed connection info dialog
6. THE Desktop_App SHALL log connection events for troubleshooting
7. THE Desktop_App SHALL support exporting diagnostic logs
8. THE Desktop_App SHALL alert on connection quality degradation

### Requirement 13: Platform Integration

**User Story:** As an operator, I want native platform integration, so that the app feels natural on my OS.

#### Acceptance Criteria

1. THE Desktop_App SHALL use native window decorations where appropriate
2. THE Desktop_App SHALL support system tray icon with quick actions
3. THE Desktop_App SHALL support OS notifications for connection events
4. THE Desktop_App SHALL support OS dark/light theme
5. THE Desktop_App SHALL support high-DPI displays
6. THE Desktop_App SHALL support accessibility features (screen reader, keyboard navigation)
7. THE Desktop_App SHALL register for zrc:// URL scheme handling
8. THE Desktop_App SHALL support single-instance with focus on re-launch
