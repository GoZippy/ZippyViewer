# Design Document: zrc-security

## Overview

The zrc-security module defines the security architecture, threat model, and security controls for the ZRC system. This document establishes security requirements that all components must satisfy and provides guidance for security-conscious implementation.

## Threat Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ZRC Threat Model                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  THREAT ACTORS                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │   │
│  │  │  Network    │  │  Malicious  │  │  Malicious  │  │Compromised│  │   │
│  │  │  Attacker   │  │  Directory  │  │   Relay     │  │ Endpoint  │  │   │
│  │  │  (MITM)     │  │   Node      │  │   Server    │  │           │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  TRUST BOUNDARIES                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                       │   │
│  │    ┌─────────┐         ┌─────────┐         ┌─────────┐              │   │
│  │    │ Agent   │◄───────►│ Network │◄───────►│Controller│             │   │
│  │    │(Trusted)│  E2EE   │(Untrust)│  E2EE   │(Trusted) │             │   │
│  │    └─────────┘         └─────────┘         └─────────┘              │   │
│  │         │                   │                   │                    │   │
│  │         ▼                   ▼                   ▼                    │   │
│  │    ┌─────────┐         ┌─────────┐         ┌─────────┐              │   │
│  │    │  OS     │         │Directory│         │  OS     │              │   │
│  │    │Keystore │         │  Relay  │         │Keystore │              │   │
│  │    │(Trusted)│         │(Untrust)│         │(Trusted)│              │   │
│  │    └─────────┘         └─────────┘         └─────────┘              │   │
│  │                                                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  SECURITY PROPERTIES                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  • Confidentiality: E2EE between paired endpoints                    │   │
│  │  • Integrity: Signed messages, verified identities                   │   │
│  │  • Authenticity: Identity pinning after pairing                      │   │
│  │  • Availability: Graceful degradation, no single point of failure    │   │
│  │  • Non-repudiation: Signed audit logs                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Security Controls

### Identity Pinning

```rust
/// Identity verification for MITM protection
pub struct IdentityVerifier {
    pinned_keys: HashMap<PeerId, PinnedIdentity>,
}

pub struct PinnedIdentity {
    pub sign_pub: [u8; 32],
    pub kex_pub: [u8; 32],
    pub pinned_at: SystemTime,
    pub last_verified: SystemTime,
}

impl IdentityVerifier {
    /// Verify peer identity matches pinned keys
    pub fn verify_identity(
        &self,
        peer_id: &PeerId,
        presented_keys: &PublicKeyBundleV1,
    ) -> Result<(), SecurityError> {
        let pinned = self.pinned_keys.get(peer_id)
            .ok_or(SecurityError::UnknownPeer)?;
        
        // Constant-time comparison
        if !constant_time_eq(&pinned.sign_pub, &presented_keys.sign_pub) {
            return Err(SecurityError::IdentityMismatch {
                peer_id: peer_id.clone(),
                expected: hex::encode(&pinned.sign_pub[..8]),
                received: hex::encode(&presented_keys.sign_pub[..8]),
            });
        }
        
        if !constant_time_eq(&pinned.kex_pub, &presented_keys.kex_pub) {
            return Err(SecurityError::IdentityMismatch {
                peer_id: peer_id.clone(),
                expected: hex::encode(&pinned.kex_pub[..8]),
                received: hex::encode(&presented_keys.kex_pub[..8]),
            });
        }
        
        Ok(())
    }
    
    /// Pin new identity (first contact or after SAS verification)
    pub fn pin_identity(
        &mut self,
        peer_id: PeerId,
        keys: PublicKeyBundleV1,
    ) {
        self.pinned_keys.insert(peer_id, PinnedIdentity {
            sign_pub: keys.sign_pub,
            kex_pub: keys.kex_pub,
            pinned_at: SystemTime::now(),
            last_verified: SystemTime::now(),
        });
    }
}
```

### SAS Verification

```rust
/// Short Authentication String for MITM detection
pub struct SasVerification;

impl SasVerification {
    /// Compute SAS from session transcript
    pub fn compute_sas(transcript: &SessionTranscript) -> String {
        // Hash the transcript
        let hash = Sha256::digest(&transcript.to_bytes());
        
        // Convert to 6-digit decimal SAS
        let value = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        format!("{:06}", value % 1_000_000)
    }
    
    /// Compute transcript from handshake messages
    pub fn compute_transcript(
        initiator_hello: &[u8],
        responder_hello: &[u8],
        initiator_id: &[u8],
        responder_id: &[u8],
    ) -> SessionTranscript {
        let mut transcript = Vec::new();
        transcript.extend_from_slice(b"ZRC-SAS-v1");
        transcript.extend_from_slice(initiator_hello);
        transcript.extend_from_slice(responder_hello);
        transcript.extend_from_slice(initiator_id);
        transcript.extend_from_slice(responder_id);
        
        SessionTranscript(transcript)
    }
}
```

### Replay Protection

```rust
/// Replay attack prevention
pub struct ReplayProtection {
    /// Sliding window of seen sequence numbers
    window: BitVec,
    /// Highest sequence number seen
    highest_seq: u64,
    /// Window size
    window_size: u64,
}

impl ReplayProtection {
    pub fn new(window_size: u64) -> Self {
        Self {
            window: BitVec::repeat(false, window_size as usize),
            highest_seq: 0,
            window_size,
        }
    }
    
    /// Check if sequence number is valid (not replayed)
    pub fn check_and_update(&mut self, seq: u64) -> Result<(), SecurityError> {
        if seq == 0 {
            return Err(SecurityError::InvalidSequence);
        }
        
        if seq > self.highest_seq {
            // New highest - shift window
            let shift = (seq - self.highest_seq).min(self.window_size);
            self.window.shift_left(shift as usize);
            self.highest_seq = seq;
            self.window.set((self.window_size - 1) as usize, true);
            Ok(())
        } else if self.highest_seq - seq >= self.window_size {
            // Too old - outside window
            Err(SecurityError::ReplayDetected { sequence: seq })
        } else {
            // Within window - check if seen
            let index = (self.window_size - 1 - (self.highest_seq - seq)) as usize;
            if self.window[index] {
                Err(SecurityError::ReplayDetected { sequence: seq })
            } else {
                self.window.set(index, true);
                Ok(())
            }
        }
    }
}

/// Timestamp-based replay protection for tickets
pub struct TicketValidator {
    max_age: Duration,
    max_future: Duration,
}

impl TicketValidator {
    pub fn validate_timestamp(&self, ticket_time: u64) -> Result<(), SecurityError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if ticket_time > now + self.max_future.as_secs() {
            return Err(SecurityError::TicketFromFuture);
        }
        
        if now > ticket_time + self.max_age.as_secs() {
            return Err(SecurityError::TicketExpired);
        }
        
        Ok(())
    }
}
```

### Session Key Derivation

```rust
/// Secure session key derivation
pub struct SessionKeyDeriver;

impl SessionKeyDeriver {
    /// Derive session keys from shared secret
    pub fn derive_keys(
        shared_secret: &SharedSecret,
        session_id: &[u8; 32],
        initiator_id: &[u8; 32],
        responder_id: &[u8; 32],
    ) -> SessionKeys {
        let mut info = Vec::new();
        info.extend_from_slice(b"ZRC-SESSION-KEYS-v1");
        info.extend_from_slice(session_id);
        info.extend_from_slice(initiator_id);
        info.extend_from_slice(responder_id);
        
        let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        
        let mut keys = SessionKeys::default();
        
        // Derive separate keys for each direction and channel
        hk.expand(b"initiator-to-responder-control", &mut keys.i2r_control).unwrap();
        hk.expand(b"responder-to-initiator-control", &mut keys.r2i_control).unwrap();
        hk.expand(b"initiator-to-responder-frames", &mut keys.i2r_frames).unwrap();
        hk.expand(b"responder-to-initiator-frames", &mut keys.r2i_frames).unwrap();
        hk.expand(b"initiator-to-responder-files", &mut keys.i2r_files).unwrap();
        hk.expand(b"responder-to-initiator-files", &mut keys.r2i_files).unwrap();
        
        keys
    }
}

pub struct SessionKeys {
    pub i2r_control: [u8; 32],
    pub r2i_control: [u8; 32],
    pub i2r_frames: [u8; 32],
    pub r2i_frames: [u8; 32],
    pub i2r_files: [u8; 32],
    pub r2i_files: [u8; 32],
}
```

### Rate Limiting

```rust
use governor::{Quota, RateLimiter, state::keyed::DefaultKeyedStateStore};

/// Rate limiter for abuse prevention
pub struct SecurityRateLimiter {
    auth_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    pairing_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    session_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
}

impl SecurityRateLimiter {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            auth_limiter: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(config.auth_per_minute).unwrap())
            ),
            pairing_limiter: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(config.pairing_per_minute).unwrap())
            ),
            session_limiter: RateLimiter::keyed(
                Quota::per_minute(NonZeroU32::new(config.session_per_minute).unwrap())
            ),
        }
    }
    
    pub fn check_auth(&self, source: &str) -> Result<(), SecurityError> {
        self.auth_limiter.check_key(&source.to_string())
            .map_err(|_| SecurityError::RateLimited { 
                operation: "authentication",
                retry_after: Duration::from_secs(60),
            })
    }
    
    pub fn check_pairing(&self, source: &str) -> Result<(), SecurityError> {
        self.pairing_limiter.check_key(&source.to_string())
            .map_err(|_| SecurityError::RateLimited {
                operation: "pairing",
                retry_after: Duration::from_secs(60),
            })
    }
}
```

### Audit Logging

```rust
/// Signed audit log for non-repudiation
pub struct AuditLogger {
    signing_key: SigningKey,
    log_writer: Box<dyn AuditLogWriter>,
}

impl AuditLogger {
    /// Log security event with signature
    pub fn log(&self, event: SecurityEvent) -> Result<(), AuditError> {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: event.event_type(),
            actor: event.actor(),
            target: event.target(),
            details: event.details(),
            signature: Vec::new(), // Filled below
        };
        
        // Sign the entry
        let entry_bytes = entry.to_canonical_bytes();
        let signature = self.signing_key.sign(&entry_bytes);
        
        let signed_entry = AuditEntry {
            signature: signature.to_bytes().to_vec(),
            ..entry
        };
        
        self.log_writer.write(signed_entry)
    }
    
    /// Verify audit log integrity
    pub fn verify_log(&self, entries: &[AuditEntry]) -> Result<(), AuditError> {
        for entry in entries {
            let entry_bytes = entry.to_canonical_bytes_without_signature();
            let signature = Signature::from_bytes(&entry.signature)?;
            
            self.signing_key.verify(&entry_bytes, &signature)
                .map_err(|_| AuditError::IntegrityViolation { entry_id: entry.id })?;
        }
        Ok(())
    }
}

pub enum SecurityEvent {
    AuthenticationAttempt { success: bool, source: String },
    PairingRequest { operator_id: String, device_id: String },
    PairingApproved { operator_id: String, device_id: String },
    PairingRevoked { operator_id: String, device_id: String, reason: String },
    SessionStarted { session_id: String, operator_id: String, device_id: String },
    SessionEnded { session_id: String, reason: String },
    PermissionEscalation { session_id: String, new_permissions: u32 },
    IdentityMismatch { peer_id: String },
    ReplayAttempt { sequence: u64 },
    RateLimitExceeded { source: String, operation: String },
}
```

## Correctness Properties

### Property 1: E2EE Guarantee
*For any* session data, only the paired agent and controller SHALL be able to decrypt the content.
**Validates: Requirements 2.1, 7.1**

### Property 2: Identity Binding
*For any* connection after pairing, the peer's identity keys SHALL match the pinned keys from pairing.
**Validates: Requirements 2.1, 2.2, 2.4**

### Property 3: Replay Window
*For any* message with sequence number outside the replay window, the message SHALL be rejected.
**Validates: Requirements 3.1, 3.4, 3.5**

### Property 4: Key Separation
*For any* session, each direction and channel type SHALL use independently derived keys.
**Validates: Requirements 7.2, 7.3**

### Property 5: Audit Integrity
*For any* audit log entry, the signature SHALL be verifiable with the device's signing key.
**Validates: Requirements 9.6, 9.7**

### Property 6: Rate Limit Enforcement
*For any* source exceeding rate limits, subsequent requests SHALL be rejected until the window resets.
**Validates: Requirements 10.1, 10.2, 10.3**

## Security Testing

### Fuzzing Targets

```rust
// Fuzz targets for security-critical parsers
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz protobuf parsing
    let _ = EnvelopeV1::decode(data);
});

fuzz_target!(|data: &[u8]| {
    // Fuzz invite parsing
    let _ = InviteV1::decode(data);
});

fuzz_target!(|data: &[u8]| {
    // Fuzz encrypted envelope decryption
    let key = [0u8; 32];
    let _ = decrypt_envelope(data, &key);
});
```

### Property Tests

```rust
#[cfg(test)]
mod security_tests {
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn replay_protection_rejects_duplicates(seq in 1u64..1000000) {
            let mut rp = ReplayProtection::new(64);
            assert!(rp.check_and_update(seq).is_ok());
            assert!(rp.check_and_update(seq).is_err());
        }
        
        #[test]
        fn key_derivation_is_deterministic(
            secret in prop::array::uniform32(any::<u8>()),
            session_id in prop::array::uniform32(any::<u8>()),
        ) {
            let keys1 = SessionKeyDeriver::derive_keys(&secret, &session_id, &[0;32], &[1;32]);
            let keys2 = SessionKeyDeriver::derive_keys(&secret, &session_id, &[0;32], &[1;32]);
            assert_eq!(keys1.i2r_control, keys2.i2r_control);
        }
        
        #[test]
        fn sas_is_consistent(
            hello1 in prop::collection::vec(any::<u8>(), 32),
            hello2 in prop::collection::vec(any::<u8>(), 32),
        ) {
            let transcript = SasVerification::compute_transcript(&hello1, &hello2, &[0;32], &[1;32]);
            let sas1 = SasVerification::compute_sas(&transcript);
            let sas2 = SasVerification::compute_sas(&transcript);
            assert_eq!(sas1, sas2);
            assert_eq!(sas1.len(), 6);
        }
    }
}
```
