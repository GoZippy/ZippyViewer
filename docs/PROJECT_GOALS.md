# ZRC (Zippy Remote Control) - Project Goals

## Overview
Build an open-source, self-hostable remote desktop platform (TeamViewer-class) with privacy-first defaults, using ZippyCoin mesh for identity and session initiation.

## Core Goals

### Primary Objectives
1. **Open Alternative**: Create a TeamViewer-class remote desktop solution that can be self-hosted
2. **Privacy-First**: Use ZippyCoin mesh layer for identity, pairing, and session initiation
3. **Security**: End-to-end encryption, identity pinning, capability-based access control
4. **Cross-Platform**: Native support for Windows, macOS, Linux, Android, iOS
5. **Decentralized**: Support for home-hostable directory nodes and mesh-first connectivity

### Key Features

#### Tier A - Core Remote Session (MVP)
- Remote view/control (multi-monitor, scaling, clipboard, audio)
- Attended + unattended access
- Chat, file transfer, session recording
- Reboot/reconnect, UAC/admin elevation handling
- Address book / device list, grouping, tagging

#### Tier B - Admin + Enterprise Controls
- Org/tenant management, roles, audit logs, policies
- 2FA/SSO (OIDC/SAML), device trust, conditional access
- Mass deployment, custom branding/modules, update rings

#### Tier C - Platform Extras (Future)
- Asset inventory, patching, scripting/automation
- Mobile device workflows, AR/annotation
- IoT/embedded tooling

## Architecture Principles

### Transport Strategy
- **Preferred**: Zippy mesh handshake + direct P2P (QUIC/WebRTC)
- **Fallback**: Self-hosted rendezvous/relay server
- **Last Resort**: Direct IP (LAN/VPN/port-forward)
- **Security**: E2EE layer above transport (relays can't decrypt)

### Security Model
- **Identity**: Device/User keys (Ed25519 for signing, X25519 for KEX)
- **Pairing**: Invite-only by default, temporary discoverability optional
- **MITM Protection**: SAS (Short Authentication String) verification, PAKE (future)
- **Access Control**: Capability tokens (SessionTicket) for unattended sessions
- **Key Pinning**: Trust-on-first-use with out-of-band fingerprint verification

### Discovery Modes
1. **Invite-Only (Default)**: QR/link/file with device fingerprint
2. **Temporary Discoverability**: Short-lived code-based discovery (5-15 min TTL)

### Connection Methods
1. **Mesh-First (Preferred)**: Zippy mesh mailbox for session initiation
2. **Self-Hosted Server**: HTTP mailbox (rendezvous) as fallback
3. **Direct IP**: LAN/VPN when available

## Technology Stack

### Core Language
- **Rust**: Primary language for security-sensitive code, cross-platform compatibility

### Key Libraries
- **prost**: Protobuf code generation
- **ed25519-dalek**: Ed25519 signatures
- **x25519-dalek**: X25519 key exchange
- **chacha20poly1305**: AEAD encryption
- **quinn**: QUIC transport
- **tokio**: Async runtime

### Platform-Specific
- **Windows**: Desktop Duplication API, SendInput
- **macOS**: ScreenCaptureKit, CGEvent
- **Linux**: PipeWire (Wayland), X11 fallback
- **Android**: MediaProjection, AccessibilityService
- **iOS**: ReplayKit (screen share only, no full control)

## Implementation Phases

### Phase 0 - Architecture Spike (2-4 weeks)
- Choose transport (WebRTC vs QUIC)
- Prove: capture → encode → transmit → render → input loop
- Windows + macOS or Windows + Linux first

### Phase 1 - MVP (Tier A basics)
- Host agent + controller (desktop)
- Rendezvous + relay
- E2EE session + pairing
- Clipboard + file transfer
- Self-hostable docker-compose release

### Phase 2 - Hardening + Usability
- Unattended access with device trust + approvals
- Session recording (client-side encrypted)
- Admin console (users/devices/groups)
- Packaging polish (MSI/pkg/deb)

### Phase 3 - Mobile Expansion
- Android host + controller
- iOS screen share (ReplayKit) + annotation

### Phase 4 - Enterprise Controls
- OIDC/SAML SSO, SCIM provisioning
- Policy engine, audit exports, integrations

## Repository Structure

```
zippy-remote/
  Cargo.toml (workspace)
  crates/
    zrc-proto/          # protobuf schemas + generated code (prost)
    zrc-crypto/         # keys, envelopes, SAS, hybrid KEX, zeroize
    zrc-core/           # pairing/session state machines, policies, tickets
    zrc-transport/      # traits + common framing; no OS deps
    zrc-mesh/           # Zippy mesh adapter (preferred initiation path)
    zrc-rendezvous/     # server (signaling, presence) - untrusted
    zrc-relay/          # relay (TURN-like or custom forwarder)
    zrc-dirnode/        # home-hostable DHT/mailbox node
    zrc-agent/          # host agent orchestrator (calls platform modules)
    zrc-controller/     # desktop controller app (native UI)
    zrc-platform-win/   # capture/input/service wrappers
    zrc-platform-mac/
    zrc-platform-linux/
    zrc-platform-android/  # JNI bridge + minimal Rust wrappers
    zrc-platform-ios/      # UniFFI/FFI + minimal Rust wrappers
  apps/
    desktop-controller/   # (optional) separate UI crate using egui/wgpu
    admin-console/        # (optional) web UI not required for MVP
```

## Security Requirements

### Must-Have Controls
- Mutual auth (operator ↔ device), not just a password
- Pairing/trust model for unattended access
- Visible in-session indicator + consent UX for attended sessions
- Short-lived session capabilities (minutes, not days)
- Signed updates (auto-update is the #1 real-world compromise vector)

### Threat Model
- MITM on directory/rendezvous/relay
- Impersonation of device/operator
- Replay attacks (pair/session)
- Downgrade attacks (transport/cipher suite)
- Key compromise and recovery
- Metadata privacy (who connects to whom, when)

## Differentiators

1. **Mesh-First Session Initiation**: Device pairing + session capability tokens signed via ZippyCoin identity layer
2. **Split-Control Mode**: Allow "view-only + guided overlay" by default; require higher trust to enable input
3. **Private Relays**: User can run relays on cheap VPS nodes; auto-select nearest
4. **Forensics-Friendly Audit**: Cryptographic session receipts (who, when, what permissions) without logging content

