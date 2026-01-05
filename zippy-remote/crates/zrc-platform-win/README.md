# zrc-platform-win

Windows platform abstraction layer for Zippy Remote Control (ZRC).

## Status: ✅ Complete

All implementation tasks and tests are complete. The crate compiles successfully and is ready for integration.

## Features

- **Screen Capture**: GDI, DXGI Desktop Duplication, and WGC (Windows Graphics Capture) support
- **Input Injection**: Mouse and keyboard input via SendInput API
- **Special Keys**: Alt+Tab, Win+L, Ctrl+Shift+Esc, and Ctrl+Alt+Del support
- **Windows Service**: Full service lifecycle and session change handling
- **DPAPI Key Storage**: Secure key storage using Windows Data Protection API
- **Clipboard Access**: Text and image clipboard operations
- **UAC Handling**: Secure desktop detection and switching
- **System Information**: Windows version, monitors, network adapters, VM detection

## Usage

```rust
use zrc_platform_win::WinPlatform;
use zrc_core::platform::HostPlatform;

let platform = WinPlatform::new()?;

// Capture a frame
let frame = platform.capture_frame().await?;

// Inject input
platform.apply_input(InputEvent::MouseMove { x: 100, y: 100 }).await?;
```

## Testing

Run all tests:
```bash
cargo test -p zrc-platform-win --lib
```

Run specific test suites:
```bash
# Validation tests
cargo test -p zrc-platform-win --lib validation

# Property tests
cargo test -p zrc-platform-win --lib property
```

## Documentation

- [VALIDATION.md](VALIDATION.md) - Component validation report
- [TEST_SUMMARY.md](TEST_SUMMARY.md) - Test coverage summary
- [COMPLETION_STATUS.md](COMPLETION_STATUS.md) - Completion status

## Requirements

- Windows 7+ (GDI capture)
- Windows 8+ (DXGI capture)
- Windows 10 1903+ (WGC capture)

## License

Apache-2.0 OR MIT
