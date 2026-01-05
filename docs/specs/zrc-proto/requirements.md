# zrc-proto Requirements

## Purpose
Define stable wire formats (protobuf v1) for:
- identity/pairing/session
- envelope (signed + sealed payload)
- control plane (input, clipboard, files)
- transport negotiation (QUIC/WebRTC/direct/relay)
- directory records (signed, time-bounded)

## Must-have
- Backwards compatible changes: additive fields only, never reuse tags.
- Deterministic security-critical hashing inputs MUST NOT depend on protobuf encoding quirks.
- Provide explicit versioning (V1 suffix) in message names.
- Enumerations for:
  - transports, cipher suites, permissions, error codes, roles.

## Security requirements
- No secret material in directory or invites beyond time-bounded invite secret.
- Tickets bound to session tuple (session_id, operator_id, device_id, nonce).
- Support signature type extensibility (Ed25519 now, PQC later).

## Non-goals
- Storing large media streams in protobuf messages (frames are binary blobs over QUIC streams).
