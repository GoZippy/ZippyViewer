# ZRC Compatibility Matrices

## A) Platform Feature Matrix

Legend: ✅ supported | 🟡 partial/limited | ❌ not planned yet

| Feature | Win Agent | mac Agent | Linux Agent | Android Controller | iOS Controller | Desktop Controller |
|---------|-----------|-----------|-------------|-------------------|----------------|-------------------|
| Pairing + key pinning | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Session tickets (unattended) | ✅ | ✅ | ✅ | 🟡 (cached) | 🟡 (cached) | ✅ |
| Capture single monitor | ✅ (GDI/WGC) | 🟡 (SCK) | 🟡 (PipeWire/X11) | ❌ | ❌ | N/A |
| Multi-monitor | 🟡 | 🟡 | 🟡 | N/A | N/A | ✅ (viewer) |
| Input injection | ✅ | 🟡 (perm-heavy) | 🟡 (Wayland limits) | N/A | N/A | ✅ (send events) |
| Clipboard | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 |
| File transfer | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 |
| Audio | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 | 🟡 |
| Encoding (VP8/H264) | 🟡 | 🟡 | 🟡 | ✅ decode | ✅ decode | ✅ decode |
| Install as service/daemon | ✅ | 🟡 (launchd) | ✅ (systemd) | ❌ | ❌ | N/A |

### Platform-Specific Notes

**Windows Agent:**
- Capture: GDI fallback + WGC/DXGI preferred path
- Input: SendInput for mouse + key + unicode
- UAC/elevation documented and tested

**macOS Agent:**
- ScreenCaptureKit capture + permission onboarding
- CGEvent input injection + Accessibility permission flow

**Linux Agent:**
- PipeWire portal capture works on GNOME/KDE
- X11 fallback works
- Wayland input limitations clearly handled (requires portal or compositor-specific support)

**Android/iOS Controller:**
- Frame rendering efficient (battery)
- Touch→mouse mapping sane
- Background behavior correct

## B) Network Connectivity Matrix

| Network Environment | Direct QUIC | WebRTC P2P | TURN Relay | Rendezvous Mailbox | Mesh Mailbox |
|--------------------|-------------|------------|------------|-------------------|--------------|
| Same LAN | ✅ best | ✅ best | ❌ | ✅ | ✅ |
| NAT, port-forward unavailable | 🟡 (sometimes) | ✅ (ICE) | ✅ | ✅ (control only) | ✅ |
| Symmetric NAT / CGNAT | ❌ typical | 🟡 (TURN needed) | ✅ required | ✅ (control only) | ✅ |
| Corporate proxy restrictive | 🟡 | 🟡 | ✅ (443) | ✅ (HTTPS) | 🟡 (depends) |
| Offline / local-only | ✅ | ✅ | ❌ | ❌ | 🟡 (local mesh) |

**Critical note:** Rendezvous mailbox alone can't carry high-rate media. It's control-plane + negotiation only.

## C) Protocol/Version Compatibility Matrix

| Component | Versioning Rule | Must Support | Deprecation Policy |
|-----------|-----------------|--------------|-------------------|
| zrc-proto | V1/V2 side-by-side | V1 for 2 years | disable old only after dual-stack |
| zrc-crypto | feature-gated algorithms | Ed25519/X25519/ChaCha20Poly1305 | PQC optional until stable |
| zrc-agent/controller | rolling upgrades | N-1 compatibility | hard block if crypto policy changes |
| dirnode/relay | stateless protocol | N/N-1 | deprecate tokens gradually |

## D) Data Retention & Privacy Matrix

**Key Principle:** Assume anything not E2EE can leak. Reduce metadata, make retention short, keep logs opt-in.

| Component | Sees in Transit | Stores by Default | Retention Default | Operator Controls | Privacy Risks | Required Mitigations |
|-----------|-----------------|-------------------|-------------------|-------------------|---------------|---------------------|
| zrc-agent | plaintext local only; remote data encrypted | pairing keys, policy, minimal logs | indefinite for pairings; logs 7 days | revoke pairings; disable unattended; log levels | local compromise | OS keystore; encryption; least-privilege |
| zrc-controller/desktop | decrypted frames/input locally | pairing cache, tickets (short) | tickets minutes-hours; logs off by default | clear cache; "forget device" | device compromise | encrypted local store; no sensitive logs |
| zrc-rendezvous | envelope bytes, IPs, timing, sizes | message queue in memory | seconds–minutes | queue caps; auth; disable logs | traffic analysis | cap/TTL; minimize logs; optional auth |
| zrc-dirnode | signed records + queries (token use), IPs | records + tokens | tokens minutes; records TTL | disable discoverability; rotate tokens | relationship mapping | anti-enum, rate limit, minimal responses |
| zrc-relay | IPs, timing, bandwidth | allocation table | minutes-hours | caps + logs off | traffic analysis | minimize metadata; quotas; rotate allocation IDs |
| Mesh layer | depends on mesh | depends | depends | user-managed | metadata leakage | onion-ish routing optional; minimize IDs |
| Managed cloud (paid) | IPs, auth events, usage stats | telemetry + logs (opt-in) | short by default | clear/export | trust concerns | transparent docs; privacy mode; local-only option |

### Paranoid Mode (Self-Hosters)

Release-blocking privacy requirement: provide a "paranoid mode" for self-hosters:
- No persistent session logs
- No external calls
- Minimal metadata in dirnode/relay
- Strict key pinning and alerts

## E) Acceptance Test Checklists (Release-Blocking Gates)

### 1) zrc-proto
- [ ] Backward compatibility: additive changes only; tag audit enforced in CI
- [ ] Golden vector decode/encode roundtrip tests in Rust
- [ ] Message size limits defined for each major message type
- [ ] Protocol version negotiation behavior specified for mismatch
- [ ] Fuzz: protobuf decoding doesn't panic or hang

### 2) zrc-crypto
- [ ] Transcript hash vectors: known input → known output across platforms
- [ ] Envelope: verify signature before decrypt; tamper in any header field fails
- [ ] Sender ID must equal sha256(sender_sign_pub); mismatch fails
- [ ] Ticket verification: expiry, binding, signature all required
- [ ] Session AEAD: replay test (duplicate packet fails)
- [ ] Session AEAD: counter out-of-order test behavior defined (drop or buffer)
- [ ] Session AEAD: nonce reuse impossible by construction
- [ ] Fuzz targets: envelope_open, ticket_verify, frame decode

### 3) zrc-core
- [ ] Pairing flow: invalid invite secret proof rejected
- [ ] Pairing flow: SAS displayed/verified in discoverable mode
- [ ] Pairing flow: PairReceipt signature verifies and pins keys
- [ ] Session init: unattended requires valid ticket OR explicit consent
- [ ] Session init: transport downgrade prevention
- [ ] Store: persistence path (sqlite) passes crash/restart tests
- [ ] Rate limiting hooks exist and are enforced in agent

### 4) zrc-rendezvous (mailbox)
- [ ] No plaintext ever handled (only opaque bytes)
- [ ] Message size cap enforced
- [ ] Queue cap per recipient enforced
- [ ] TTL eviction works
- [ ] Rate limiting works (per IP and per recipient)
- [ ] Optional auth token works
- [ ] Abuse tests: mailbox enumeration mitigations (constant-time-ish responses, no leakage)

### 5) zrc-dirnode (home directory + discoverability)
- [ ] Signed DirRecord verification enforced client-side
- [ ] Device identity key binds endpoints + cert fingerprints (no substitution)
- [ ] Discovery token: time-bounded, non-enumerable
- [ ] Discovery token: cannot be minted by dirnode unless explicitly intended
- [ ] Search mode: cannot be indexed broadly (no "list all devices")
- [ ] Search mode: rate limiting and anti-bruteforce
- [ ] Privacy: minimal record returned in discovery mode
- [ ] Rotation: tokens revoke cleanly and immediately

### 6) zrc-relay
- [ ] Relay never sees plaintext (session AEAD verified)
- [ ] Allocation tokens expire and are bound to device identity
- [ ] Bandwidth caps enforced
- [ ] DoS resilience: per-allocation and per-IP throttles
- [ ] Logs default minimal; PII optional and off by default
- [ ] Multi-instance scaling test (at least two relays)

### 7) zrc-agent (host daemon)
- [ ] Safe defaults: no unattended without explicit enable
- [ ] Safe defaults: consent required by default
- [ ] Safe defaults: LAN listening off by default
- [ ] Pairing prompts show SAS in discovery mode
- [ ] Input safety: stuck keys recovery on disconnect
- [ ] Input safety: "panic" stop hotkey local
- [ ] Input safety: permission-limited control modes
- [ ] Capture: multi-monitor metadata correct
- [ ] Capture: frame pacing doesn't spike CPU
- [ ] Service behavior: starts on boot (where supported)
- [ ] Service behavior: logs rotate; no secret material in logs
- [ ] Security: key storage in OS keystore or encrypted file fallback
- [ ] Security: device cert binding to identity key implemented
- [ ] Regression: reconnect loops, session drops, and partial network failures handled

### 8) zrc-controller (CLI)
- [ ] Pair/import invite works (QR/base64)
- [ ] Prints SAS when required and blocks until confirmed
- [ ] Session init produces ticket and caches it securely
- [ ] Connect ladder works (mesh → WebRTC P2P → TURN → rendezvous)
- [ ] Debug flags show transport decisions without leaking secrets

### 9) zrc-desktop (GUI)
- [ ] Frame rendering stable; no tearing; handles resize
- [ ] Input mapping correct (scale and multi-monitor)
- [ ] Disconnect/reconnect UI correct
- [ ] Clear security indicators: paired device identity displayed
- [ ] Clear security indicators: new identity/cert triggers warning
- [ ] Accessibility + keyboard support basics

### 10) Updater
- [ ] Update manifests signed and verified
- [ ] Rollback safe
- [ ] Key rotation procedure tested
- [ ] Offline update supported

### 11) Security Program
- [ ] Threat model document exists and is current
- [ ] External audit checklist prepared
- [ ] Fuzzing runs in CI
- [ ] Incident response + key compromise playbook exists

## F) Quality Gates (Non-Negotiable)

**CI must run:**
- Formatting
- Clippy
- Unit tests
- Integration e2e harness
- Fuzz smoke

**Security regression suite:**
- MITM attempts
- Replay attacks
- Downgrade attacks
- Stale ticket usage
- Swapped IDs

**Performance budgets:**
- CPU cap targets (TBD per platform)
- Memory cap targets (TBD per platform)
- Latency targets (TBD, aim for <100ms input-to-display)
