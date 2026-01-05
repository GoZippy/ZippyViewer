Project goals and extracted artifacts from ChatGPT conversation
=============================================================

Summary
-------
- Build an open, self-hostable remote desktop platform (TeamViewer-class) with privacy-first defaults using a ZippyCoin mesh for identity and session initiation.
- Prioritize an MVP: Host agent, Controller app, Rendezvous/Signaling server, Relay/TURN, with WebRTC/QUIC transport and an E2EE envelope.

Key design decisions (from chat)
--------------------------------
- Rust for core (cross-platform, security-sensitive code).
- Use protobuf (prost) for wire formats.
- Mesh-first session initiation (Zippy mesh preferred); server/direct IP as fallback.
- Invite-only pairing by default, temporary discoverability optional.
- Capability tokens (SessionTicket) for unattended access; SAS/PAKE for MITM protection.

MVP components
--------------
- `zrc-core` (identity, crypto, session state machine)
- `agent-windows`, `agent-mac`, `agent-linux` (platform shims)
- `controller-desktop` (operator UI)
- `zrc-proto` (protobuf definitions)
- `zrc-transport` (transport traits: control plane, discovery, media)
- `zrc-dirnode`, `zrc-rendezvous`, `zrc-relay` (home-node / server components)

Files extracted from the ChatGPT transcript and added/verified in the repo
-----------------------------------------------------------------------
- `zippy-remote/crates/zrc-proto/proto/zrc_v1.proto` — protobuf schema (pairing, session tickets, discoverability, negotiation, revocation, audit).
- `zippy-remote/crates/zrc-proto/proto/build.rs` — prost build script (already present/verified).
- `zippy-remote/crates/zrc-transport/src/lib.rs` — transport traits (ControlPlaneTransport, DiscoveryTransport, MediaTransport) (already present/verified).

Next steps
----------
1. Implement `zrc-core` crate skeleton and wire protobuf codegen integration (prost). 
2. Add unit tests for the pairing handshake and SAS computation (proto-based fixtures).
3. Start a minimal reference implementation for Host agent (Windows) and controller using `transport_quic` feature.

If you'd like, I'll proceed to: generate prost types from `zrc_v1.proto`, add a `zrc-core` crate skeleton, and wire up a small test harness for the PairRequest/PairReceipt flows.
