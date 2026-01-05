# Architecture Overview

ZRC (Zippy Remote Control) is designed as a privacy-first, self-hostable remote desktop platform.

## System Components

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
│  │    (QUIC forwarding, E2E encrypted)        │                 │
│  └────────────────────────────────────────────┘                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Connection Flow

1. **Discovery**: Devices register with the rendezvous server
2. **Pairing**: One-time secure pairing establishes trust
3. **Session**: Controller initiates session via rendezvous
4. **Connection**: Direct P2P via QUIC, or relay fallback
5. **Media**: Screen, input, and clipboard streaming

## Security Model

- **End-to-End Encryption**: All sessions encrypted with ChaCha20-Poly1305
- **Mutual Authentication**: Both parties verify each other via Ed25519
- **Zero-Knowledge Relay**: Relay servers never see plaintext
- **Capability Tokens**: Time-limited, permission-scoped access

## Crate Dependencies

```
zrc-proto (protobuf schemas)
    ↓
zrc-crypto (Ed25519, X25519, ChaCha20)
    ↓
zrc-core (state machines, policies)
    ↓
zrc-transport (QUIC, framing)
    ↓
┌───────────────┬───────────────┬───────────────┐
│  zrc-agent    │ zrc-controller│  zrc-desktop  │
└───────────────┴───────────────┴───────────────┘
```

## Platform Support

| Platform | Controller | Agent | Notes |
|----------|------------|-------|-------|
| Windows x64 | ✅ | ✅ | Full support |
| macOS Universal | ✅ | ✅ | Full support |
| Linux x64/ARM64 | ✅ | ✅ | Full support |
| Android | ✅ | ✅ | Mobile support |
| iOS | ✅ | View only | Platform restrictions |
