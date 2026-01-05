# Implementation Plan: zrc-core

## Overview

Implementation tasks for the core business logic crate. This crate implements state machines for pairing and sessions, policy enforcement, message dispatch, and transport negotiation.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with dependencies
    - zrc-proto, zrc-crypto, tokio, thiserror, async-trait
    - _Requirements: 1.1, 3.1_
  - [x] 1.2 Create module structure
    - pairing, session, policy, dispatch, transport, store, audit, rate_limit
    - _Requirements: 1.1, 3.1, 5.1, 6.1_

- [x] 2. Implement Store trait and in-memory implementation
  - [x] 2.1 Define Store trait with async methods
    - save_pairing, get_pairing, list_pairings, delete_pairing
    - save_invite, get_invite, delete_invite
    - save_ticket, get_ticket, revoke_ticket
    - _Requirements: 8.1, 8.2, 8.3_
  - [x] 2.2 Implement InMemoryStore for testing
    - Thread-safe with RwLock
    - _Requirements: 8.4_
  - [ ]* 2.3 Write property test for store round-trip
    - **Property 6: Store Round-Trip**
    - **Validates: Requirements 8.8**

- [x] 3. Implement Pairing Host state machine
  - [x] 3.1 Define PairingHostState enum
    - Idle, InviteGenerated, AwaitingRequest, AwaitingApproval, Paired, Failed
    - _Requirements: 1.1_
  - [x] 3.2 Implement generate_invite
    - Create InviteV1 with random secret and expiry
    - Store invite and secret
    - _Requirements: 1.2_
  - [x] 3.3 Implement handle_request
    - Verify invite_proof using stored secret
    - Transition to AwaitingApproval or Failed
    - _Requirements: 1.3, 1.4_
  - [x] 3.4 Implement approve and reject
    - Generate PairReceiptV1 with permissions and signature
    - Store pairing on approval
    - _Requirements: 1.5, 1.6, 1.7_
  - [ ]* 3.5 Write property test for pairing proof verification
    - **Property 1: Pairing Proof Verification**
    - **Validates: Requirements 1.3, 1.4**

- [x] 4. Implement Pairing Controller state machine
  - [x] 4.1 Define PairingControllerState enum
    - Idle, InviteImported, RequestSent, AwaitingSAS, Paired, Failed
    - _Requirements: 2.1_
  - [x] 4.2 Implement import_invite
    - Parse and validate InviteV1
    - _Requirements: 2.2_
  - [x] 4.3 Implement send_request
    - Generate PairRequestV1 with invite_proof
    - _Requirements: 2.3_
  - [x] 4.4 Implement handle_receipt
    - Verify device signature
    - Compute and display SAS
    - _Requirements: 2.4, 2.5, 2.6_
  - [x] 4.5 Implement confirm_sas and timeout handling
    - Store pairing on confirmation
    - 5-minute timeout
    - _Requirements: 2.7, 2.8_

- [x] 5. Checkpoint - Verify pairing state machines
  - Ensure pairing flow tests pass
  - Ask the user if questions arise

- [x] 6. Implement Session Host state machine
  - [x] 6.1 Define SessionHostState enum
    - Idle, RequestReceived, AwaitingConsent, Negotiating, Active, Ended
    - _Requirements: 3.1_
  - [x] 6.2 Implement handle_request
    - Verify operator is paired with valid permissions
    - Check consent policy
    - _Requirements: 3.2, 3.3, 3.4_
  - [x] 6.3 Implement approve
    - Issue SessionTicketV1 with permissions and expiry
    - Include transport negotiation params
    - _Requirements: 3.5, 3.6, 3.7_
  - [x] 6.4 Implement reject and end_session
    - Handle consent denial and session termination
    - _Requirements: 3.5, 3.9_
  - [ ]* 6.5 Write property test for permission enforcement
    - **Property 2: Permission Enforcement**
    - **Validates: Requirements 5.6**

- [x] 7. Implement Session Controller state machine
  - [x] 7.1 Define SessionControllerState enum
    - Idle, RequestSent, TicketReceived, Connecting, Active, Ended
    - _Requirements: 4.1_
  - [x] 7.2 Implement start_session
    - Generate SessionInitRequestV1 with capabilities
    - _Requirements: 4.2_
  - [x] 7.3 Implement handle_response
    - Verify device signature, extract ticket
    - Initiate transport connection
    - _Requirements: 4.3, 4.4, 4.5_
  - [x] 7.4 Implement ticket renewal and reconnection
    - Monitor expiry, handle disconnection
    - _Requirements: 4.7, 4.8_

- [x] 8. Implement Policy Engine
  - [x] 8.1 Define ConsentMode enum and PolicyEngine struct
    - ALWAYS_REQUIRE, UNATTENDED_ALLOWED, TRUSTED_OPERATORS_ONLY
    - _Requirements: 5.1_
  - [x] 8.2 Implement requires_consent
    - Check consent mode and operator trust
    - _Requirements: 5.2, 5.3, 5.4_
  - [x] 8.3 Implement validate_permissions
    - Enforce permission scoping and limits
    - _Requirements: 5.5, 5.6_
  - [x] 8.4 Implement time-based restrictions
    - Allowed hours and days
    - _Requirements: 5.7, 5.8_
  - [ ]* 8.5 Write property test for consent policy enforcement
    - **Property 3: Consent Policy Enforcement**
    - **Validates: Requirements 5.2**

- [x] 9. Implement Message Dispatcher
  - [x] 9.1 Define Dispatcher struct and MessageHandler trait
    - Handler registration by msg_type
    - _Requirements: 6.1, 6.2, 6.5_
  - [x] 9.2 Implement dispatch
    - Verify signature, decrypt, route to handler
    - _Requirements: 6.3, 6.4_
  - [x] 9.3 Implement statistics tracking
    - Received, dispatched, dropped counts
    - _Requirements: 6.6, 6.7_

- [x] 10. Implement Transport Negotiator
  - [x] 10.1 Define TransportNegotiator and preferences
    - Priority order: MESH → DIRECT → RELAY
    - _Requirements: 7.1_
  - [x] 10.2 Implement generate_params
    - QUIC parameters, relay tokens
    - _Requirements: 7.2, 7.3_
  - [x] 10.3 Implement select_transport
    - Evaluate options, handle fallback
    - _Requirements: 7.4, 7.5, 7.7_

- [x] 11. Implement Rate Limiter
  - [x] 11.1 Define RateLimiter struct
    - Track counts per source
    - _Requirements: 10.1_
  - [x] 11.2 Implement check_rate_limit
    - Configurable limits, exponential backoff
    - _Requirements: 10.2, 10.3, 10.4_
  - [x] 11.3 Implement allowlist and reset
    - Trusted operators, window expiry
    - _Requirements: 10.5, 10.6, 10.7_
  - [ ]* 11.4 Write property test for rate limit enforcement
    - **Property 5: Rate Limit Enforcement**
    - **Validates: Requirements 10.3**

- [x] 12. Implement Audit Event generation
  - [x] 12.1 Define AuditEvent enum and AuditSink trait
    - All event types with required fields
    - _Requirements: 9.1, 9.2, 9.3, 9.4_
  - [x] 12.2 Implement pluggable sinks
    - Memory buffer, file sink
    - _Requirements: 9.5_
  - [x] 12.3 Implement event signing
    - Sign with device key, exclude sensitive data
    - _Requirements: 9.6, 9.7_
  - [ ]* 12.4 Write property test for audit event completeness
    - **Property 7: Audit Event Completeness**
    - **Validates: Requirements 9.1, 9.2, 9.3**

- [x] 13. Implement Error types
  - [x] 13.1 Define error types with thiserror
    - AuthError, PermissionError, TicketError, etc.
    - _Requirements: 11.1, 11.2_
  - [x] 13.2 Implement error mapping to ErrorV1
    - Wire-safe error messages
    - _Requirements: 11.3, 11.4_

- [x] 14. Implement SQLite Store (production)
  - [x] 14.1 Implement SqliteStore
    - Atomic operations, migrations
    - _Requirements: 8.5, 8.6, 8.7_
  - [ ]* 14.2 Write property test for SQLite store round-trip
    - **Property 6: Store Round-Trip**
    - **Validates: Requirements 8.8**

- [x] 15. Checkpoint - Verify all tests pass
  - Ensure all property tests pass with 100+ iterations
  - Verify state machine transitions
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- State machines are the core of this crate
- Store trait enables testing with in-memory implementation
- Policy engine is critical for security
