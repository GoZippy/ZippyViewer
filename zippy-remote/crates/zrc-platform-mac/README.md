# zrc-platform-mac

macOS platform abstraction layer for ZRC (Zippy Remote Control).

## Overview

This crate provides macOS-specific functionality including:
- Screen capture via ScreenCaptureKit (macOS 12.3+) and CGDisplayStream fallback
- Input injection via CGEvent (mouse and keyboard)
- Keychain storage for secure key management
- LaunchAgent/LaunchDaemon service integration
- Clipboard access via NSPasteboard
- System information collection

## Requirements

- macOS 10.15+ (baseline)
- macOS 12.3+ (for ScreenCaptureKit)
- Screen recording permission
- Accessibility permission (for input injection)

## Code Signing

See [CODE_SIGNING.md](CODE_SIGNING.md) for complete code signing and notarization documentation.

### Quick Reference

- **Hardened Runtime**: Required for distribution
- **Screen Recording**: Runtime permission (no entitlement needed)
- **Accessibility**: Runtime permission (no entitlement needed)
- **Notarization**: Required for distribution outside Mac App Store

## Building

```bash
cargo build --package zrc-platform-mac
```

## Testing

Note: Full testing requires a macOS environment. Some APIs require Objective-C bindings that are not yet fully implemented.

## Implementation Status

All core structures and interfaces are implemented. See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for detailed status.

**Fully Implemented:**
- Launchd service integration
- System information collection
- Code signing documentation

**Structure Complete (API Integration Needed):**
- Screen capture (ScreenCaptureKit/CGDisplayStream)
- Input injection (CGEvent APIs)
- Keychain storage (security-framework)
- Clipboard access (NSPasteboard)

See [COMPLETION_STATUS.md](COMPLETION_STATUS.md) for complete task status.
