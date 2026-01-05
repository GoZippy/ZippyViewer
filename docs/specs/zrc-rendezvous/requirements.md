# zrc-rendezvous Requirements

## Purpose
Self-hosted mailbox server for untrusted byte-forwarding:
- POST envelope bytes to recipient queue
- GET long-poll to retrieve next envelope

## Must-have
- Simple REST API:
  - POST /v1/mailbox/{rid_hex}
  - GET  /v1/mailbox/{rid_hex}?wait_ms=...
- Data retention limits:
  - queue length cap per recipient
  - message size cap
  - TTL eviction
- Abuse controls:
  - optional auth token
  - optional IP rate limits
- No access to plaintext (E2EE).

## Nonfunctional
- Single binary deployment.
- Minimal config (env vars).
