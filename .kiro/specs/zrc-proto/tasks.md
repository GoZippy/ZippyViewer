# Implementation Plan: zrc-proto

## Overview

Implementation tasks for the Protocol Buffers wire format definitions. This is the foundational crate that all other components depend on.

**Build Order:** #1 (No dependencies - START HERE)

## Tasks

- [x] 1. Set up crate structure and build system
  - [x] 1.1 Create Cargo.toml with prost and prost-build dependencies
    - Configure prost-build for code generation
    - Add serde feature flags for JSON serialization
    - _Requirements: 1.1, 1.2_
  - [x] 1.2 Create build.rs for protobuf compilation
    - Configure prost-build to generate from proto/ directory
    - Set up type attributes for serde derives
    - _Requirements: 1.3_
  - [x] 1.3 Create proto/ directory structure
    - _Requirements: 1.1_

- [x] 2. Define identity and key messages
  - [x] 2.1 Implement DeviceIdV1 and OperatorIdV1 messages
    - 32-byte identifier fields
    - _Requirements: 2.1, 2.2_
  - [x] 2.2 Implement PublicKeyBundleV1 message
    - Ed25519 signing public key (32 bytes)
    - X25519 key exchange public key (32 bytes)
    - _Requirements: 2.3, 2.4_
  - [ ]* 2.3 Write property test for identity round-trip encoding
    - **Property 1: Round-trip Encoding Consistency**
    - **Validates: Requirements 2.7, 10.6**

- [x] 3. Define pairing messages
  - [x] 3.1 Implement InviteV1 message
    - Device ID, signing public key, invite secret hash
    - Expiration timestamp, transport hints
    - _Requirements: 3.1, 3.2, 3.3_
  - [x] 3.2 Implement PairRequestV1 message
    - Operator identity, invite proof, requested permissions
    - Nonce and timestamp for replay protection
    - _Requirements: 3.4, 3.5_
  - [x] 3.3 Implement PairReceiptV1 message
    - Granted permissions, session binding, device signature
    - _Requirements: 3.6, 3.7_
  - [x] 3.4 Write property test for pairing message round-trip

    - **Property 1: Round-trip Encoding Consistency**
    - **Validates: Requirements 2.7, 10.6**

- [x] 4. Define session messages
  - [x] 4.1 Implement SessionInitRequestV1 message
    - Session ID, requested capabilities, transport preference
    - Operator signature
    - _Requirements: 4.1, 4.2_
  - [x] 4.2 Implement SessionInitResponseV1 message
    - Granted capabilities, transport params, issued ticket
    - Include CertBindingV1 for identity-bound DTLS
    - _Requirements: 4.3, 4.4_
  - [x] 4.3 Implement SessionTicketV1 message
    - Ticket ID, permissions, expiration, session binding
    - Device signature for verification
    - _Requirements: 4.5, 4.6_
  - [ ]* 4.4 Write property test for session ticket validity
    - **Property 5: Timestamp Validity**
    - **Validates: Requirements 3.5, 6.5**

- [x] 5. Define envelope and encryption messages
  - [x] 5.1 Implement EnvelopeV1 message
    - Header, ephemeral KEX public key, encrypted payload
    - Signature and AAD fields
    - _Requirements: 5.1, 5.2_
  - [x] 5.2 Implement EnvelopeHeaderV1 message
    - Version, message type, sender/recipient IDs
    - Timestamp and nonce
    - _Requirements: 5.3, 5.4_
  - [ ]* 5.3 Write property test for signature coverage
    - **Property 4: Signature Coverage Completeness**
    - **Validates: Requirements 4.4, 6.6, 7.1**

- [x] 6. Define WebRTC signaling messages (NEW for WebRTC-first)
  - [x] 6.1 Implement CertBindingV1 message
    - DTLS fingerprint (32 bytes SHA-256)
    - Fingerprint signature (Ed25519 signature of fingerprint)
    - Signer public key (for verification)
    - _Requirements: Security - Identity-bound DTLS_
  - [x] 6.2 Implement SignalingMessageV1 with oneof payload
    - Offer: SDP string + CertBindingV1
    - Answer: SDP string + CertBindingV1
    - IceCandidate: candidate string, sdp_mid, sdp_mline_index
    - SessionEnd: reason string
    - _Requirements: WebRTC signaling_
  - [x] 6.3 Implement IceConfigV1 message
    - STUN server URLs
    - TURN server configs (url, username, credential)
    - ICE transport policy
    - _Requirements: NAT traversal config_

- [x] 7. Define control messages
  - [x] 7.1 Implement ControlMsgV1 with oneof payload
    - Message type, sequence number, timestamp
    - Input, clipboard, frame, file, session control variants
    - _Requirements: 6.1, 6.2_
  - [x] 7.2 Implement InputEventV1 message
    - Mouse coordinates, buttons, key codes, modifiers
    - Text input and scroll delta
    - _Requirements: 6.3, 6.4_
  - [x] 7.3 Implement ClipboardMsgV1 and FrameMetadataV1
    - _Requirements: 6.5, 6.6_

- [x] 8. Define directory and transport messages
  - [x] 8.1 Implement DirRecordV1 message
    - Subject ID, endpoints, TTL, signature
    - _Requirements: 7.1, 7.2_
  - [x] 8.2 Implement DiscoveryTokenV1 message
    - Token ID, scope, expiration, signature
    - _Requirements: 7.3, 7.4_
  - [x] 8.3 Implement EndpointHintsV1 and QuicParamsV1
    - Direct addresses, relay tokens, rendezvous URLs
    - _Requirements: 7.5, 7.6_

- [x] 9. Define enumerations
  - [x] 9.1 Implement MsgTypeV1 enumeration
    - All message type variants with UNSPECIFIED = 0
    - Add SIGNALING_OFFER, SIGNALING_ANSWER, SIGNALING_ICE for WebRTC
    - _Requirements: 8.1_
  - [x] 9.2 Implement PermissionsV1 as bitflags
    - VIEW, CONTROL, CLIPBOARD, FILE_TRANSFER, AUDIO, UNATTENDED
    - _Requirements: 8.2_
  - [x] 9.3 Implement remaining enumerations
    - InputEventTypeV1, ErrorCodeV1, TransportPreferenceV1
    - _Requirements: 8.3, 8.4_

- [x] 10. Define error messages
  - [x] 10.1 Implement ErrorV1 message
    - Error code, message, details map, timestamp
    - _Requirements: 9.1, 9.2_

- [x] 11. Add helper methods and validation
  - [x] 11.1 Add validation methods for message fields
    - Size validation for byte fields
    - Timestamp validation
    - _Requirements: 10.1, 10.2_
  - [x] 11.2 Add conversion traits (From/Into)
    - Between proto types and domain types
    - _Requirements: 10.3_
  - [ ]* 11.3 Write property test for field tag uniqueness
    - **Property 2: Field Tag Uniqueness**
    - **Validates: Requirements 10.2, 10.3**
  - [ ]* 11.4 Write property test for version suffix consistency
    - **Property 3: Version Suffix Consistency**
    - **Validates: Requirements 10.1, 10.4**

- [x] 12. Checkpoint - Verify all tests pass
  - Run: `cargo test -p zrc-proto`
  - Run: `cargo clippy -p zrc-proto`
  - Ensure all property tests pass with 100+ iterations
  - Verify round-trip encoding for all message types
  - **On completion:** Update `docs/specs/EXECUTION_QUEUE.md` status to ✅ COMPLETE
  - **Next component:** zrc-crypto

## Notes

- Tasks marked with `*` are optional property-based tests
- This crate has no runtime dependencies beyond prost
- All message names must end with V1 suffix for versioning
- Field tags 1-15 reserved for frequently used fields
- CertBindingV1 is CRITICAL for identity-bound DTLS (security blocker)
