# ZRC (Zippy Remote Control)

[![Build](https://github.com/GoZippy/ZippyViewer/actions/workflows/build.yml/badge.svg)](https://github.com/GoZippy/ZippyViewer/actions/workflows/build.yml)
[![PR Validation](https://github.com/GoZippy/ZippyViewer/actions/workflows/pr.yml/badge.svg)](https://github.com/GoZippy/ZippyViewer/actions/workflows/pr.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)

An open-source, self-hostable remote desktop platform with privacy-first defaults and end-to-end encryption.

## Overview

ZRC provides TeamViewer-class remote desktop functionality that you can fully self-host. Key features include:

- **End-to-End Encryption**: All sessions are encrypted; relay servers never see plaintext
- **Privacy-First**: Mesh-based identity and session initiation via ZippyCoin protocol
- **Cross-Platform**: Native support for Windows, macOS, Linux, Android, and iOS
- **Self-Hostable**: Run your own rendezvous, relay, and directory servers
- **Security-Focused**: Capability-based access control, mutual authentication, signed updates

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ZRC Architecture                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    P2P/QUIC    ┌──────────────┐               │
│  │  Controller  │ ◄────────────► │    Agent     │               │
│  │  (Operator)  │                │   (Device)   │               │
│  └──────┬───────┘                └──────┬───────┘               │
│         │                               │                        │
│         │ Signaling                     │ Signaling              │
│         ▼                               ▼                        │
│  ┌────────────────────────────────────────────┐                 │
│  │           Rendezvous Server                │                 │
│  │     (Session initiation, mailbox)          │                 │
│  └────────────────────────────────────────────┘                 │
│                         │                                        │
│                         │ Fallback                               │
│                         ▼                                        │
│  ┌────────────────────────────────────────────┐                 │
│  │             Relay Server                   │                 │
│  │    (QUIC forwarding, no plaintext)         │                 │
│  └────────────────────────────────────────────┘                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Description |
|-------|-------------|
| `zrc-proto` | Protobuf schemas and generated code |
| `zrc-crypto` | Cryptographic primitives (Ed25519, X25519, ChaCha20-Poly1305) |
| `zrc-core` | Pairing and session state machines, policies |
| `zrc-transport` | Transport traits and common framing |
| `zrc-rendezvous` | Signaling and presence server |
| `zrc-relay` | QUIC relay for NAT traversal |
| `zrc-dirnode` | Self-hostable directory node |
| `zrc-agent` | Host agent daemon |
| `zrc-controller` | Session controller logic |
| `zrc-desktop` | Desktop viewer application |
| `zrc-updater` | Secure auto-update system |
| `zrc-admin-console` | Web-based administration |
| `zrc-security` | Audit logging and access control |
| `zrc-platform-*` | Platform-specific implementations |

## Quick Start

### Prerequisites

- Rust 1.75+ (stable)
- Platform-specific dependencies (see below)

### Building

```bash
# Clone the repository
git clone https://github.com/GoZippy/ZippyViewer.git
cd ZippyViewer/zippy-remote

# Build all cross-platform crates
cargo build --release

# Build specific binaries
cargo build --release --bin zrc-desktop
cargo build --release --bin zrc-agent
cargo build --release --bin zrc-relay
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p zrc-core
cargo test -p zrc-crypto

# Run integration tests
cargo test -p zrc-core --test integration
```

### Running Components

```bash
# Start the relay server
cargo run --bin zrc-relay -- --config relay.toml

# Start the rendezvous server
cargo run --bin zrc-rendezvous -- --config rendezvous.toml

# Start the desktop viewer
cargo run --bin zrc-desktop

# Start the host agent
cargo run --bin zrc-agent -- --foreground
```

## Configuration

### Relay Server

See `crates/zrc-relay/examples/relay.toml` for configuration options.

```toml
[server]
listen_addr = "0.0.0.0"
listen_port = 4433
max_allocations = 10000

[limits]
default_bandwidth = 10485760  # 10 MB/s
default_quota = 1073741824    # 1 GB

[security]
rate_limit_per_ip = 10
```

### Rendezvous Server

See `crates/zrc-rendezvous/examples/rendezvous.toml` for configuration options.

## Security

### Threat Model

ZRC is designed to be secure against:

- **Man-in-the-middle attacks** on signaling servers
- **Impersonation** of devices or operators
- **Replay attacks** on pairing and sessions
- **Downgrade attacks** on transport/cipher suites
- **Relay snooping** (all data is E2E encrypted)

### Key Principles

1. **Mutual Authentication**: Both parties verify each other's identity
2. **Short-Lived Sessions**: Capability tokens expire quickly
3. **Explicit Consent**: Attended sessions require user approval
4. **Signed Updates**: Prevents supply-chain attacks

## Platform Support

| Platform | Controller | Agent | Status |
|----------|------------|-------|--------|
| Windows x64 | ✅ | ✅ | Production |
| macOS (Universal) | ✅ | ✅ | Production |
| Linux x64 | ✅ | ✅ | Production |
| Linux ARM64 | ✅ | ✅ | Production |
| Android | ✅ | ✅ | Beta |
| iOS | ✅ | View Only | Beta |

## Development

### Project Structure

```
zippy-remote/
├── Cargo.toml          # Workspace manifest
├── crates/
│   ├── zrc-proto/      # Protocol definitions
│   ├── zrc-crypto/     # Cryptographic operations
│   ├── zrc-core/       # Core state machines
│   ├── zrc-transport/  # Transport abstractions
│   ├── zrc-*           # Other crates
└── target/             # Build output
```

### Code Style

- Follow Rust API guidelines
- Use `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Add tests for new functionality

### CI/CD

- **PR Validation**: Lint, test, security audit, coverage
- **Build**: Multi-platform release builds
- **Nightly**: Extended testing and benchmarks

## Documentation

Generate API documentation:

```bash
cargo doc --open --no-deps
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

## Acknowledgments

- [Quinn](https://github.com/quinn-rs/quinn) - QUIC implementation
- [Ed25519-Dalek](https://github.com/dalek-cryptography/curve25519-dalek) - Cryptographic primitives
- [Tokio](https://tokio.rs/) - Async runtime
