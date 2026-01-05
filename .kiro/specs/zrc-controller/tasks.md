# Implementation Plan: zrc-controller

## Overview

Implementation tasks for the command-line interface (CLI) controller. This power-user tool enables pairing with devices, initiating sessions, and debugging transport and cryptography.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with dependencies
    - zrc-core, zrc-crypto, zrc-proto, zrc-transport
    - clap, tokio, serde, comfy-table
    - _Requirements: 1.1, 9.1_
  - [x] 1.2 Create module structure
    - cli, identity, pairing, session, input, pairings, output, debug
    - _Requirements: 1.1, 2.1, 3.1, 5.1, 7.1, 9.1, 12.1_

- [x] 2. Implement CLI Layer
  - [x] 2.1 Define CLI structure with clap
    - Commands: pair, session, input, pairings, identity, frames, debug
    - _Requirements: 1.1, 2.1, 3.1, 5.1, 7.1, 11.3, 12.1_
  - [x] 2.2 Implement global flags
    - --output, --verbose, --debug, --config
    - _Requirements: 9.1, 9.7, 9.8_

- [x] 3. Implement Identity Manager
  - [x] 3.1 Implement key generation on first run
    - Ed25519 signing, X25519 key exchange
    - _Requirements: 11.1_
  - [x] 3.2 Implement secure key storage
    - OS keystore where available
    - _Requirements: 11.2_
  - [x] 3.3 Implement identity show command
    - Display operator_id, fingerprint, created_at
    - _Requirements: 11.3, 11.4_
  - [x] 3.4 Implement identity export and rotate
    - _Requirements: 11.5, 11.6, 11.7_
  - [ ]* 3.5 Write property test for identity persistence
    - **Property 8: Identity Persistence**
    - **Validates: Requirements 11.1, 11.2**

- [x] 4. Implement Invite Import
  - [x] 4.1 Implement base64 parsing
    - _Requirements: 1.2_
  - [x] 4.2 Implement file parsing
    - JSON and binary formats
    - _Requirements: 1.3_
  - [x] 4.3 Implement QR code parsing
    - From image file
    - _Requirements: 1.4_
  - [x] 4.4 Implement validation and display
    - Show device_id, expires_at, transport_hints
    - _Requirements: 1.5, 1.6_
  - [ ]* 4.5 Write property test for invite validation
    - **Property 1: Invite Validation**
    - **Validates: Requirements 1.5, 1.6**

- [x] 5. Checkpoint - Verify identity and invite handling
  - Ensure identity and invite commands work
  - Ask the user if questions arise

- [x] 6. Implement Pairing Client
  - [x] 6.1 Implement pair command
    - Generate PairRequestV1 with invite proof
    - _Requirements: 2.1, 2.2_
  - [x] 6.2 Implement transport sending
    - Send via configured transport
    - _Requirements: 2.3_
  - [x] 6.3 Implement receipt handling
    - Wait for PairReceiptV1, verify signature
    - _Requirements: 2.4_
  - [x] 6.4 Implement SAS verification
    - Display SAS, prompt for confirmation
    - _Requirements: 2.5_
  - [x] 6.5 Implement pairing storage
    - Store device keys and permissions
    - _Requirements: 2.6_
  - [x] 6.6 Write property test for pairing proof correctness

    - **Property 2: Pairing Proof Correctness**
    - **Validates: Requirement 2.2**

- [-] 7. Implement Session Client
  - [x] 7.1 Implement session start command
    - Verify pairing, generate SessionInitRequestV1
    - _Requirements: 3.1, 3.2, 3.3_
  - [ ] 7.2 Implement response handling
    - Wait for SessionInitResponseV1
    - _Requirements: 3.5, 3.6_
  - [ ] 7.3 Implement session connect command
    - QUIC connection with cert verification
    - _Requirements: 4.1, 4.2, 4.3_
  - [ ] 7.4 Implement ticket authentication
    - Send ticket, establish streams
    - _Requirements: 4.4, 4.5, 4.6_
  - [ ]* 7.5 Write property test for session ticket verification
    - **Property 3: Session Ticket Verification**
    - **Validates: Requirements 4.3, 4.4**

- [x] 8. Implement Input Commands
  - [x] 8.1 Implement mouse command
    - Move, click, scroll
    - _Requirements: 5.1, 5.4_
  - [x] 8.2 Implement key command
    - Key down, key up
    - _Requirements: 5.2_
  - [x] 8.3 Implement text command
    - Text string input
    - _Requirements: 5.3_
  - [x] 8.4 Implement validation and sending
    - _Requirements: 5.5, 5.6, 5.7_

- [x] 9. Implement Frame Reception
  - [x] 9.1 Implement frame receiving
    - Receive over QUIC stream
    - _Requirements: 6.1, 6.2_
  - [x] 9.2 Implement frames save command
    - Save to file
    - _Requirements: 6.3_
  - [x] 9.3 Implement frames stats command
    - Display frame rate, resolution, bandwidth
    - _Requirements: 6.4, 6.5_

- [x] 10. Implement Pairings Store
  - [x] 10.1 Implement SQLite storage
    - Store device keys, permissions, timestamps
    - _Requirements: 7.1, 7.2_
  - [x] 10.2 Implement list and show commands
    - _Requirements: 7.1, 7.3_
  - [x] 10.3 Implement revoke command
    - With confirmation
    - _Requirements: 7.4, 7.7_
  - [x] 10.4 Implement export and import
    - _Requirements: 7.5, 7.6_

- [x] 11. Implement Transport Configuration
  - [x] 11.1 Implement transport flag
    - mesh, rendezvous, direct, relay, auto
    - _Requirements: 8.1, 8.2_
  - [x] 11.2 Implement transport ladder
    - Try in order when auto
    - _Requirements: 8.3_
  - [x] 11.3 Implement URL configuration
    - --rendezvous-url, --relay-url, --mesh-node
    - _Requirements: 8.4, 8.5, 8.6_
  - [ ]* 11.4 Write property test for transport ladder order
    - **Property 4: Transport Ladder Order**
    - **Validates: Requirement 8.3**

- [x] 12. Implement Output Formatter
  - [x] 12.1 Implement JSON output
    - Consistent schema
    - _Requirements: 9.1, 9.4_
  - [x] 12.2 Implement table output
    - Human-readable tables
    - _Requirements: 9.2_
  - [x] 12.3 Implement quiet output
    - Exit codes only
    - _Requirements: 9.3_
  - [x] 12.4 Implement exit codes
    - 0=success, 1=error, 2=auth_failed, 3=timeout
    - _Requirements: 9.6_
  - [x] 12.5 Write property test for output format consistency

    - **Property 5: Output Format Consistency**
    - **Validates: Requirements 9.1, 9.4**
  - [x] 12.6 Write property test for exit code accuracy

    - **Property 6: Exit Code Accuracy**
    - **Validates: Requirement 9.6**

- [x] 13. Implement Configuration
  - [x] 13.1 Define config file structure
    - TOML format
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
  - [x] 13.2 Implement config loading
    - Platform-specific paths
    - _Requirements: 10.1_
  - [x] 13.3 Implement CLI override
    - Command-line overrides config
    - _Requirements: 10.5_
  - [x] 13.4 Implement default config creation
    - _Requirements: 10.7_
  - [x] 13.5 Write property test for configuration override precedence

    - **Property 7: Configuration Override Precedence**
    - **Validates: Requirement 10.5**

- [x] 14. Implement Debug Tools
  - [x] 14.1 Implement envelope decode
    - _Requirements: 12.1_
  - [x] 14.2 Implement transcript compute
    - _Requirements: 12.2_
  - [x] 14.3 Implement SAS compute
    - _Requirements: 12.3_
  - [x] 14.4 Implement transport test
    - _Requirements: 12.4_
  - [x] 14.5 Implement packet capture
    - _Requirements: 12.6_

- [x] 15. Checkpoint - Verify all tests pass
  - Ensure all property tests pass with 100+ iterations
  - Verify CLI functionality end-to-end
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- CLI serves as reference implementation for controller protocol
- JSON output enables scripting and automation
- Exit codes are critical for CI/CD integration
