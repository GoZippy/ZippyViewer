# zrc-crypto Requirements

## Purpose
Provide audited, pure-Rust cryptographic building blocks used by all components.

## Must-have capabilities
- Canonical Transcript hashing (tag-length-value).
- Pairing proof:
  - HMAC(invite_secret, canonical_pair_request_fields)
- SAS:
  - 6-digit derived from transcript hash (stable across platforms)
- Envelope:
  - Signed header + sealed payload using X25519 + HKDF + ChaCha20Poly1305
  - Signature covers header + kex + aad + ciphertext
- Ticket:
  - Device-signed SessionTicketV1
  - Verify binding, expiry, signature
- Session Crypto:
  - Derive per-session AEAD keys from session_binding + ticket_id
  - Encrypt control and frame payloads above transport (E2EE)

## Security requirements
- No OpenSSL dependency.
- Zeroize secret buffers where practical.
- Reject downgrade: pinned identity keys required after pairing.
- Verify before decrypt where possible (signature check first).

## Non-goals
- NAT traversal logic (belongs in networking layer).
- GUI / capture logic.
