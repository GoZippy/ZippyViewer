# ZRC Threat Model

## Overview

This document defines the threat model for the Zippy Remote Control (ZRC) system. It identifies threat actors, trust boundaries, attack vectors, and the mitigations implemented to address each threat.

**Requirements:** 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7

## Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ZRC Trust Boundaries                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  TRUSTED COMPONENTS                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  • Agent (device) - Local endpoint with access to device resources  │   │
│  │  • Controller (operator) - Remote endpoint initiating connections   │   │
│  │  • OS Keystore - Secure key storage (DPAPI/Keychain/Secret Service)  │   │
│  │  • Local Network (optional) - Direct LAN connections                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  UNTRUSTED COMPONENTS                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  • Directory Server - Public key directory (may be malicious)       │   │
│  │  • Rendezvous Server - Signaling server (may be malicious)          │   │
│  │  • Relay Server - TURN/forwarding server (may be malicious)         │   │
│  │  • Network Infrastructure - Routers, ISPs, etc.                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  TRUST BOUNDARY CROSSINGS                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Agent ←→ Network ←→ Controller (E2EE required)                     │   │
│  │  Agent → OS Keystore (trusted for key storage)                       │   │
│  │  Controller → OS Keystore (trusted for key storage)                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Threat Actors

### 1. Network Attacker (MITM)
**Description:** An attacker positioned on the network path between agent and controller, capable of intercepting, modifying, or injecting network traffic.

**Capabilities:**
- Intercept network traffic
- Modify packets in transit
- Inject malicious packets
- Perform man-in-the-middle attacks
- Observe traffic patterns and metadata

**Likelihood:** High (public networks, compromised routers)
**Impact:** Critical (full session compromise if successful)

### 2. Malicious Directory Node
**Description:** A compromised or malicious directory server that provides public key information.

**Capabilities:**
- Provide incorrect public keys
- Perform key substitution attacks
- Enumerate device/operator identities
- Track pairing relationships

**Likelihood:** Medium (depends on directory trust model)
**Impact:** High (enables MITM if identity pinning fails)

### 3. Malicious Rendezvous Server
**Description:** A compromised or malicious signaling/rendezvous server.

**Capabilities:**
- Modify signaling messages
- Perform key substitution during handshake
- Enumerate active sessions
- Track connection patterns

**Likelihood:** Medium (if using untrusted rendezvous)
**Impact:** High (enables MITM if identity pinning fails)

### 4. Malicious Relay Server
**Description:** A compromised or malicious TURN/relay server.

**Capabilities:**
- Observe encrypted traffic (cannot decrypt due to E2EE)
- Perform traffic analysis
- Drop or delay packets
- Track connection metadata

**Likelihood:** Medium (if using untrusted relay)
**Impact:** Medium (privacy impact, but cannot decrypt)

### 5. Compromised Endpoint
**Description:** An attacker who has compromised either the agent or controller device.

**Capabilities:**
- Access stored keys (if keystore compromised)
- Access active session keys
- Impersonate the compromised endpoint
- Access device resources

**Likelihood:** Low (requires device compromise)
**Impact:** Critical (full endpoint compromise)

## Data Flow Diagrams

### Encryption Boundaries

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Encryption Boundaries                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Agent (Trusted)                    Network (Untrusted)    Controller (Trust)│
│  ┌──────────────┐                                                          │
│  │ Private Keys │                                                          │
│  │ (Keystore)   │                                                          │
│  └──────┬───────┘                                                          │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────────┐                                                       │
│  │ E2EE Encryption │ ────────[Encrypted]───────────► ┌──────────────────┐ │
│  │ (Session Keys)   │                                 │ E2EE Decryption  │ │
│  └──────────────────┘                                 │ (Session Keys)   │ │
│         │                                               └──────┬───────────┘ │
│         │                                                       │            │
│         │                                                       ▼            │
│         │                                               ┌──────────────┐     │
│         │                                               │ Private Keys │     │
│         │                                               │ (Keystore)   │     │
│         │                                               └──────────────┘     │
│         │                                                                   │
│  ┌──────▼───────┐                                                           │
│  │ Transport    │ ────────[TLS/QUIC]───────────► ┌──────────────────┐       │
│  │ Encryption   │                                 │ Transport        │       │
│  │ (TLS/QUIC)  │                                 │ Encryption       │       │
│  └──────────────┘                                 └──────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Material Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Key Material Flow                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Identity Keys (Ed25519/X25519)                                          │
│     ┌─────────────┐                                                         │
│     │  Generated  │                                                         │
│     │  at device  │                                                         │
│     └──────┬──────┘                                                         │
│            │                                                                 │
│            ▼                                                                 │
│     ┌─────────────┐                                                         │
│     │  Stored in  │                                                         │
│     │  OS Keystore│                                                         │
│     └──────┬──────┘                                                         │
│            │                                                                 │
│            ▼                                                                 │
│     ┌─────────────┐                                                         │
│     │  Pinned     │                                                         │
│     │  after      │                                                         │
│     │  pairing    │                                                         │
│     └─────────────┘                                                         │
│                                                                              │
│  2. Session Keys (HKDF-derived)                                             │
│     ┌─────────────┐                                                         │
│     │  Shared     │                                                         │
│     │  Secret    │                                                         │
│     │  (X25519)  │                                                         │
│     └──────┬──────┘                                                         │
│            │                                                                 │
│            ▼                                                                 │
│     ┌─────────────┐                                                         │
│     │  HKDF       │                                                         │
│     │  Derivation │                                                         │
│     └──────┬──────┘                                                         │
│            │                                                                 │
│            ▼                                                                 │
│     ┌─────────────┐                                                         │
│     │  Separate   │                                                         │
│     │  keys per  │                                                         │
│     │  direction │                                                         │
│     │  & channel │                                                         │
│     └─────────────┘                                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Cryptographic Operations

| Operation | Purpose | Algorithm | Location |
|-----------|---------|-----------|----------|
| Identity Signing | Device/operator authentication | Ed25519 | zrc-crypto |
| Key Exchange | Session key establishment | X25519 | zrc-crypto |
| Session Encryption | E2EE data protection | ChaCha20Poly1305 | zrc-crypto |
| Key Derivation | Session key generation | HKDF-SHA256 | zrc-security |
| SAS Computation | MITM detection | SHA-256 | zrc-security |
| Audit Signing | Log integrity | Ed25519 | zrc-security |
| Replay Protection | Sequence tracking | Sliding window | zrc-security |

## Threat Mitigations

### Threat 1: Network Attacker (MITM)

**Threat:** Intercept and modify traffic between agent and controller.

**Mitigations:**
1. **E2EE (End-to-End Encryption)** - All session data encrypted with session-specific keys
   - **Requirement:** 7.1
   - **Effectiveness:** High - Prevents decryption even if transport is compromised
2. **Identity Pinning** - Verify peer identity keys on every connection
   - **Requirement:** 2.1, 2.2
   - **Effectiveness:** High - Detects key substitution attacks
3. **SAS Verification** - Short Authentication String for first-contact verification
   - **Requirement:** 2.3, 2.6
   - **Effectiveness:** Medium - Requires out-of-band verification
4. **Transport Security** - TLS/QUIC with certificate pinning
   - **Requirement:** 2.8
   - **Effectiveness:** Medium - Defense in depth

**Likelihood After Mitigation:** Low
**Impact After Mitigation:** Low

### Threat 2: Malicious Directory Node

**Threat:** Provide incorrect public keys to enable MITM.

**Mitigations:**
1. **Identity Pinning** - Never trust directory keys without verification
   - **Requirement:** 2.7
   - **Effectiveness:** High - Keys pinned after first successful pairing
2. **SAS Verification** - Verify identity on first contact
   - **Requirement:** 2.3, 2.6
   - **Effectiveness:** Medium - Requires user verification
3. **Key Rotation** - Support key rotation with re-pairing
   - **Requirement:** 5.1, 5.2
   - **Effectiveness:** Medium - Allows recovery from compromise

**Likelihood After Mitigation:** Low
**Impact After Mitigation:** Low

### Threat 3: Malicious Rendezvous Server

**Threat:** Modify signaling messages or perform key substitution.

**Mitigations:**
1. **Identity Pinning** - Verify peer identity on every connection
   - **Requirement:** 2.1, 2.2
   - **Effectiveness:** High - Detects key substitution
2. **Signed Handshakes** - Sign handshake messages with identity keys
   - **Requirement:** 4.4, 4.5
   - **Effectiveness:** High - Prevents handshake tampering
3. **Replay Protection** - Prevent reuse of handshake messages
   - **Requirement:** 3.1, 3.4, 3.5
   - **Effectiveness:** High - Prevents replay attacks

**Likelihood After Mitigation:** Low
**Impact After Mitigation:** Low

### Threat 4: Malicious Relay Server

**Threat:** Observe traffic patterns and metadata.

**Mitigations:**
1. **E2EE** - All data encrypted before relay
   - **Requirement:** 7.1
   - **Effectiveness:** High - Relay cannot decrypt
2. **Metadata Minimization** - Minimize metadata exposure
   - **Requirement:** 12.1
   - **Effectiveness:** Medium - Reduces information leakage

**Likelihood After Mitigation:** Medium (cannot prevent observation)
**Impact After Mitigation:** Low (cannot decrypt content)

### Threat 5: Compromised Endpoint

**Threat:** Attacker has compromised device and access to keys.

**Mitigations:**
1. **Secure Key Storage** - Use OS keystore with access controls
   - **Requirement:** 6.1, 6.4
   - **Effectiveness:** Medium - Reduces key exposure risk
2. **Key Rotation** - Support emergency key rotation
   - **Requirement:** 5.1, 5.5
   - **Effectiveness:** Medium - Allows recovery
3. **Session Timeouts** - Limit session lifetime
   - **Requirement:** 7.4
   - **Effectiveness:** Low - Limits exposure window
4. **Audit Logging** - Detect unauthorized access
   - **Requirement:** 9.1, 9.2, 9.3
   - **Effectiveness:** Low - Detection only, not prevention

**Likelihood After Mitigation:** Low (requires device compromise)
**Impact After Mitigation:** High (endpoint compromise is severe)

## Security Assumptions

### Trusted Components

1. **OS Keystore** - We assume the OS-provided keystore (DPAPI/Keychain/Secret Service) is secure and properly protected by the operating system.
2. **Local Device** - We assume the agent device itself is not compromised at the time of key generation and storage.
3. **Random Number Generation** - We assume the OS RNG (getrandom) provides cryptographically secure randomness.
4. **Cryptographic Libraries** - We assume ed25519-dalek, x25519-dalek, and chacha20poly1305 are correctly implemented.

### Untrusted Components

1. **Network Infrastructure** - All network components are untrusted.
2. **Directory/Rendezvous/Relay Servers** - These servers are untrusted and may be malicious.
3. **Remote Controller** - The controller is untrusted until identity is verified and pinned.

### Limitations

1. **Physical Access** - If an attacker has physical access to a device with unlocked keystore, keys may be accessible.
2. **Malware** - If malware is present on a device, it may access keys from memory or keystore.
3. **Side Channels** - Timing attacks and side-channel attacks are not fully mitigated (constant-time comparison helps but is not complete protection).

## Threat Rating Summary

| Threat | Likelihood | Impact | Mitigation Effectiveness | Residual Risk |
|--------|-----------|--------|-------------------------|---------------|
| Network Attacker (MITM) | High → Low | Critical → Low | High | Low |
| Malicious Directory | Medium → Low | High → Low | High | Low |
| Malicious Rendezvous | Medium → Low | High → Low | High | Low |
| Malicious Relay | Medium | Medium → Low | High | Low |
| Compromised Endpoint | Low | Critical → High | Medium | Medium |

## Review Schedule

This threat model SHALL be reviewed:
- With each major release
- When new attack vectors are discovered
- When new features are added that change trust boundaries
- Annually as part of security audit

**Last Updated:** 2024-01-XX
**Next Review:** 2025-01-XX
