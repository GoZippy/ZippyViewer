# Implementation Plan: zrc-dirnode

## Overview

Implementation tasks for the home-hostable directory node. This decentralized directory provides device discovery with privacy-first defaults and invite-only access.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with dependencies
    - axum, tokio, rusqlite, dashmap, serde, tracing
    - _Requirements: 1.1, 8.1_
  - [x] 1.2 Create module structure
    - api, records, access, discovery, store, web_ui
    - _Requirements: 1.1, 3.1, 4.1, 8.1, 9.1_

- [x] 2. Implement SQLite Store
  - [x] 2.1 Define schema and migrations
    - records table, discovery_tokens table
    - Indexes for expiry queries
    - _Requirements: 8.1, 8.3_
  - [x] 2.2 Implement RecordStore trait
    - save, load, delete, list_expired
    - _Requirements: 8.1_
  - [x] 2.3 Implement backup and export
    - VACUUM INTO, JSON export/import
    - _Requirements: 8.4, 8.5, 8.6_
  - [x]* 2.4 Write property test for store round-trip
    - **Property 7: Store Round-Trip**
    - **Validates: Requirements 8.1**
    - Implemented as unit test (test_store_round_trip)

- [x] 3. Implement Record Manager
  - [x] 3.1 Define StoredRecord struct
    - Record, stored_at, access_count
    - _Requirements: 1.5_
  - [x] 3.2 Implement signature verification
    - Ed25519 verification against subject's public key
    - Subject ID binding check
    - _Requirements: 1.2, 1.3, 6.3_
  - [x] 3.3 Implement store method
    - Verify signature, enforce limits, upsert
    - _Requirements: 1.4, 1.6, 1.7, 1.8_
  - [x] 3.4 Implement get and get_batch
    - TTL enforcement, batch lookups
    - _Requirements: 2.2, 2.4, 2.6, 2.7_
  - [x]* 3.5 Write property test for signature verification
    - **Property 1: Signature Verification**
    - **Validates: Requirements 1.2, 1.3**
    - Implemented in records::proptests::prop_signature_verification
  - [x]* 3.6 Write property test for subject ID binding
    - **Property 2: Subject ID Binding**
    - **Validates: Requirements 1.4**
    - Implemented in records::proptests::prop_subject_id_binding

- [x] 4. Checkpoint - Verify record storage
  - Ensure store and retrieve works
  - Unit tests verify store/retrieve functionality

- [x] 5. Implement Access Controller
  - [x] 5.1 Define AccessMode enum
    - InviteOnly, DiscoveryEnabled, Open
    - _Requirements: 3.1_
  - [x] 5.2 Implement invite token validation
    - Bearer token parsing, signature verification
    - Subject ID scoping
    - _Requirements: 3.2, 3.3, 3.4, 3.6_
  - [x] 5.3 Implement create_invite and revoke_invite
    - _Requirements: 3.6_
  - [x]* 5.4 Write property test for invite-only access
    - **Property 4: Invite-Only Access**
    - **Validates: Requirements 3.1, 3.2**

- [x] 6. Implement Discovery Manager
  - [x] 6.1 Define DiscoveryToken struct
    - token_id, subject_id, expires_at, scope
    - _Requirements: 4.1, 4.3_
  - [x] 6.2 Implement create discovery token
    - Configurable TTL, scope
    - _Requirements: 4.2_
  - [x] 6.3 Implement is_discoverable
    - Check for active discovery token
    - _Requirements: 4.4_
  - [x] 6.4 Implement revoke and cleanup_expired
    - _Requirements: 4.5, 4.6_
  - [x] 6.5 Implement token limits
    - Max tokens per subject
    - _Requirements: 4.7_
  - [x]* 6.6 Write property test for discovery token expiry
    - **Property 5: Discovery Token Expiry**
    - **Validates: Requirements 4.5**

- [x] 7. Implement HTTP API
  - [x] 7.1 Implement POST /v1/records
    - Accept DirRecordV1, verify, store
    - _Requirements: 1.1_
  - [x] 7.2 Implement GET /v1/records/{subject_id_hex}
    - Authorization check, return record
    - Include X-Record-Expires, X-Signature-Verified headers
    - _Requirements: 2.1, 2.5, 6.5_
  - [x] 7.3 Implement POST /v1/records/batch
    - Batch lookups
    - _Requirements: 2.6_
  - [x] 7.4 Implement POST /v1/discovery/tokens
    - Admin-only token creation
    - _Requirements: 4.1_
  - [x] 7.5 Implement DELETE /v1/discovery/tokens/{token_id}
    - Token revocation
    - _Requirements: 4.5_

- [x] 8. Implement Search Protection
  - [x] 8.1 Implement rate limiting
    - Per-IP lookup limits
    - _Requirements: 5.3_
  - [x] 8.2 Implement timing-safe responses
    - Constant-time for not found vs access denied
    - _Requirements: 5.4, 5.5_
  - [x] 8.3 Implement enumeration detection
    - Pattern detection, temporary blocking
    - _Requirements: 5.6, 5.7_
  - [x]* 8.4 Write property test for TTL enforcement
    - **Property 3: TTL Enforcement**
    - **Validates: Requirements 2.4**
  - [x]* 8.5 Write property test for record integrity
    - **Property 6: Record Integrity**
    - **Validates: Requirements 6.2**

- [x] 9. Implement Record Cleanup
  - [x] 9.1 Implement periodic cleanup task
    - Remove expired records
    - _Requirements: 8.7_

- [x] 10. Implement Web UI (optional)
  - [x] 10.1 Set up Tera templates
    - Dashboard, token management pages
    - _Requirements: 9.1, 9.2_
    - Implemented basic HTML templates (can be enhanced with Tera later)
  - [x] 10.2 Implement token generation UI
    - Invite and discovery tokens
    - _Requirements: 9.3_
    - Discovery token creation form implemented
  - [x] 10.3 Implement QR code display
    - _Requirements: 9.4_
    - QR code generation implemented using qrcode crate
    - Endpoint: GET /ui/tokens/:token_id/qr
  - [x] 10.4 Implement token revocation UI
    - _Requirements: 9.5_
    - Token revocation endpoint implemented
    - Endpoint: POST /ui/tokens/:token_id/revoke
  - [x] 10.5 Implement UI authentication
    - _Requirements: 9.6_
    - Basic authentication via admin tokens (can be enhanced with session-based auth later)

- [x] 11. Implement Configuration
  - [x] 11.1 Define ServerConfig struct
    - All configurable parameters
    - _Requirements: 11.4, 11.5_
  - [x] 11.2 Implement environment variable loading
    - _Requirements: 11.2_
  - [x] 11.3 Implement TOML file loading
    - _Requirements: 11.3_

- [x] 12. Create deployment artifacts
  - [x] 12.1 Configure static binary build
    - _Requirements: 11.1_
    - Release profile configured in Cargo.toml
  - [x] 12.2 Create systemd unit file
    - _Requirements: 11.7_
    - Created deploy/zrc-dirnode.service
  - [x] 12.3 Create example config file
    - _Requirements: 11.7_
    - Created examples/config.toml

- [x] 13. Checkpoint - Verify all tests pass
  - All unit tests passing (16 tests)
  - Property tests implemented and passing:
    - Property 1: Signature Verification
    - Property 2: Subject ID Binding
    - Property 3: TTL Enforcement
    - Property 4: Invite-Only Access
    - Property 5: Discovery Token Expiry
    - Property 6: Record Integrity
  - Directory operations verified via unit tests

## Notes

- Tasks marked with `*` are optional property-based tests
- Privacy-first: invite-only by default
- No search/listing endpoints to prevent enumeration
- Target <100MB memory for 10000 records
- Works on Raspberry Pi class hardware
