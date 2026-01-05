# Design Document: zrc-crypto

## Overview

The zrc-crypto crate provides all cryptographic primitives for the ZRC system using pure-Rust implementations. This crate centralizes security-critical operations to enable focused auditing and ensures consistent cryptographic behavior across all platforms. No OpenSSL dependency is used.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        zrc-crypto                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  transcript  │  │   identity   │  │   pairing    │          │
│  │  (hashing)   │  │   (keys)     │  │   (proofs)   │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │     sas      │  │   envelope   │  │    ticket    │          │
│  │  (MITM det)  │  │  (seal/open) │  │  (sign/ver)  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │session_crypto│  │  directory   │  │    replay    │          │
│  │  (E2EE)      │  │  (records)   │  │  (counters)  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│  Dependencies: ed25519-dalek, x25519-dalek, chacha20poly1305,   │
│                sha2, hmac, hkdf, zeroize, rand                   │
└─────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Transcript Module

```rust
/// Canonical transcript for deterministic hashing
pub struct Transcript {
    hasher: Sha256,
    finalized: bool,
}

impl Transcript {
    pub fn new(domain: &str) -> Self;
    
    /// Append tagged data: tag(4) || len(4) || data
    pub fn append(&mut self, tag: u32, data: &[u8]) -> Result<(), TranscriptError>;
    
    /// Finalize and return 32-byte hash
    pub fn finalize(self) -> [u8; 32];
    
    /// Clone for branching derivations
    pub fn fork(&self) -> Self;
}

// Standard tags
pub const TAG_DEVICE_ID: u32 = 1;
pub const TAG_OPERATOR_ID: u32 = 2;
pub const TAG_SIGN_PUB: u32 = 3;
pub const TAG_KEX_PUB: u32 = 4;
pub const TAG_NONCE: u32 = 5;
pub const TAG_TIMESTAMP: u32 = 6;
pub const TAG_SESSION_ID: u32 = 7;
pub const TAG_PERMISSIONS: u32 = 8;
```

### Identity Module

```rust
/// Device or operator identity keypair
pub struct Identity {
    sign_key: SigningKey,  // Ed25519
    kex_key: StaticSecret, // X25519
}

impl Identity {
    /// Generate new random identity
    pub fn generate() -> Self;
    
    /// Derive ID from signing public key
    pub fn id(&self) -> [u8; 32];
    
    /// Get public key bundle
    pub fn public_bundle(&self) -> PublicKeyBundle;
    
    /// Sign message with Ed25519
    pub fn sign(&self, message: &[u8]) -> Signature;
    
    /// Perform X25519 key exchange
    pub fn key_exchange(&self, peer_pub: &[u8; 32]) -> SharedSecret;
}

impl Drop for Identity {
    fn drop(&mut self) {
        self.sign_key.zeroize();
        self.kex_key.zeroize();
    }
}

/// Verify Ed25519 signature
pub fn verify_signature(
    pub_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64]
) -> Result<(), SignatureError>;
```

### Pairing Module

```rust
/// Generate invite proof for pairing
pub fn generate_invite_proof(
    invite_secret: &[u8; 32],
    pair_request: &PairRequestTranscript,
) -> [u8; 32];

/// Verify invite proof
pub fn verify_invite_proof(
    invite_secret: &[u8; 32],
    pair_request: &PairRequestTranscript,
    proof: &[u8; 32],
) -> Result<(), ProofError>;

/// Transcript data for pairing
pub struct PairRequestTranscript {
    pub operator_id: [u8; 32],
    pub device_id: [u8; 32],
    pub operator_sign_pub: [u8; 32],
    pub operator_kex_pub: [u8; 32],
    pub nonce: [u8; 32],
    pub timestamp: u64,
}
```

### SAS Module

```rust
/// Compute 6-digit Short Authentication String
pub fn compute_sas(transcript: &SasTranscript) -> String;

/// SAS transcript data
pub struct SasTranscript {
    pub device_sign_pub: [u8; 32],
    pub operator_sign_pub: [u8; 32],
    pub device_kex_pub: [u8; 32],
    pub operator_kex_pub: [u8; 32],
    pub session_id: [u8; 32],
    pub device_nonce: [u8; 32],
    pub operator_nonce: [u8; 32],
}

// SAS derivation: first 4 bytes of hash as u32, mod 1_000_000
// Format with leading zeros: format!("{:06}", code)
```

### Envelope Module

```rust
/// Seal a message for a recipient
pub fn seal_envelope(
    sender_identity: &Identity,
    recipient_kex_pub: &[u8; 32],
    msg_type: MsgType,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<EnvelopeV1, EnvelopeError>;

/// Open a sealed envelope
pub fn open_envelope(
    recipient_identity: &Identity,
    sender_sign_pub: &[u8; 32],
    envelope: &EnvelopeV1,
) -> Result<Vec<u8>, EnvelopeError>;

// Envelope construction:
// 1. Generate ephemeral X25519 keypair
// 2. Perform ECDH: shared = X25519(ephemeral_priv, recipient_pub)
// 3. Derive key: HKDF-SHA256(shared, info="zrc_envelope_v1")
// 4. Encrypt: ChaCha20Poly1305(key, nonce, plaintext, aad)
// 5. Sign: Ed25519(sender_priv, header || ephemeral_pub || aad || ciphertext)
```

### Ticket Module

```rust
/// Sign a session ticket
pub fn sign_ticket(
    device_identity: &Identity,
    ticket: &SessionTicketData,
) -> Result<SessionTicketV1, TicketError>;

/// Verify a session ticket
pub fn verify_ticket(
    device_sign_pub: &[u8; 32],
    ticket: &SessionTicketV1,
    expected_binding: &[u8; 32],
    now: u64,
) -> Result<(), TicketError>;

pub struct SessionTicketData {
    pub ticket_id: [u8; 16],
    pub session_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub device_id: [u8; 32],
    pub permissions: u32,
    pub expires_at: u64,
    pub session_binding: [u8; 32],
}
```

### Session Crypto Module

```rust
/// Derive session encryption keys
pub fn derive_session_keys(
    session_binding: &[u8; 32],
    ticket_id: &[u8; 16],
    shared_secret: &[u8; 32],
) -> SessionKeys;

pub struct SessionKeys {
    pub host_to_controller: ChannelKeys,
    pub controller_to_host: ChannelKeys,
}

pub struct ChannelKeys {
    pub control: [u8; 32],
    pub frames: [u8; 32],
    pub clipboard: [u8; 32],
    pub files: [u8; 32],
}

/// Encrypt control message
pub fn encrypt_control(
    key: &[u8; 32],
    sequence: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError>;

/// Decrypt control message
pub fn decrypt_control(
    key: &[u8; 32],
    sequence: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError>;

// AAD includes sequence number to prevent replay
// Nonce derived from sequence: nonce = sequence || zeros
```

### Replay Protection Module

```rust
/// Monotonic counter for sequence numbers
pub struct MonotonicCounter {
    value: AtomicU64,
}

impl MonotonicCounter {
    pub fn new(initial: u64) -> Self;
    pub fn increment(&self) -> u64;
    pub fn current(&self) -> u64;
}

/// Sliding window nonce tracker
pub struct NonceTracker {
    window_start: u64,
    seen_bitmap: BitVec,
    window_size: usize,
}

impl NonceTracker {
    pub fn new(window_size: usize) -> Self;
    
    /// Check if nonce is valid (not seen before)
    pub fn check_and_mark(&mut self, nonce: u64) -> Result<(), ReplayError>;
}
```

## Data Models

### Key Derivation Hierarchy

```
Identity Seed
    │
    ├── Ed25519 Signing Key
    │       └── Device/Operator ID (SHA256 of pub key)
    │
    └── X25519 Key Exchange Key

Session Establishment
    │
    ├── ECDH Shared Secret
    │       │
    │       └── HKDF with session_binding + ticket_id
    │               │
    │               ├── Host→Controller Keys
    │               │       ├── Control Channel Key
    │               │       ├── Frames Channel Key
    │               │       ├── Clipboard Channel Key
    │               │       └── Files Channel Key
    │               │
    │               └── Controller→Host Keys
    │                       └── (same structure)
```

### Algorithm Parameters

| Algorithm | Parameters |
|-----------|------------|
| Ed25519 | Standard (RFC 8032) |
| X25519 | Standard (RFC 7748) |
| ChaCha20Poly1305 | 256-bit key, 96-bit nonce, 128-bit tag |
| HKDF | SHA-256, variable output |
| HMAC | SHA-256 |
| SHA-256 | Standard |

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Transcript Determinism
*For any* sequence of append operations with identical tags and data, the finalized hash SHALL be identical across all platforms and invocations.
**Validates: Requirements 1.4**

### Property 2: Signature Round-Trip
*For any* message and identity keypair, signing then verifying with the corresponding public key SHALL succeed.
**Validates: Requirements 2.7**

### Property 3: Envelope Round-Trip
*For any* plaintext and valid sender/recipient keypairs, sealing then opening an envelope SHALL return the original plaintext.
**Validates: Requirements 5.8, 6.7**

### Property 4: SAS Consistency
*For any* SAS transcript, both parties computing SAS from identical transcript data SHALL produce identical 6-digit codes.
**Validates: Requirements 4.4**

### Property 5: Ticket Expiry Enforcement
*For any* ticket with expires_at in the past, verification SHALL fail with TicketExpired error.
**Validates: Requirements 7.4**

### Property 6: Replay Detection
*For any* nonce that has been previously marked in a NonceTracker, subsequent check_and_mark calls SHALL return ReplayError.
**Validates: Requirements 10.4**

### Property 7: Key Zeroization
*For any* Identity or SessionKeys that is dropped, the underlying key material SHALL be overwritten with zeros before deallocation.
**Validates: Requirements 11.1, 11.2, 11.3**

## Error Handling

| Error Type | Condition | Recovery |
|------------|-----------|----------|
| SignatureError | Invalid signature | Reject message, log event |
| DecryptionError | AEAD tag mismatch | Reject message, log event |
| TicketExpired | expires_at < now | Request new ticket |
| ReplayError | Duplicate nonce | Drop message, log event |
| TranscriptError | Append after finalize | Programming error, panic |

## Testing Strategy

### Unit Tests
- Test vector verification for all algorithms
- Edge cases: empty inputs, max-size inputs
- Error condition coverage

### Property-Based Tests
- Transcript determinism (100+ random sequences)
- Signature round-trip (100+ random messages)
- Envelope round-trip (100+ random plaintexts)
- SAS consistency (100+ random transcripts)
- Replay detection (100+ random nonce sequences)

### Security Tests
- Timing attack resistance for comparisons
- Zeroization verification (where testable)
- Fuzzing for parser/decoder paths
