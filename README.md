# ZippyViewer

A comprehensive remote desktop and device control platform built with security and privacy at its core.

## Project Structure

```
ZippyViewer/
├── zippy-remote/       # Core Rust implementation (ZRC - Zippy Remote Control)
├── android-app/        # Android application
├── coturn-setup/       # TURN/STUN server configuration
└── docs/               # Documentation and specifications
```

## Components

### ZRC (Zippy Remote Control)

The core platform implementation in Rust. Provides:

- **End-to-End Encryption** - All sessions encrypted; relay servers never see plaintext
- **Cross-Platform** - Windows, macOS, Linux, Android, iOS
- **Self-Hostable** - Run your own infrastructure
- **Privacy-First** - No telemetry, no tracking

See [zippy-remote/README.md](zippy-remote/README.md) for detailed documentation.

### Android App

Native Android application for remote device access. See [android-app/README.md](android-app/README.md).

### TURN/STUN Infrastructure

Production-ready Coturn configuration for NAT traversal. See [coturn-setup/README.md](coturn-setup/README.md).

## Getting Started

### Prerequisites

- Rust 1.75+ (stable)
- Protocol Buffers compiler (`protoc`)
- Platform-specific SDKs as needed

### Quick Build

```bash
cd zippy-remote
cargo build --release
```

### Running Tests

```bash
cd zippy-remote
cargo test
```

## Documentation

- [Project Goals](docs/PROJECT_GOALS.md)
- [Architecture Specs](docs/specs/)
- [Contributing](zippy-remote/CONTRIBUTING.md)

## Status

🚧 **Pre-release** - Under active development. Not yet ready for production use.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
