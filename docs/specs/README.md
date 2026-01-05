# ZRC Specs (Comprehensive)

ZRC (Zippy Remote Control) is an open-source, cross-platform remote access platform (TeamViewer-class) with:
- **Preferred transport**: ZippyCoin/ZippyMesh handshake + mailbox routing
- **Fallbacks**: self-hosted rendezvous mailbox, direct IP, relay
- **Security**: identity pinning + transport pinning + E2EE session crypto above transport

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ZRC Architecture                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐        │
│  │   Desktop    │     │   Mobile     │     │     CLI      │        │
│  │  Controller  │     │  Controller  │     │  Controller  │        │
│  │ (zrc-desktop)│     │(android/ios) │     │(zrc-controller)       │
│  └──────┬───────┘     └──────┬───────┘     └──────┬───────┘        │
│         │                    │                    │                 │
│         └────────────────────┼────────────────────┘                 │
│                              │                                      │
│                    ┌─────────▼─────────┐                           │
│                    │     zrc-core      │                           │
│                    │ (business logic)  │                           │
│                    └─────────┬─────────┘                           │
│                              │                                      │
│         ┌────────────────────┼────────────────────┐                │
│         │                    │                    │                 │
│  ┌──────▼──────┐     ┌──────▼──────┐     ┌──────▼──────┐          │
│  │ zrc-crypto  │     │ zrc-proto   │     │zrc-transport│          │
│  │ (security)  │     │(wire format)│     │  (traits)   │          │
│  └─────────────┘     └─────────────┘     └─────────────┘          │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                         Services Layer                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐        │
│  │zrc-rendezvous│     │  zrc-relay   │     │  zrc-dirnode │        │
│  │(HTTP mailbox)│     │(NAT fallback)│     │ (directory)  │        │
│  └──────────────┘     └──────────────┘     └──────────────┘        │
│                                                                      │
├─────────────────────────────────────────────────────────────────────┤
│                          Host Layer                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│                    ┌─────────────────────┐                          │
│                    │     zrc-agent       │                          │
│                    │   (host daemon)     │                          │
│                    └──────────┬──────────┘                          │
│                               │                                      │
│    ┌──────────────────────────┼──────────────────────────┐         │
│    │                          │                          │          │
│  ┌─▼────────────┐   ┌────────▼────────┐   ┌────────────▼─┐        │
│  │platform-win  │   │  platform-mac   │   │platform-linux│        │
│  │(capture/input│   │(capture/input)  │   │(capture/input│        │
│  └──────────────┘   └─────────────────┘   └──────────────┘        │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Specifications

### Core Libraries
| Component | Purpose | Status |
|-----------|---------|--------|
| [zrc-proto](zrc-proto/) | Protobuf wire formats for all messages | Spec Complete |
| [zrc-crypto](zrc-crypto/) | Cryptographic primitives (Ed25519, X25519, ChaCha20) | Spec Complete |
| [zrc-core](zrc-core/) | Business logic, state machines, policies | Spec Complete |
| [zrc-transport](../../../.kiro/specs/zrc-transport/) | Transport traits and framing | Spec Complete |

### Services
| Component | Purpose | Status |
|-----------|---------|--------|
| [zrc-rendezvous](zrc-rendezvous/) | Self-hosted HTTP mailbox server | Spec Complete |
| [zrc-relay](zrc-relay/) | QUIC relay for NAT traversal | Spec Complete |
| [zrc-dirnode](zrc-dirnode/) | Home-hostable directory node | Spec Complete |
| [zrc-admin-console](zrc-admin-console/) | Web admin UI | Spec Complete |

### Executables
| Component | Purpose | Status |
|-----------|---------|--------|
| [zrc-agent](zrc-agent/) | Host daemon/service | Spec Complete |
| [zrc-controller](zrc-controller/) | CLI controller tool | Spec Complete |
| [zrc-desktop](zrc-desktop/) | GUI viewer/controller | Spec Complete |

### Platform Crates
| Component | Purpose | Status |
|-----------|---------|--------|
| [zrc-platform-win](zrc-platform-win/) | Windows capture/input | Spec Complete |
| [zrc-platform-mac](zrc-platform-mac/) | macOS capture/input | Spec Complete |
| [zrc-platform-linux](zrc-platform-linux/) | Linux capture/input | Spec Complete |
| [zrc-platform-android](zrc-platform-android/) | Android controller/host | Spec Complete |
| [zrc-platform-ios](zrc-platform-ios/) | iOS controller | Spec Complete |

### Operations & Tooling
| Component | Purpose | Status |
|-----------|---------|--------|
| [zrc-ci](zrc-ci/) | Build, signing, releases | Spec Complete |
| [zrc-updater](zrc-updater/) | Secure auto-update | Spec Complete |
| [zrc-security](zrc-security/) | Threat model, security controls | Spec Complete |

## Terminology

- **Operator**: Controller user initiating session
- **Device/Host**: Machine being controlled (runs agent)
- **Pairing**: Establishing trust (pin keys, permissions)
- **Ticket**: Short-lived capability token for unattended sessions
- **Directory**: Discovery/pairing helper (never trusted for confidentiality)
- **Envelope**: Signed and sealed message container
- **SAS**: Short Authentication String for MITM detection
- **Transport**: Communication mechanism (mesh, rendezvous, direct, relay)

## Security Model

### Trust Boundaries
1. **Endpoints (Agent/Controller)**: Fully trusted, hold private keys
2. **Directory/Rendezvous**: Semi-trusted for availability, NOT for confidentiality
3. **Relay**: Untrusted, forwards encrypted traffic only
4. **Network**: Untrusted, all traffic encrypted

### Key Security Properties
- **Identity Pinning**: After pairing, only accept known public keys
- **Transport Pinning**: QUIC certificate fingerprint verification
- **E2EE**: Session encryption above transport layer
- **Short-lived Tickets**: Minutes, not days
- **Consent**: Visible indicator, user approval for attended sessions

## Implementation Phases

### Phase 0: Architecture Spike (2-4 weeks)
- Choose transport (WebRTC vs QUIC) ✓ QUIC chosen
- Prove: capture → encode → transmit → render → input loop
- Windows + macOS or Windows + Linux first

### Phase 1: MVP (Tier A basics)
- Host agent + controller (desktop)
- Rendezvous + relay
- E2EE session + pairing
- Clipboard + file transfer
- Self-hostable docker-compose release

### Phase 2: Hardening + Usability
- Unattended access with device trust + approvals
- Session recording (client-side encrypted)
- Admin console (users/devices/groups)
- Packaging polish (MSI/pkg/deb)

### Phase 3: Mobile Expansion
- Android host + controller
- iOS screen share (ReplayKit) + annotation

### Phase 4: Enterprise Controls
- OIDC/SAML SSO, SCIM provisioning
- Policy engine, audit exports, integrations

## Differentiators

1. **Mesh-First Session Initiation**: Device pairing + session capability tokens signed via ZippyCoin identity layer
2. **Split-Control Mode**: Allow "view-only + guided overlay" by default; require higher trust to enable input
3. **Private Relays**: User can run relays on cheap VPS nodes; auto-select nearest
4. **Forensics-Friendly Audit**: Cryptographic session receipts (who, when, what permissions) without logging content
5. **Self-Hostable**: Single-binary services, minimal dependencies, runs on Raspberry Pi

## Getting Started

See individual component specs for detailed requirements, design, and implementation tasks.

For the comprehensive Kiro-style specs with EARS requirements and correctness properties, see:
- `.kiro/specs/zrc-*/requirements.md` - Detailed requirements
- `.kiro/specs/zrc-*/design.md` - Architecture and design
- `.kiro/specs/zrc-*/tasks.md` - Implementation tasks

## Key Documents

- **[COMPATIBILITY_MATRICES.md](COMPATIBILITY_MATRICES.md)** - Platform features, network connectivity, protocol versioning, data retention/privacy, and acceptance test checklists
- **[program/tasks.md](program/tasks.md)** - Master milestone tracker with Phase 0 security blockers

## Architecture Decision: WebRTC-first Hybrid

The project uses a WebRTC-first hybrid architecture:
- **Control Plane (Rust)**: Identity keys, pairing, invite-only discovery, session authorization tickets, directory records, audit events
- **Media Plane (WebRTC)**: Video/audio streams via libwebrtc (C++ engine via FFI), DataChannels for control/clipboard/files
- **Fallback Relay**: coturn for TURN relay (self-hostable)

This approach leverages WebRTC's battle-tested NAT traversal (ICE), congestion control, and codec negotiation while keeping the security model in Rust.

### Phase 0 Security Blockers (MUST complete before Phase 1)
1. **Identity-bound DTLS cert** - Prevents signaling MITM attacks
2. **Replay protection + deterministic nonces** - Prevents packet replay attacks
3. **Transport downgrade prevention** - Prevents forced fallback to weaker transports
