# zrc-core Requirements

## Purpose
Shared core logic used by agent/controller:
- pairing/session state machines
- policy hooks (consent, permissions)
- message dispatch by msg_type
- transport negotiation decisions

## Must-have
- PairingHost/PairingController flows:
  - verify invite secret proof
  - store pinned device keys
  - generate PairReceipt with device signature
- SessionHost/SessionController:
  - require consent based on pairing policy
  - issue and verify tickets for unattended
  - transport negotiation response (QUIC params)
- Dispatch:
  - decode EnvelopeV1 -> open -> route to handler
- Storage abstraction:
  - memory store (MVP)
  - trait interface for sqlite/sled/rocks (later)

## Nonfunctional
- Deterministic behavior across OS.
- Minimal dependencies; portable to mobile.

## Non-goals
- Actual networking sockets implementation (transport modules or executables).
