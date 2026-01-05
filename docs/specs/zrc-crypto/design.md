# zrc-crypto Design

## Algorithms (v1)
- Identity signatures: Ed25519
- Key exchange: X25519
- KDF: HKDF-SHA256
- AEAD: ChaCha20Poly1305
- Hash: SHA-256
- Proof: HMAC-SHA256

## Canonical Transcript
- Append(tag:u32, len:u32, bytes)
- Used for: SAS, pair_proof_input, binding, directory signatures

## Layering
- QUIC/TLS pinning protects bootstrap ticket exchange
- Session AEAD protects all subsequent data (control+frames) end-to-end

## Failure handling
- Hard fail on signature mismatch or id mismatch.
- Return explicit error types for logging and telemetry.

## Future
- PQC signatures/KEM as optional key types (Dilithium/Kyber)
- Hybrid mode with classical + PQC.
