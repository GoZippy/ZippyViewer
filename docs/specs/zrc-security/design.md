# zrc-security Design

## Security posture
- Assume directory/rendezvous/relay are hostile.
- Confidentiality and integrity rely on:
  - pinned device/operator keys
  - envelope signatures
  - session AEAD

## Recommended defaults
- Mesh handshake as preferred:
  - reduces reliance on central rendezvous
  - can reduce metadata leakage if mesh supports it
- Discoverable mode:
  - time-bounded token only
  - requires SAS verification on both ends
  - aggressive rate limiting

## Replay protections (phase plan)
- V1 MVP:
  - tickets expire in minutes
  - session_binding includes nonce
- Next:
  - per-channel counters in AAD
  - store last-seen counters per session

## Key management plan
- Use OS keystore where possible:
  - Windows DPAPI / CNG
  - macOS Keychain
  - Linux Secret Service/libsecret
  - Android Keystore
  - iOS Keychain

