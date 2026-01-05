# zrc-agent Requirements (Host daemon/service)

## Purpose
Run on host machine to:
- accept pairing/session requests
- capture screen/audio
- inject input
- manage permissions and consent UI
- provide local and remote connectivity (WebRTC P2P + TURN fallback)

## Architecture: WebRTC-first Hybrid
- **Control Plane (Rust)**: Identity, pairing, tickets, policy
- **Media Plane (WebRTC)**: Video/audio via libwebrtc, DataChannels for control/clipboard/files
- **Relay**: coturn for TURN (self-hostable)

## Must-have features (MVP -> full)

### MVP (Phase 0/1) - BLOCKING SECURITY ITEMS FIRST
- **Identity-bound DTLS cert** (prevents signaling MITM)
  - Sign DTLS fingerprint with Ed25519 identity key
  - Verify against pinned identity from PairReceipt
  - Alert on cert change for same device
- **Replay protection**
  - Deterministic nonces: stream_id || counter
  - Sliding window replay filter
- Pairing and session init
- WebRTC P2P with ICE + TURN fallback
- Capture (Windows primary display) -> stream frames
- Input inject (mouse move/click, key events)
- Consent gating (always require until unattended enabled)

### Phase 2+
- Multi-monitor capture and selection
- Clipboard sync (via DataChannel)
- File transfer (via DataChannel)
- UAC/elevation flow (Windows)
- Session recording (optional)
- Remote reboot/reconnect (optional)
- Platform encoders (Media Foundation, VideoToolbox, VAAPI)

## Security
- Store keys in OS keystore where possible (DPAPI/Keychain/Secret Service)
- Never accept control without verified ticket and/or consent
- Rate limit pairing attempts and session attempts
- Audit log of pairings and sessions (signed with device key)
- Identity-bound DTLS prevents signaling MITM
- Replay protection prevents packet replay attacks
- Transport downgrade prevention

## Safe Defaults
- No unattended without explicit enable
- Consent required by default
- LAN listening off by default
- Alert on cert change for same device identity
