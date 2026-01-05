# zrc-security

Security controls and threat model for the ZRC system.

## Overview

This crate provides security controls including:
- Identity pinning and verification (MITM protection)
- SAS (Short Authentication String) verification
- Replay attack prevention
- Session key derivation
- Rate limiting
- Audit logging with cryptographic signing
- Downgrade attack prevention
- Key compromise recovery

## Threat Model

See [THREAT_MODEL.md](THREAT_MODEL.md) for the complete threat model documentation.

## Usage

### Identity Pinning

```rust
use zrc_security::identity::{IdentityVerifier, PeerId};
use zrc_proto::v1::PublicKeyBundleV1;

let mut verifier = IdentityVerifier::new();
let peer_id: PeerId = [0u8; 32]; // 32-byte peer ID
let keys = PublicKeyBundleV1 { /* ... */ };

// Pin identity after first contact or SAS verification
verifier.pin_identity(peer_id, keys.clone())?;

// Verify on subsequent connections
verifier.verify_identity(&peer_id, &keys)?;
```

### Replay Protection

```rust
use zrc_security::replay::ReplayProtection;

let mut rp = ReplayProtection::new(1024); // 1024 packet window
rp.check_and_update(sequence_number)?;
```

### Session Key Derivation

```rust
use zrc_security::session_keys::SessionKeyDeriver;

let keys = SessionKeyDeriver::derive_keys(
    &shared_secret,
    &session_id,
    &initiator_id,
    &responder_id,
);
```

### Rate Limiting

```rust
use zrc_security::rate_limit::{SecurityRateLimiter, RateLimitConfig};

let limiter = SecurityRateLimiter::new(RateLimitConfig::default());
limiter.check_auth("source_ip")?;
```

### Audit Logging

```rust
use zrc_security::audit::{AuditLogger, SecurityEvent, FileAuditLogWriter};
use ed25519_dalek::SigningKey;

let signing_key = SigningKey::generate(&mut OsRng);
let writer = Box::new(FileAuditLogWriter::new(log_path));
let logger = AuditLogger::new(signing_key, writer);

logger.log(SecurityEvent::SessionStarted {
    session_id: "session_123".to_string(),
    operator_id: "operator_456".to_string(),
    device_id: "device_789".to_string(),
})?;
```

## Testing

### Property Tests

Run property tests with:
```bash
cargo test -p zrc-security --features proptest
```

### Fuzzing

Fuzzing targets are in `fuzz/fuzz_targets/`. Run with:
```bash
cargo fuzz run -p zrc-security-fuzz protobuf_parsing
```

## Requirements

This module implements requirements from `.kiro/specs/zrc-security/requirements.md`:
- Requirement 1: Threat Model Documentation
- Requirement 2: MITM Protection
- Requirement 3: Replay Attack Prevention
- Requirement 4: Downgrade Attack Prevention
- Requirement 5: Key Compromise Recovery
- Requirement 7: Session Security
- Requirement 9: Audit and Logging
- Requirement 10: Rate Limiting and Abuse Prevention
- Requirement 13: Security Testing
