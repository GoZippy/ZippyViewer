# Implementation Plan: zrc-crypto

## Overview

Implementation tasks for the cryptographic primitives crate. This crate provides all security-critical operations using pure-Rust implementations with no OpenSSL dependency.

**Build Order:** #2 (Depends on: zrc-proto)

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with crypto dependencies
    - ed25519-dalek, x25519-dalek, chacha20poly1305
    - sha2, hmac, hkdf, zeroize, rand
    - _Requirements: 2.1, 2.2, 5.3, 5.4_
  - [x] 1.2 Create module structure
    - transcript, identity, pairing, sas, envelope, ticket, session_crypto, replay, cert_binding
    - _Requirements: 1.1, 2.1_

- [x] 2. Implement Transcript module
  - [x] 2.1 Implement Transcript struct with SHA-256 hasher
    - append(tag, data) with tag(4) || len(4) || data format
    - finalize() returning 32-byte hash
    - fork() for branching derivations
    - _Requirements: 1.1, 1.2, 1.3_
  - [x] 2.2 Define standard tag constants
    - TAG_DEVICE_ID, TAG_OPERATOR_ID, TAG_SIGN_PUB, etc.
    - _Requirements: 1.1_
  - [x]* 2.3 Write property test for transcript determinism
    - **Property 1: Transcript Determinism**
    - **Validates: Requirements 1.4**

- [x] 3. Implement Identity module
  - [x] 3.1 Implement Identity struct with Ed25519 and X25519 keys
    - generate() using secure random
    - id() deriving SHA-256(sign_pub)
    - public_bundle() returning PublicKeyBundle
    - _Requirements: 2.1, 2.2, 2.3_
  - [x] 3.2 Implement signing and verification
    - sign(message) returning 64-byte signature
    - verify_signature(pub_key, message, signature)
    - _Requirements: 2.4, 2.5_
  - [x] 3.3 Implement key exchange
    - key_exchange(peer_pub) returning SharedSecret
    - _Requirements: 5.2_
  - [x] 3.4 Implement Drop with zeroization
    - Zeroize sign_key and kex_key on drop
    - _Requirements: 2.6, 11.1_
  - [x]* 3.5 Write property test for signature round-trip
    - **Property 2: Signature Round-Trip**
    - **Validates: Requirements 2.7**

- [x] 4. Implement Identity-Bound DTLS Cert Binding (SECURITY BLOCKER)
  - [x] 4.1 Implement sign_cert_fingerprint
    - Input: DTLS cert fingerprint (32 bytes SHA-256)
    - Output: Ed25519 signature of fingerprint
    - _Requirements: Security - Identity-bound DTLS_
  - [x] 4.2 Implement verify_cert_binding
    - Input: fingerprint, signature, pinned_sign_pub
    - Verify signature matches pinned identity from PairReceipt
    - _Requirements: Security - Prevent signaling MITM_
  - [x] 4.3 Implement CertBinding struct
    - fingerprint: [u8; 32]
    - signature: [u8; 64]
    - signer_pub: [u8; 32]
    - _Requirements: Security - Identity-bound DTLS_
  - [x]* 4.4 Write property test for cert binding round-trip
    - **Property: Cert Binding Round-Trip**
    - **Validates: Security - Identity-bound DTLS**

- [x] 5. Implement Pairing module
  - [x] 5.1 Implement invite proof generation
    - generate_invite_proof(invite_secret, pair_request_transcript)
    - HMAC-SHA256 based proof
    - _Requirements: 3.1, 3.2_
  - [x] 5.2 Implement invite proof verification
    - verify_invite_proof(invite_secret, pair_request, proof)
    - Timestamp validation (5 minute window)
    - _Requirements: 3.3, 3.4, 3.5_
  - [x]* 5.3 Write property test for pairing proof round-trip
    - **Property: Pairing Proof Round-Trip**
    - **Validates: Requirements 3.6**

- [x] 6. Implement SAS module
  - [x] 6.1 Implement SAS computation
    - compute_sas(transcript) returning 6-digit string
    - Formula: (hash[0..4] as u32) % 1_000_000
    - Format with leading zeros
    - _Requirements: 4.1, 4.2, 4.5_
  - [x] 6.2 Define SasTranscript struct
    - Both public keys, session_id, nonces
    - _Requirements: 4.3_
  - [x]* 6.3 Write property test for SAS consistency
    - **Property 4: SAS Consistency**
    - **Validates: Requirements 4.4**

- [x] 7. Implement Envelope module
  - [x] 7.1 Implement seal_envelope
    - Generate ephemeral X25519 keypair
    - ECDH + HKDF key derivation
    - ChaCha20Poly1305 encryption
    - Ed25519 signature over header + ciphertext
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_
  - [x] 7.2 Implement open_envelope
    - Verify signature before decryption
    - ECDH + HKDF key derivation
    - ChaCha20Poly1305 decryption
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_
  - [x]* 7.3 Write property test for envelope round-trip
    - **Property 3: Envelope Round-Trip**
    - **Validates: Requirements 5.8, 6.7**

- [x] 8. Implement Ticket module
  - [x] 8.1 Implement sign_ticket
    - Canonical transcript of ticket fields
    - Ed25519 signature
    - _Requirements: 7.1, 7.2_
  - [x] 8.2 Implement verify_ticket
    - Signature verification
    - Expiration check
    - Session binding verification
    - _Requirements: 7.3, 7.4, 7.5_
  - [x]* 8.3 Write property test for ticket expiry enforcement
    - **Property 5: Ticket Expiry Enforcement**
    - **Validates: Requirements 7.4**

- [x] 9. Implement Session Crypto module with Replay Protection (SECURITY BLOCKER)
  - [x] 9.1 Implement derive_session_keys
    - HKDF from session_binding + ticket_id + shared_secret
    - Separate keys per direction and channel
    - _Requirements: 8.1, 8.2, 8.3_
  - [x] 9.2 Implement deterministic nonce generation
    - nonce = stream_id (32-bit) || counter (64-bit) = 12 bytes total
    - Counter MUST be monotonically increasing per stream
    - _Requirements: Security - Replay protection_
  - [x] 9.3 Implement encrypt_control and decrypt_control
    - ChaCha20Poly1305 with deterministic nonce
    - Include stream_id + counter in AAD
    - _Requirements: 8.4, 8.5, 8.6_
  - [x]* 9.4 Write property test for session crypto round-trip
    - **Property: Session Crypto Round-Trip**
    - **Validates: Requirements 8.8**
  - [x]* 9.5 Write property test for nonce uniqueness
    - **Property: Nonce Uniqueness**
    - **Validates: Security - No nonce reuse**

- [x] 10. Implement Replay Filter (SECURITY BLOCKER)
  - [x] 10.1 Implement ReplayFilter struct
    - Per-stream counter tracking
    - Sliding window bitmap (default 1024 packets)
    - _Requirements: Security - Replay protection_
  - [x] 10.2 Implement check_and_update method
    - Input: stream_id, counter
    - Reject if counter already seen
    - Reject if counter too old (outside window)
    - Update window on success
    - _Requirements: Security - Replay protection_
  - [x] 10.3 Implement generate_nonce helper
    - Combines stream_id + counter into 12-byte nonce
    - _Requirements: Security - Deterministic nonces_
  - [x]* 10.4 Write property test for replay detection
    - **Property 6: Replay Detection**
    - Duplicate packet MUST fail
    - **Validates: Security - Replay protection**

- [x] 11. Implement Directory Record signing
  - [x] 11.1 Implement sign_record and verify_record
    - Canonical transcript of record fields
    - Expiration validation (timestamp + ttl)
    - Subject ID verification
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_
  - [x]* 11.2 Write property test for directory record round-trip
    - **Property: Directory Record Round-Trip**
    - **Validates: Requirements 9.6**

- [x] 12. Implement secure memory handling
  - [x] 12.1 Add Zeroize derives to all key types
    - _Requirements: 11.1, 11.2, 11.3_
  - [x] 12.2 Implement constant-time comparison utilities
    - _Requirements: 11.4, 11.5_
  - [x]* 12.3 Write property test for key zeroization
    - **Property 7: Key Zeroization**
    - **Validates: Requirements 11.1, 11.2, 11.3**

- [x] 13. Checkpoint - Verify all tests pass
  - Run: `cargo test -p zrc-crypto`
  - Run: `cargo clippy -p zrc-crypto`
  - Ensure all property tests pass with 100+ iterations
  - **CRITICAL:** Verify replay attack test (duplicate packet fails)
  - **CRITICAL:** Verify cert binding round-trip works
  - **On completion:** Update `docs/specs/EXECUTION_QUEUE.md` status to ✅ COMPLETE
  - **Next component:** zrc-core

## Notes

- Tasks marked with `*` are optional property-based tests
- All crypto uses pure-Rust implementations (no OpenSSL)
- Zeroization is critical for security - test where possible
- Constant-time operations prevent timing attacks
- **SECURITY BLOCKERS:** Tasks 4, 9, 10 are Phase 0 security requirements
