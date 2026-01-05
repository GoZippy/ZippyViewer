# Implementation Plan: zrc-security

## Overview

Implementation tasks for the security architecture and controls. This module defines threat model documentation, security controls implementation, and security testing infrastructure.

## Tasks

- [x] 1. Document threat model
  - [x] 1.1 Create THREAT_MODEL.md
    - Trust boundaries diagram
    - Threat actors enumeration
    - _Requirements: 1.1, 1.2, 1.3_
  - [x] 1.2 Document data flow diagrams
    - Encryption boundaries
    - Key material flow
    - _Requirements: 1.3, 1.4_
  - [x] 1.3 Document threat mitigations
    - Map mitigations to threats
    - Rate by likelihood and impact
    - _Requirements: 1.6, 1.7_
  - [x] 1.4 Create security assumptions document
    - Trusted components
    - _Requirements: 1.5_

- [x] 2. Implement identity pinning
  - [x] 2.1 Create IdentityVerifier struct
    - Pinned keys storage
    - _Requirements: 2.1, 2.2_
  - [x] 2.2 Implement verify_identity method
    - Constant-time comparison
    - _Requirements: 2.2, 2.4_
  - [x] 2.3 Implement pin_identity method
    - First contact and SAS verification
    - _Requirements: 2.1_
  - [x] 2.4 Write property test for identity binding

    - **Property 2: Identity Binding**
    - **Validates: Requirements 2.1, 2.2, 2.4**

- [x] 3. Implement SAS verification
  - [x] 3.1 Create SasVerification struct
    - Transcript computation
    - _Requirements: 2.3, 2.6_
  - [x] 3.2 Implement compute_sas method
    - SHA-256 hash to 6-digit decimal
    - _Requirements: 2.3_
  - [x] 3.3 Implement compute_transcript method
    - Include handshake messages and IDs
    - _Requirements: 2.3_

- [x] 4. Implement replay protection
  - [x] 4.1 Create ReplayProtection struct
    - Sliding window bitmap
    - _Requirements: 3.1, 3.4, 3.5_
  - [x] 4.2 Implement check_and_update method
    - Window management
    - Duplicate detection
    - _Requirements: 3.4, 3.5_
  - [x] 4.3 Create TicketValidator struct
    - Timestamp-based validation
    - _Requirements: 3.2, 3.3_
  - [x] 4.4 Write property test for replay window

    - **Property 3: Replay Window**
    - **Validates: Requirements 3.1, 3.4, 3.5**

- [x] 5. Implement session key derivation
  - [x] 5.1 Create SessionKeyDeriver struct
    - HKDF-based derivation
    - _Requirements: 7.1, 7.2, 7.3_
  - [x] 5.2 Implement derive_keys method
    - Separate keys per direction and channel
    - _Requirements: 7.2, 7.3_
  - [x]* 5.3 Write property test for key separation
    - **Property 4: Key Separation**
    - **Validates: Requirements 7.2, 7.3**

- [x] 6. Implement rate limiting
  - [x] 6.1 Create SecurityRateLimiter struct
    - governor-based rate limiting
    - _Requirements: 10.1, 10.2, 10.3_
  - [x] 6.2 Implement check methods
    - check_auth, check_pairing, check_session
    - _Requirements: 10.1, 10.2, 10.3_
  - [x] 6.3 Implement exponential backoff
    - _Requirements: 10.4_ (Note: governor crate provides this)
  - [x]* 6.4 Write property test for rate limit enforcement
    - **Property 6: Rate Limit Enforcement**
    - **Validates: Requirements 10.1, 10.2, 10.3**

- [x] 7. Implement audit logging
  - [x] 7.1 Create AuditLogger struct
    - Signing key for entries
    - Log writer abstraction
    - _Requirements: 9.1, 9.6_
  - [x] 7.2 Implement log method
    - Create signed audit entry
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_
  - [x] 7.3 Implement verify_log method
    - Verify entry signatures
    - _Requirements: 9.7_
  - [x] 7.4 Define SecurityEvent enum
    - All security-relevant events
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_
  - [x]* 7.5 Write property test for audit integrity
    - **Property 5: Audit Integrity**
    - **Validates: Requirements 9.6, 9.7**

- [x] 8. Implement downgrade protection
  - [x] 8.1 Create algorithm version checking
    - Minimum cipher suite requirements
    - _Requirements: 4.1, 4.2_
  - [x] 8.2 Implement handshake algorithm verification
    - Include algorithm IDs in signed handshake
    - _Requirements: 4.4, 4.5_
  - [x] 8.3 Implement downgrade detection logging
    - _Requirements: 4.6_

- [x] 9. Implement key compromise recovery
  - [x] 9.1 Create key rotation mechanism
    - Device and operator key rotation
    - _Requirements: 5.1, 5.2_
  - [x] 9.2 Implement rotation propagation
    - Notify paired endpoints
    - _Requirements: 5.3_
  - [x] 9.3 Implement emergency revocation
    - _Requirements: 5.5_
  - [x] 9.4 Implement rotation history
    - _Requirements: 5.6_

- [x] 10. Implement security testing infrastructure
  - [x] 10.1 Create fuzzing targets
    - Protobuf parsing
    - Invite parsing
    - Envelope decryption
    - _Requirements: 13.1_
  - [x] 10.2 Create property tests
    - Replay protection
    - Key derivation
    - SAS computation
    - _Requirements: 13.2_
  - [x] 10.3 Configure static analysis
    - cargo-audit, clippy security lints
    - _Requirements: 13.3_
    - Created clippy.toml and .cargo-audit.toml configuration files

- [x] 11. Checkpoint - Verify all security controls
  - [x] All property tests implemented (Properties 2, 3, 4, 5, 6)
  - [x] Fuzzing targets created (protobuf_parsing, invite_parsing, envelope_decryption)
  - [x] Threat model coverage reviewed (THREAT_MODEL.md complete)
  - [x] Static analysis configured (clippy.toml, .cargo-audit.toml)
  - All security controls implemented and tested

## Notes

- Tasks marked with `*` are optional property-based tests
- Security controls are cross-cutting across all components
- Threat model should be reviewed with each major release
- Fuzzing targets should run in CI nightly
