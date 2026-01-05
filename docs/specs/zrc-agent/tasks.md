# zrc-agent Tasks

## Phase 0 - Security Foundation (BLOCKING)
- [ ] Identity-bound DTLS cert
  - [ ] Sign DTLS fingerprint with Ed25519 identity key
  - [ ] Verify peer fingerprint against pinned identity
  - [ ] Alert on cert change for same device
- [ ] Replay protection
  - [ ] Deterministic nonces: stream_id || counter
  - [ ] Per-stream counters in AEAD AAD
  - [ ] Sliding window replay filter
- [ ] Transport downgrade prevention

## Phase 0 - Windows MVP (WebRTC-first)
- [ ] Implement service skeleton with config + logging
- [ ] Wire core pairing/session handlers to signaling
- [ ] WebRTC integration (libwebrtc FFI or webrtc-rs)
- [ ] ICE configuration (STUN + coturn TURN)
- [ ] Windows capture stub + SendInput stub
- [ ] Consent policy (always require)

## Phase 1 - Reliable & Self-Hostable
- [ ] Persistent pairing store (sqlite)
- [ ] Unattended tickets + revocation
- [ ] Cross-platform capture/input scaffolding
- [ ] coturn deployment guide for self-hosters
- [ ] Local LAN discovery (optional, mDNS)

## Phase 2 - Usable on Real Internet
- [ ] Video encoding via platform encoders (WebRTC handles negotiation)
  - [ ] Windows: Media Foundation / D3D11 + H.264
  - [ ] macOS: VideoToolbox
  - [ ] Linux: VAAPI/NVENC
- [ ] Adaptive bitrate (WebRTC congestion control)
- [ ] Clipboard sync (DataChannel)
- [ ] File transfer (DataChannel)

## Phase 3 - Polish
- [ ] Updater integration + code signing checks
- [ ] Policy engine + admin controls
- [ ] Audit log export
- [ ] Multi-monitor support
- [ ] Session recording (optional)
