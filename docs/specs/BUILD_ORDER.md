# ZRC Project Build Order

## Overview

This document defines the exact order in which components should be built, their dependencies, and milestone checkpoints. Follow this order strictly to avoid blocked dependencies.

## Build Phases

### Phase 0: Foundation (Security Blockers + Core Libraries)
> **Goal:** Establish cryptographic foundation and wire formats
> **Milestone Gate:** All property tests pass, golden vectors verified

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 1 | zrc-proto | None | Wire formats, CertBindingV1, SignalingMessage | ✅ COMPLETE |
| 2 | zrc-crypto | zrc-proto | Identity-bound DTLS, replay protection, envelope | ✅ COMPLETE |
| 3 | zrc-core | zrc-proto, zrc-crypto | Pairing/session state machines, policy hooks | ✅ COMPLETE |

**Milestone 0 Checkpoint:**
- [x] zrc-proto: Golden vector roundtrip tests pass (17 tests)
- [x] zrc-crypto: Transcript hash vectors match across platforms (61 tests)
- [x] zrc-crypto: Replay attack test (duplicate packet fails)
- [x] zrc-core: Pairing flow rejects invalid invite proof (130 tests)
- [x] All unit tests pass
- [x] `cargo clippy` clean

---

### Phase 0.5: Windows MVP Infrastructure
> **Goal:** Platform capture/input + signaling server
> **Milestone Gate:** Can capture frames and inject input on Windows

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 4 | zrc-platform-win | zrc-proto | GDI/WGC capture, SendInput injection | ✅ COMPLETE |
| 5 | zrc-rendezvous | zrc-proto, zrc-crypto | HTTP mailbox for signaling | ✅ COMPLETE |

**Milestone 0.5 Checkpoint:**
- [x] zrc-platform-win: Capture single monitor works (34 tests)
- [x] zrc-platform-win: Mouse/keyboard injection works
- [x] zrc-rendezvous: Message send/receive works (5 tests)
- [x] zrc-rendezvous: Rate limiting enforced
- [x] All unit tests pass

---

### Phase 1: End-to-End MVP
> **Goal:** Working remote control Windows → Windows
> **Milestone Gate:** Can pair, connect, view screen, control input

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 6 | zrc-agent | zrc-core, zrc-platform-win, zrc-rendezvous | Host daemon with WebRTC | ✅ COMPILES (0 tests) |
| 7 | zrc-controller | zrc-core | CLI controller | ✅ COMPLETE (144 tests) |
| 8 | zrc-desktop | zrc-controller | GUI viewer | 🔄 IN_PROGRESS (Task 1 done, 9 tests) |

**Milestone 1 Checkpoint:**
- [ ] End-to-end: invite → pair → session → frames → input
- [ ] Identity-bound DTLS verified (cert change triggers alert)
- [ ] Consent prompt displays and blocks until approved
- [ ] Session terminates cleanly (no stuck keys)
- [ ] All property tests pass (100+ iterations)
- [ ] Manual QA: 10-minute remote session stable

---

### Phase 1.5: NAT Traversal
> **Goal:** Work across NAT/firewalls
> **Milestone Gate:** Connection works from different networks

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 9 | coturn-setup | None (external) | TURN relay deployment | ⏳ PENDING |
| 10 | zrc-relay | zrc-proto, zrc-crypto | Custom relay (optional) | ⏳ PENDING |

**Milestone 1.5 Checkpoint:**
- [ ] WebRTC P2P works on same LAN
- [ ] TURN fallback works across NAT
- [ ] Connection ladder: P2P → TURN verified
- [ ] Relay never sees plaintext (E2EE verified)

---

### Phase 2: Directory + Discovery
> **Goal:** Device discovery without manual IP exchange
> **Milestone Gate:** Can find devices by discovery token

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 11 | zrc-dirnode | zrc-proto, zrc-crypto | Home directory server | ⏳ PENDING |

**Milestone 2 Checkpoint:**
- [ ] Signed DirRecord verification works
- [ ] Discovery tokens are time-bounded
- [ ] Anti-enumeration protections work
- [ ] Token rotation works

---

### Phase 3: Cross-Platform Agents
> **Goal:** Support macOS and Linux hosts
> **Milestone Gate:** Remote control works on all desktop platforms

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 12 | zrc-platform-mac | zrc-proto | ScreenCaptureKit, CGEvent | ⏳ PENDING |
| 13 | zrc-platform-linux | zrc-proto | PipeWire, X11/Wayland input | ⏳ PENDING |

**Milestone 3 Checkpoint:**
- [ ] macOS capture + input works
- [ ] Linux X11 capture + input works
- [ ] Linux Wayland limitations documented and handled
- [ ] Cross-platform sessions work (Win→Mac, Mac→Linux, etc.)

---

### Phase 4: Mobile + Polish
> **Goal:** Mobile controllers, auto-update, admin UI
> **Milestone Gate:** Production-ready release

| Order | Component | Dependencies | Deliverables | Status |
|-------|-----------|--------------|--------------|--------|
| 14 | zrc-platform-android | zrc-proto, zrc-core | Android controller app | ⏳ PENDING |
| 15 | zrc-platform-ios | zrc-proto, zrc-core | iOS controller app | ⏳ PENDING |
| 16 | zrc-updater | zrc-crypto | Secure auto-update | ⏳ PENDING |
| 17 | zrc-admin-console | zrc-core | Web admin UI | ⏳ PENDING |
| 18 | zrc-ci | None | Build/release automation | ⏳ PENDING |
| 19 | zrc-security | All | Threat model, audit prep | ⏳ PENDING |

**Milestone 4 Checkpoint:**
- [ ] Android controller works
- [ ] iOS controller works
- [ ] Auto-update with signed manifests works
- [ ] Admin console manages devices/users
- [ ] CI builds all platforms
- [ ] Security audit checklist complete

---

## Dependency Graph

```
Phase 0 (Foundation)
    zrc-proto ─────────────────────────────────────────┐
        │                                               │
    zrc-crypto ────────────────────────────────────┐   │
        │                                           │   │
    zrc-core ──────────────────────────────────┐   │   │
        │                                       │   │   │
Phase 0.5 (Windows MVP)                         │   │   │
    zrc-platform-win ◄─────────────────────────┼───┼───┤
        │                                       │   │   │
    zrc-rendezvous ◄───────────────────────────┼───┘   │
        │                                       │       │
Phase 1 (E2E MVP)                               │       │
    zrc-agent ◄────────────────────────────────┤       │
        │                                       │       │
    zrc-controller ◄───────────────────────────┤       │
        │                                       │       │
    zrc-desktop ◄──────────────────────────────┘       │
        │                                               │
Phase 1.5 (NAT)                                         │
    coturn-setup (external)                             │
    zrc-relay ◄────────────────────────────────────────┤
        │                                               │
Phase 2 (Discovery)                                     │
    zrc-dirnode ◄──────────────────────────────────────┤
        │                                               │
Phase 3 (Cross-Platform)                                │
    zrc-platform-mac ◄─────────────────────────────────┤
    zrc-platform-linux ◄───────────────────────────────┤
        │                                               │
Phase 4 (Mobile + Polish)                               │
    zrc-platform-android ◄─────────────────────────────┤
    zrc-platform-ios ◄─────────────────────────────────┤
    zrc-updater ◄──────────────────────────────────────┘
    zrc-admin-console
    zrc-ci
    zrc-security
```

## Execution Rules

1. **Never skip a phase** - Dependencies must be complete before starting dependent components
2. **Milestone gates are blocking** - All checkpoints must pass before proceeding
3. **Test continuously** - Run `cargo test` after every task completion
4. **Fix bugs immediately** - Don't accumulate technical debt
5. **Property tests are mandatory** - 100+ iterations minimum
6. **Document as you go** - Update specs if implementation differs

## Time Estimates (Rough)

| Phase | Components | Estimated Duration |
|-------|------------|-------------------|
| Phase 0 | 3 | 2-3 weeks |
| Phase 0.5 | 2 | 1-2 weeks |
| Phase 1 | 3 | 3-4 weeks |
| Phase 1.5 | 2 | 1 week |
| Phase 2 | 1 | 1 week |
| Phase 3 | 2 | 2-3 weeks |
| Phase 4 | 6 | 4-6 weeks |
| **Total** | **19** | **14-20 weeks** |
