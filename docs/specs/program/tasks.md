# ZRC Program Tasks (Master)

## Milestone 0 — Security Foundation (BLOCKING)
> These items MUST be completed before Phase 1. They fix critical security gaps.

- [ ] **Identity-bound QUIC/DTLS cert** (prevents signaling MITM)
  - [ ] Device signs DTLS cert fingerprint with Ed25519 identity key
  - [ ] Operator verifies fingerprint against pinned identity from PairReceipt
  - [ ] Alert + require re-approval if cert changes for same device
  - [ ] Update zrc-crypto with `sign_cert_fingerprint()` and `verify_cert_binding()`
  - [ ] Update zrc-proto with `CertBindingV1` message type

- [ ] **Replay protection + deterministic nonces**
  - [ ] Replace random nonces with `nonce = stream_id (32-bit) || counter (64-bit)`
  - [ ] Add per-stream counters to AEAD AAD
  - [ ] Implement sliding window replay filter (or strict monotonic per stream)
  - [ ] Add replay attack test: duplicate packet MUST fail
  - [ ] Update SessionCryptoV1 in zrc-crypto

- [ ] **Transport downgrade prevention**
  - [ ] Reject weaker transport if stronger available (unless user opts in)
  - [ ] Log transport selection decisions for audit

## Milestone A — Windows MVP (WebRTC-first)
- [ ] End-to-end: invite -> pair -> session -> WebRTC P2P -> frames -> input
- [ ] Identity-pinned DTLS (fingerprint signed by device identity key)
- [ ] Windows capture GDI + SendInput
- [ ] Desktop viewer with WebRTC decode
- [ ] Self-host rendezvous server
- [ ] coturn TURN relay for NAT fallback

## Milestone B — Decentralized directory
- [ ] Dirnode signed records
- [ ] Discovery tokens (time-bounded searchable mode)
- [ ] Mesh preferred transport integration
- [ ] SAS-required in discovery mode

## Milestone C — Usability & performance
- [ ] Video encoding via platform encoders (WebRTC handles codec negotiation)
  - [ ] Windows: Media Foundation / D3D11 + H.264
  - [ ] macOS: VideoToolbox
  - [ ] Linux: VAAPI/NVENC + PipeWire
- [ ] Adaptive bitrate + FPS (WebRTC congestion control)
- [ ] Multi-monitor
- [ ] Clipboard + file transfer (DataChannel initially)

## Milestone D — Cross-platform
- [ ] macOS capture (ScreenCaptureKit) + input (CGEvent + Accessibility)
- [ ] Linux capture (PipeWire portal) + input strategy
  - [ ] X11 + XTest (works)
  - [ ] Wayland: portal or compositor-specific helper (document limitations)
- [ ] Android/iOS controller apps (MediaCodec decode)

## Milestone E — Hardening
- [ ] Updater + signing
- [ ] Key storage adapters (OS keystore + encrypted file fallback)
- [ ] Audit logs + admin console
- [ ] Security review + fuzzing
- [ ] Threat model document
- [ ] Incident response + key compromise playbook

## Milestone F — Optional QUIC Transport (post-MVP)
> Only after WebRTC-first is stable and metrics collected
- [ ] QUIC as alternate transport for special cases
- [ ] Custom relay (DERP-like) for QUIC path
- [ ] Performance comparison: WebRTC vs QUIC

