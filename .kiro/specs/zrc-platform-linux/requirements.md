# Requirements Document: zrc-platform-linux

## Introduction

The zrc-platform-linux crate implements Linux-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides screen capture, input injection, and system integration for Linux platforms, handling the diversity of display servers (X11, Wayland) and desktop environments.

## Glossary

- **X11**: Traditional X Window System display server
- **Wayland**: Modern display server protocol replacing X11
- **PipeWire**: Multimedia framework for screen capture on Wayland
- **XDG_Portal**: Desktop integration API for sandboxed applications
- **uinput**: Kernel module for virtual input device creation
- **XTest**: X11 extension for input injection
- **libsecret**: GNOME keyring integration for secure storage
- **systemd**: System and service manager for Linux

## Requirements

### Requirement 1: Screen Capture - X11

**User Story:** As a developer, I want X11 capture, so that screen capture works on traditional Linux desktops.

#### Acceptance Criteria

1. THE Linux_Capture SHALL implement X11 screen capture using XGetImage or XShmGetImage
2. THE Linux_Capture SHALL detect X11 display server via DISPLAY environment variable
3. THE Linux_Capture SHALL support shared memory extension (MIT-SHM) for performance
4. THE Linux_Capture SHALL capture the root window for full desktop
5. THE Linux_Capture SHALL support capturing specific screens in multi-head setups
6. THE Linux_Capture SHALL handle display resolution changes
7. THE Linux_Capture SHALL achieve minimum 30 fps with SHM extension
8. THE Linux_Capture SHALL properly release X11 resources

### Requirement 2: Screen Capture - Wayland/PipeWire

**User Story:** As a developer, I want Wayland capture, so that screen capture works on modern Linux desktops.

#### Acceptance Criteria

1. THE Linux_Capture SHALL implement PipeWire-based capture for Wayland
2. THE Linux_Capture SHALL use xdg-desktop-portal for screen capture permission
3. THE Linux_Capture SHALL handle user permission dialog
4. THE Linux_Capture SHALL support screen selection dialog
5. THE Linux_Capture SHALL handle PipeWire stream lifecycle
6. THE Linux_Capture SHALL achieve 60 fps on supported compositors
7. THE Linux_Capture SHALL detect Wayland session via WAYLAND_DISPLAY
8. THE Linux_Capture SHALL fall back to X11 capture via XWayland if needed

### Requirement 3: Multi-Monitor Support

**User Story:** As a developer, I want multi-monitor capture, so that all displays can be accessed.

#### Acceptance Criteria

1. THE Linux_Capture SHALL enumerate monitors via Xinerama/XRandR (X11) or portal (Wayland)
2. THE Linux_Capture SHALL provide monitor metadata: name, resolution, position
3. THE Linux_Capture SHALL support capturing individual monitors
4. THE Linux_Capture SHALL handle monitor hotplug events
5. THE Linux_Capture SHALL handle different DPI per monitor
6. THE Linux_Capture SHALL support virtual desktops/workspaces
7. THE Linux_Capture SHALL handle monitor arrangement changes
8. THE Linux_Capture SHALL detect primary monitor

### Requirement 4: Input Injection - X11/XTest

**User Story:** As a developer, I want X11 input injection, so that remote control works on X11 desktops.

#### Acceptance Criteria

1. THE Linux_Input SHALL inject input using XTest extension
2. THE Linux_Input SHALL support mouse move, click, and scroll
3. THE Linux_Input SHALL support keyboard key events
4. THE Linux_Input SHALL handle X11 keysym to keycode mapping
5. THE Linux_Input SHALL support modifier keys
6. THE Linux_Input SHALL handle keyboard layout detection
7. THE Linux_Input SHALL implement key release on session end
8. THE Linux_Input SHALL detect XTest extension availability

### Requirement 5: Input Injection - uinput

**User Story:** As a developer, I want uinput injection, so that input works with elevated privileges.

#### Acceptance Criteria

1. THE Linux_Input SHALL support uinput virtual device creation
2. THE Linux_Input SHALL create virtual mouse device
3. THE Linux_Input SHALL create virtual keyboard device
4. THE Linux_Input SHALL handle uinput permission requirements
5. THE Linux_Input SHALL support absolute and relative mouse positioning
6. THE Linux_Input SHALL support all standard keyboard keys
7. THE Linux_Input SHALL properly destroy virtual devices on cleanup
8. THE Linux_Input SHALL fall back to XTest when uinput unavailable

### Requirement 6: Input Injection - Wayland Limitations

**User Story:** As a developer, I want Wayland input handling, so that users understand limitations.

#### Acceptance Criteria

1. THE Linux_Input SHALL detect Wayland session and input limitations
2. THE Linux_Input SHALL use XWayland for input when available
3. THE Linux_Input SHALL document Wayland input restrictions
4. THE Linux_Input SHALL support compositor-specific input portals where available
5. THE Linux_Input SHALL provide clear error messages for unsupported operations
6. THE Linux_Input SHALL support libei (input emulation interface) when available
7. THE Linux_Input SHALL detect and report input capability status
8. THE Linux_Input SHALL recommend X11 session for full functionality

### Requirement 7: Secure Key Storage

**User Story:** As a developer, I want secure key storage, so that cryptographic keys are protected.

#### Acceptance Criteria

1. THE Linux_Platform SHALL support libsecret/GNOME Keyring for key storage
2. THE Linux_Platform SHALL support KWallet for KDE environments
3. THE Linux_Platform SHALL fall back to encrypted file storage
4. THE Linux_Platform SHALL use appropriate encryption for file-based storage
5. THE Linux_Platform SHALL handle keyring locked state
6. THE Linux_Platform SHALL support headless operation without keyring
7. THE Linux_Platform SHALL zeroize key material after use
8. THE Linux_Platform SHALL detect available secret service

### Requirement 8: systemd Integration

**User Story:** As a developer, I want systemd integration, so that the agent runs reliably as a service.

#### Acceptance Criteria

1. THE Linux_Platform SHALL provide systemd unit file template
2. THE Linux_Platform SHALL support user and system service modes
3. THE Linux_Platform SHALL implement sd_notify for startup notification
4. THE Linux_Platform SHALL support socket activation (optional)
5. THE Linux_Platform SHALL handle service restart and watchdog
6. THE Linux_Platform SHALL log to journald
7. THE Linux_Platform SHALL support service hardening options
8. THE Linux_Platform SHALL handle graphical session requirements

### Requirement 9: Clipboard Access

**User Story:** As a developer, I want clipboard access, so that clipboard sync works on Linux.

#### Acceptance Criteria

1. THE Linux_Platform SHALL access X11 clipboard via CLIPBOARD and PRIMARY selections
2. THE Linux_Platform SHALL support text (UTF8_STRING, STRING)
3. THE Linux_Platform SHALL support images (image/png)
4. THE Linux_Platform SHALL handle clipboard ownership
5. THE Linux_Platform SHALL detect clipboard changes
6. THE Linux_Platform SHALL support Wayland clipboard via portal
7. THE Linux_Platform SHALL handle clipboard format negotiation
8. THE Linux_Platform SHALL enforce clipboard size limits

### Requirement 10: Desktop Environment Detection

**User Story:** As a developer, I want DE detection, so that platform behavior adapts appropriately.

#### Acceptance Criteria

1. THE Linux_Platform SHALL detect desktop environment (GNOME, KDE, XFCE, etc.)
2. THE Linux_Platform SHALL detect display server (X11, Wayland, XWayland)
3. THE Linux_Platform SHALL detect session type (graphical, console)
4. THE Linux_Platform SHALL adapt behavior based on environment
5. THE Linux_Platform SHALL report environment in system information
6. THE Linux_Platform SHALL handle headless/server environments
7. THE Linux_Platform SHALL detect container/VM environment
8. THE Linux_Platform SHALL support multiple simultaneous sessions

### Requirement 11: System Information

**User Story:** As a developer, I want system information access, so that device details can be reported.

#### Acceptance Criteria

1. THE Linux_Platform SHALL report distribution name and version
2. THE Linux_Platform SHALL report kernel version
3. THE Linux_Platform SHALL report hostname
4. THE Linux_Platform SHALL report logged-in users
5. THE Linux_Platform SHALL report display configuration
6. THE Linux_Platform SHALL report network interface information
7. THE Linux_Platform SHALL detect architecture (x86_64, aarch64)
8. THE Linux_Platform SHALL report system uptime

### Requirement 12: Packaging Support

**User Story:** As a developer, I want packaging support, so that the agent can be distributed easily.

#### Acceptance Criteria

1. THE Linux_Platform SHALL support .deb package format
2. THE Linux_Platform SHALL support .rpm package format
3. THE Linux_Platform SHALL support AppImage format
4. THE Linux_Platform SHALL support Flatpak format (with limitations)
5. THE Linux_Platform SHALL provide package build scripts
6. THE Linux_Platform SHALL handle package dependencies correctly
7. THE Linux_Platform SHALL support post-install configuration
8. THE Linux_Platform SHALL support package signing
