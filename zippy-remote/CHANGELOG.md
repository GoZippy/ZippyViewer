# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Core Infrastructure
- **zrc-proto**: Protocol buffer definitions for all ZRC messages
- **zrc-crypto**: Cryptographic primitives including Ed25519 signatures, X25519 key exchange, ChaCha20-Poly1305 AEAD
- **zrc-core**: Pairing and session state machines with policy enforcement
- **zrc-transport**: Transport abstractions and common framing utilities

#### Server Components
- **zrc-rendezvous**: Signaling and presence server with HTTP mailbox
- **zrc-relay**: QUIC relay server for NAT traversal with bandwidth limiting
- **zrc-dirnode**: Self-hostable directory node for device discovery

#### Client Applications
- **zrc-agent**: Host agent daemon with platform-specific capture and input
- **zrc-controller**: Session controller logic for operators
- **zrc-desktop**: Desktop viewer application with egui-based UI

#### Platform Support
- **zrc-platform-win**: Windows implementation (Desktop Duplication API, SendInput)
- **zrc-platform-mac**: macOS implementation (ScreenCaptureKit, CGEvent)
- **zrc-platform-linux**: Linux implementation (PipeWire, X11/Wayland)
- **zrc-platform-android**: Android implementation (MediaProjection, AccessibilityService)
- **zrc-platform-ios**: iOS implementation (ReplayKit - view only)

#### Security & Administration
- **zrc-security**: Comprehensive security controls including audit logging, access control, threat detection
- **zrc-admin-console**: Web-based administration interface
- **zrc-updater**: Secure auto-update system with rollback support

#### Infrastructure
- GitHub Actions workflows for CI/CD
- Multi-platform builds (Windows, macOS, Linux, Android, iOS)
- Security audit integration with cargo-audit
- Code coverage reporting

### Security
- End-to-end encryption for all sessions
- Mutual authentication between operators and devices
- Short-lived capability tokens
- Signed updates to prevent supply-chain attacks
- Audit logging for compliance

### Documentation
- Comprehensive README with architecture overview
- API documentation (rustdoc)
- Deployment guides for self-hosting

## [0.1.0] - TBD

Initial release.

---

## Release Process

1. Update version numbers in all `Cargo.toml` files
2. Update this CHANGELOG with release date
3. Create git tag: `git tag -a v0.1.0 -m "Release 0.1.0"`
4. Push tag: `git push origin v0.1.0`
5. GitHub Actions will automatically build and publish releases

## Version Numbering

- **MAJOR**: Breaking changes to public APIs or protocols
- **MINOR**: New features, backwards compatible
- **PATCH**: Bug fixes, security updates
