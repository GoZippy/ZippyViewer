# zrc-platform-linux

Linux platform abstraction layer for ZRC (Zippy Remote Control).

## Overview

This crate provides Linux-specific functionality including:
- Screen capture via X11 SHM/Basic and PipeWire/Portal (Wayland)
- Input injection via XTest (X11) and uinput (Wayland/privileged)
- Secret Service storage for secure key management
- systemd service integration
- Clipboard access via X11 selections and Wayland portals
- Desktop environment detection

## Requirements

- Linux with X11 or Wayland session
- X11: MIT-SHM extension (for fast capture) or basic X11
- Wayland: xdg-desktop-portal and PipeWire (for capture)
- Input injection: XTest extension (X11) or uinput access (Wayland)

## Features

### Capture Backends

1. **PipeWire/Portal** (Wayland, preferred)
   - Requires xdg-desktop-portal and PipeWire
   - User permission via portal dialog

2. **X11 SHM** (X11, preferred)
   - Fast shared memory capture
   - Requires MIT-SHM extension

3. **X11 Basic** (X11, fallback)
   - Universal but slower
   - Uses GetImage

### Input Injection

1. **XTest** (X11)
   - Works on X11 and XWayland
   - No special privileges needed

2. **uinput** (Wayland/privileged)
   - Requires /dev/uinput access
   - Needs udev rules or elevated privileges

## Building

```bash
cargo build --package zrc-platform-linux

# With optional features
cargo build --package zrc-platform-linux --features pipewire,uinput,secret-service,systemd
```

## Testing

Note: Full testing requires a Linux environment with X11 or Wayland session.

## Implementation Status

All core structures and interfaces are implemented. See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for detailed status.

**Fully Implemented:**
- Desktop environment detection
- System information collection
- Wayland input status detection
- File key store (fallback)
- systemd service integration

**Structure Complete (API Integration Needed):**
- Screen capture (X11 SHM/Basic, PipeWire)
- Input injection (XTest, uinput)
- Secret storage (Secret Service)
- Clipboard access (X11, Wayland portal)

See [COMPLETION_STATUS.md](COMPLETION_STATUS.md) for complete task status.
