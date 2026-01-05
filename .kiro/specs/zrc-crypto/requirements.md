# Requirements Document: zrc-crypto

## Introduction

The zrc-crypto crate provides all cryptographic primitives for the Zippy Remote Control (ZRC) system. This crate implements identity management, key exchange, envelope encryption, ticket signing, and session cryptography using pure-Rust implementations with no OpenSSL dependency. All security-critical operations are centralized here to enable focused auditing and consistent behavior across platforms.

## Glossary

- **Ed25519**: An elliptic curve digital signature algorithm for identity signing
- **X25519**: An elliptic curve Diffie-Hellman key exchange algorithm
- **ChaCha20Poly1305**: An authenticated encryption with associated data (AEAD) cipher
- **HKDF**: HMAC-based Key Derivation Function for deriving keys from shared secrets
- **Transcript**: A canonical byte sequence for deterministic cryptographic commitments
- **SAS**: Short Authentication String, a human-verifiable code for MITM detection
- **Envelope**: A signed and sealed container for encrypted payloads
- **Session_Binding**: A cryptographic commitment linking a session to specific identities
- **Zeroize**: Securely erasing sensitive data from memory after use
- **HMAC**: Hash-based Message Authentication Code for proof generation
- **Nonce**: A number used once to ensure encryption uniqueness

## Requirements

### Requirement 1: Canonical Transcript Hashing

**User Story:** As a developer, I want deterministic transcript hashing, so that cryptographic proofs are consistent across all platforms and implementations.

#### Acceptance Criteria

1. THE Transcript_Module SHALL implement append(tag: u32, data: &[u8]) that writes tag as 4-byte big-endian, length as 4-byte big-endian, then data bytes
2. THE Transcript_Module SHALL implement finalize() that returns a SHA-256 hash of all appended data
3. THE Transcript_Module SHALL implement clone() to allow branching transcripts for different derivations
4. WHEN the same sequence of append calls is made, THE Transcript_Module SHALL produce identical hash outputs across all platforms
5. THE Transcript_Module SHALL reject append calls after finalize() has been called
6. FOR ALL valid transcript sequences, appending then finalizing SHALL produce a deterministic 32-byte hash

### Requirement 2: Identity Key Management

**User Story:** As a developer, I want identity key generation and management, so that devices and operators have cryptographically secure identities.

#### Acceptance Criteria

1. THE Identity_Module SHALL generate Ed25519 signing keypairs using a cryptographically secure random source
2. THE Identity_Module SHALL generate X25519 key exchange keypairs using a cryptographically secure random source
3. THE Identity_Module SHALL derive device_id as SHA-256(sign_pub) truncated to 32 bytes
4. THE Identity_Module SHALL implement sign(message: &[u8]) returning a 64-byte Ed25519 signature
5. THE Identity_Module SHALL implement verify(pub_key, message, signature) returning bool
6. WHEN a private key is dropped, THE Identity_Module SHALL zeroize the key material from memory
7. FOR ALL generated keypairs, signing then verifying with the corresponding public key SHALL return true

### Requirement 3: Pairing Proof Generation

**User Story:** As a developer, I want pairing proof generation, so that devices can verify operators possess the invite secret without revealing it.

#### Acceptance Criteria

1. THE Pairing_Module SHALL compute invite_proof as HMAC-SHA256(invite_secret, canonical_pair_request_transcript)
2. THE Pairing_Module SHALL include in the transcript: operator_id, device_id, operator_sign_pub, operator_kex_pub, nonce, timestamp
3. THE Pairing_Module SHALL implement verify_invite_proof(invite_secret, pair_request) returning bool
4. WHEN an invalid invite_secret is used, THE Pairing_Module SHALL return false from verify_invite_proof
5. THE Pairing_Module SHALL reject proofs where the timestamp is more than 5 minutes old
6. FOR ALL valid invite secrets and pair requests, generating then verifying the proof SHALL return true

### Requirement 4: Short Authentication String (SAS)

**User Story:** As a developer, I want SAS generation for MITM detection, so that users can verbally verify they're connected to the intended peer.

#### Acceptance Criteria

1. THE SAS_Module SHALL derive a 6-digit numeric code from the transcript hash
2. THE SAS_Module SHALL use the formula: code = (hash_bytes[0..4] as u32) % 1_000_000
3. THE SAS_Module SHALL include in the SAS transcript: both public keys, session_id, and nonces from both parties
4. WHEN both parties compute SAS from the same transcript, THE SAS_Module SHALL produce identical codes
5. THE SAS_Module SHALL format the code with leading zeros (e.g., "012345")
6. FOR ALL valid transcripts, SAS generation SHALL produce a 6-character string of digits

### Requirement 5: Envelope Encryption (Sealed Box)

**User Story:** As a developer, I want envelope encryption, so that messages can be securely transmitted to recipients with confidentiality and authenticity.

#### Acceptance Criteria

1. THE Envelope_Module SHALL implement seal(sender_sign_key, sender_kex_key, recipient_kex_pub, plaintext, aad) returning EnvelopeV1
2. THE Envelope_Module SHALL perform X25519 key exchange between sender ephemeral and recipient public key
3. THE Envelope_Module SHALL derive encryption key using HKDF-SHA256 with info="zrc_envelope_v1"
4. THE Envelope_Module SHALL encrypt plaintext using ChaCha20Poly1305 with a random 24-byte nonce
5. THE Envelope_Module SHALL sign the envelope header concatenated with ciphertext using sender's Ed25519 key
6. WHEN opening an envelope, THE Envelope_Module SHALL verify signature before attempting decryption
7. IF signature verification fails, THEN THE Envelope_Module SHALL return an error without attempting decryption
8. FOR ALL valid envelopes, sealing then opening with the correct recipient key SHALL return the original plaintext

### Requirement 6: Envelope Decryption (Open Box)

**User Story:** As a developer, I want envelope decryption, so that recipients can securely receive and verify messages.

#### Acceptance Criteria

1. THE Envelope_Module SHALL implement open(recipient_kex_key, sender_sign_pub, envelope) returning Result<plaintext, Error>
2. THE Envelope_Module SHALL verify the sender signature covers header + sender_kex_pub + aad + ciphertext
3. THE Envelope_Module SHALL perform X25519 key exchange between recipient private and sender ephemeral public
4. THE Envelope_Module SHALL derive decryption key using identical HKDF parameters as seal()
5. THE Envelope_Module SHALL decrypt ciphertext using ChaCha20Poly1305 with the envelope's nonce
6. IF decryption fails (authentication tag mismatch), THEN THE Envelope_Module SHALL return a DecryptionFailed error
7. THE Envelope_Module SHALL verify sender_id matches the public key used for signature verification

### Requirement 7: Session Ticket Signing

**User Story:** As a developer, I want session ticket signing, so that devices can issue short-lived capability tokens for session access.

#### Acceptance Criteria

1. THE Ticket_Module SHALL implement sign_ticket(device_sign_key, ticket_data) returning SessionTicketV1 with signature
2. THE Ticket_Module SHALL compute signature over canonical transcript of: ticket_id, session_id, operator_id, device_id, permissions, expires_at, session_binding
3. THE Ticket_Module SHALL implement verify_ticket(device_sign_pub, ticket) returning Result<(), Error>
4. WHEN a ticket's expires_at is in the past, THE Ticket_Module SHALL return TicketExpired error
5. THE Ticket_Module SHALL verify session_binding matches the expected value for the session
6. FOR ALL valid tickets, signing then verifying with the device's public key SHALL succeed

### Requirement 8: Session Cryptography

**User Story:** As a developer, I want session-level encryption, so that all control and frame data is end-to-end encrypted above the transport layer.

#### Acceptance Criteria

1. THE Session_Crypto_Module SHALL derive session keys from: session_binding + ticket_id + shared_secret
2. THE Session_Crypto_Module SHALL derive separate keys for each direction (host→controller, controller→host)
3. THE Session_Crypto_Module SHALL derive separate keys for each channel (control, frames, clipboard, files)
4. THE Session_Crypto_Module SHALL implement encrypt_control(key, sequence, plaintext) returning ciphertext
5. THE Session_Crypto_Module SHALL implement decrypt_control(key, sequence, ciphertext) returning Result<plaintext, Error>
6. THE Session_Crypto_Module SHALL include sequence number in the AEAD additional data to prevent replay
7. IF a sequence number is reused or out of order, THEN THE Session_Crypto_Module SHALL return ReplayDetected error
8. FOR ALL valid session data, encrypting then decrypting with correct keys and sequence SHALL return original plaintext

### Requirement 9: Directory Record Signing

**User Story:** As a developer, I want directory record signing, so that device presence information can be verified as authentic.

#### Acceptance Criteria

1. THE Directory_Module SHALL implement sign_record(device_sign_key, record_data) returning DirRecordV1 with signature
2. THE Directory_Module SHALL compute signature over canonical transcript of: subject_id, endpoints, ttl_seconds, timestamp
3. THE Directory_Module SHALL implement verify_record(device_sign_pub, record) returning Result<(), Error>
4. WHEN a record's timestamp + ttl_seconds is in the past, THE Directory_Module SHALL return RecordExpired error
5. THE Directory_Module SHALL reject records where subject_id doesn't match the signing key's derived ID
6. FOR ALL valid directory records, signing then verifying SHALL succeed

### Requirement 10: Replay Protection Primitives

**User Story:** As a developer, I want replay protection primitives, so that captured messages cannot be replayed to compromise sessions.

#### Acceptance Criteria

1. THE Replay_Module SHALL implement MonotonicCounter with increment() and current() methods
2. THE Replay_Module SHALL implement NonceTracker that rejects previously-seen nonces within a window
3. THE Replay_Module SHALL support configurable window size (default: 1024 nonces)
4. WHEN a nonce is checked, THE Replay_Module SHALL return whether it's valid (not seen) or invalid (replay)
5. THE Replay_Module SHALL efficiently handle out-of-order delivery within the window
6. IF a nonce is older than the window, THEN THE Replay_Module SHALL reject it as potentially replayed

### Requirement 11: Secure Memory Handling

**User Story:** As a developer, I want secure memory handling, so that sensitive cryptographic material is protected from memory disclosure attacks.

#### Acceptance Criteria

1. THE Crypto_Module SHALL zeroize all private keys when they go out of scope
2. THE Crypto_Module SHALL zeroize all derived session keys when sessions end
3. THE Crypto_Module SHALL zeroize intermediate key material after derivation
4. THE Crypto_Module SHALL use constant-time comparison for all signature and MAC verification
5. WHEN comparing secrets, THE Crypto_Module SHALL not leak timing information about the comparison result
