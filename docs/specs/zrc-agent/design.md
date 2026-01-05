# zrc-agent Design

## Architecture Decision: WebRTC-first Hybrid

The agent uses a WebRTC-first hybrid architecture:
- **Control Plane (Rust)**: Identity keys, pairing, invite-only discovery, session authorization tickets, directory records, audit events
- **Media Plane (WebRTC)**: Video/audio streams via libwebrtc (C++ engine via FFI), DataChannels for control/clipboard/files
- **Fallback Relay**: coturn for TURN relay (self-hostable)

This approach leverages WebRTC's battle-tested NAT traversal (ICE), congestion control, and codec negotiation while keeping the security model in Rust.

## Processes
- Service/daemon process for always-on connectivity
- Optional UI helper for consent prompts (desktop session)

## Security: Identity-Bound DTLS (CRITICAL)

**Prevents signaling MITM attacks:**
1. Device generates DTLS cert → signs fingerprint with Ed25519 identity key
2. Operator receives fingerprint + signature in session negotiation
3. Operator verifies signature against pinned identity from PairReceipt
4. If fingerprint changes → alert user, require explicit re-approval
5. Directory/rendezvous CANNOT substitute a different cert (MITM blocked)

## Replay Protection

- Deterministic nonces: `nonce = stream_id (32-bit) || counter (64-bit)`
- Per-stream counters in AEAD AAD
- Sliding window replay filter per stream
- Duplicate packets reliably rejected

## Networking adapters
- Mesh adapter (preferred): mailbox style receive/send
- Rendezvous adapter: HTTP mailbox for signaling (SDP exchange)
- WebRTC P2P: ICE for NAT traversal
- TURN relay: coturn for fallback when P2P fails

## Data plane
- WebRTC PeerConnection for media streams
- DataChannels for: control, clipboard, files
- Backpressure: drop frames if send buffer congested

## Capture pipeline
- Phase 0: GDI (Windows) raw BGRA
- Phase 1: DXGI/WGC on Windows; PipeWire on Linux; ScreenCaptureKit macOS
- Phase 2: encoder via platform APIs (WebRTC handles codec negotiation)
  - Windows: Media Foundation / D3D11 + H.264
  - macOS: VideoToolbox
  - Linux: VAAPI/NVENC + PipeWire

## Input pipeline
- Windows SendInput (MVP)
- Linux: X11 + XTest (works), Wayland requires portal or compositor-specific support
- macOS CGEventPost + Accessibility permissions

## Transport Preference Order
mesh → WebRTC P2P → TURN relay → rendezvous (control-only)
