---
inclusion: always
---

# ZRC Build Order Steering

## Component Build Order

When working on ZRC, always follow this strict build order. Never start a component until its dependencies are complete.

### Phase 0: Foundation (MUST complete first)
1. **zrc-proto** - Wire formats (no dependencies)
2. **zrc-crypto** - Cryptographic primitives (depends on zrc-proto)
3. **zrc-core** - Business logic (depends on zrc-proto, zrc-crypto)

### Phase 0.5: Windows MVP Infrastructure
4. **zrc-platform-win** - Windows capture/input (depends on zrc-proto)
5. **zrc-rendezvous** - Signaling server (depends on zrc-proto, zrc-crypto)

### Phase 1: End-to-End MVP
6. **zrc-agent** - Host daemon (depends on zrc-core, zrc-platform-win, zrc-rendezvous)
7. **zrc-controller** - CLI controller (depends on zrc-core)
8. **zrc-desktop** - GUI viewer (depends on zrc-controller)

### Later Phases
9-19. See `docs/specs/BUILD_ORDER.md` for complete list

## Execution Rules

1. **Check dependencies first** - Before starting any component, verify all dependencies are complete
2. **Run tests after each task** - Execute `cargo test -p <crate>` after completing each task
3. **Fix bugs immediately** - Don't proceed with broken tests
4. **Update queue status** - Mark components in `docs/specs/EXECUTION_QUEUE.md`
5. **Property tests are mandatory** - Run with 100+ iterations

## Architecture: WebRTC-first Hybrid

- **Control Plane (Rust)**: Identity, pairing, tickets, policy
- **Media Plane (WebRTC)**: Video/audio via libwebrtc, DataChannels
- **Relay**: coturn for TURN fallback

## Security Blockers (Phase 0)

These MUST be implemented before Phase 1:
- Identity-bound DTLS cert (prevents signaling MITM)
- Replay protection with deterministic nonces
- Transport downgrade prevention

## Reference Documents

- Build order: `docs/specs/BUILD_ORDER.md`
- Execution queue: `docs/specs/EXECUTION_QUEUE.md`
- Compatibility matrices: `docs/specs/COMPATIBILITY_MATRICES.md`
- Master tasks: `docs/specs/program/tasks.md`
