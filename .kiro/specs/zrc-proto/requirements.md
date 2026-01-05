# Requirements Document: zrc-proto

## Introduction

The zrc-proto crate defines all wire formats and protocol messages for the Zippy Remote Control (ZRC) system using Protocol Buffers (protobuf). This crate serves as the single source of truth for all inter-component communication, ensuring type safety, versioning compatibility, and deterministic serialization across all platforms and languages.

## Glossary

- **Protobuf**: Protocol Buffers, a language-neutral serialization format
- **Wire_Format**: The binary representation of messages transmitted between components
- **Envelope**: A signed and sealed container for encrypted payloads
- **Transcript**: A canonical byte sequence used for cryptographic commitments
- **Session_Ticket**: A short-lived capability token authorizing session access
- **Pairing**: The process of establishing mutual trust between operator and device
- **Device**: A machine running the ZRC agent (host being controlled)
- **Operator**: A user/machine initiating remote control sessions
- **DirRecord**: A signed directory entry containing device presence information
- **Discovery_Token**: A time-bounded token enabling temporary device discoverability

## Requirements

### Requirement 1: Identity and Key Messages

**User Story:** As a developer, I want well-defined identity and key message formats, so that all components can consistently represent and exchange cryptographic identities.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define DeviceIdV1 as a 32-byte identifier derived from the device's public signing key
2. THE Proto_Schema SHALL define OperatorIdV1 as a 32-byte identifier derived from the operator's public signing key
3. THE Proto_Schema SHALL define PublicKeyBundleV1 containing sign_pub (Ed25519) and kex_pub (X25519) fields
4. THE Proto_Schema SHALL define KeyTypeV1 enumeration supporting ED25519, X25519, and future PQC key types
5. WHEN a new key type is added, THE Proto_Schema SHALL use a new enum value without reusing existing tags

### Requirement 2: Pairing Protocol Messages

**User Story:** As a developer, I want pairing protocol messages defined, so that devices and operators can establish mutual trust securely.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define InviteV1 containing device_id, device_sign_pub, invite_secret_hash, expires_at, and transport_hints
2. THE Proto_Schema SHALL define PairRequestV1 containing operator_id, operator_sign_pub, operator_kex_pub, invite_proof, requested_permissions, and nonce
3. THE Proto_Schema SHALL define PairReceiptV1 containing device_id, operator_id, permissions_granted, paired_at, device_signature, and session_binding
4. THE Proto_Schema SHALL define PermissionsV1 as a bitmask enumeration including VIEW, CONTROL, CLIPBOARD, FILE_TRANSFER, AUDIO, and UNATTENDED
5. WHEN parsing a PairRequestV1, THE Parser SHALL reject messages with missing required fields and return a descriptive error
6. THE Pretty_Printer SHALL format PairRequestV1 and PairReceiptV1 back into valid protobuf binary
7. FOR ALL valid pairing messages, parsing then encoding SHALL produce byte-equivalent output (round-trip property)

### Requirement 3: Session Initialization Messages

**User Story:** As a developer, I want session initialization messages defined, so that paired devices can establish secure remote sessions.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define SessionInitRequestV1 containing operator_id, device_id, session_id, requested_capabilities, transport_preferences, and operator_signature
2. THE Proto_Schema SHALL define SessionInitResponseV1 containing session_id, granted_capabilities, transport_params, issued_ticket, and device_signature
3. THE Proto_Schema SHALL define SessionTicketV1 containing ticket_id, session_id, operator_id, device_id, permissions, expires_at, session_binding, and device_signature
4. THE Proto_Schema SHALL define TransportPreferenceV1 enumeration including MESH_PREFERRED, DIRECT_PREFERRED, RELAY_ALLOWED, and RELAY_ONLY
5. WHEN a SessionTicketV1 is parsed, THE Parser SHALL validate that expires_at is a valid Unix timestamp

### Requirement 4: Envelope and Encryption Messages

**User Story:** As a developer, I want envelope messages defined, so that all sensitive payloads can be signed and encrypted consistently.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define EnvelopeV1 containing header, sender_kex_pub, encrypted_payload, signature, and aad (additional authenticated data)
2. THE Proto_Schema SHALL define EnvelopeHeaderV1 containing version, msg_type, sender_id, recipient_id, timestamp, and nonce
3. THE Proto_Schema SHALL define MsgTypeV1 enumeration for all message types including PAIR_REQUEST, PAIR_RECEIPT, SESSION_INIT_REQUEST, SESSION_INIT_RESPONSE, CONTROL_MSG, and ERROR
4. THE Proto_Schema SHALL ensure EnvelopeHeaderV1 fields are ordered deterministically for transcript hashing
5. WHEN an EnvelopeV1 is created, THE Envelope SHALL include a 24-byte random nonce

### Requirement 5: Control Plane Messages

**User Story:** As a developer, I want control plane messages defined, so that real-time session control can be transmitted efficiently.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define ControlMsgV1 containing msg_type, sequence_number, timestamp, and a oneof payload field
2. THE Proto_Schema SHALL define InputEventV1 containing event_type, mouse_x, mouse_y, button, key_code, modifiers, and text
3. THE Proto_Schema SHALL define InputEventTypeV1 enumeration including MOUSE_MOVE, MOUSE_DOWN, MOUSE_UP, KEY_DOWN, KEY_UP, KEY_CHAR, and SCROLL
4. THE Proto_Schema SHALL define ClipboardMsgV1 containing direction, format, and data fields
5. THE Proto_Schema SHALL define FrameMetadataV1 containing frame_id, timestamp, monitor_id, width, height, format, and flags
6. WHEN a ControlMsgV1 is received, THE Parser SHALL validate sequence_number is monotonically increasing within a session

### Requirement 6: Directory and Discovery Messages

**User Story:** As a developer, I want directory messages defined, so that devices can publish presence and be discovered securely.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define DirRecordV1 containing subject_id, device_sign_pub, endpoints, ttl_seconds, timestamp, and signature
2. THE Proto_Schema SHALL define EndpointHintsV1 containing direct_addrs, relay_tokens, rendezvous_urls, and mesh_hints
3. THE Proto_Schema SHALL define DiscoveryTokenV1 containing token_id, subject_id, expires_at, scope, and signature
4. THE Proto_Schema SHALL define DiscoveryScopeV1 enumeration including PAIRING_ONLY and SESSION_ONLY
5. WHEN a DirRecordV1 is parsed, THE Parser SHALL reject records where ttl_seconds exceeds 86400 (24 hours)
6. THE Proto_Schema SHALL ensure DirRecordV1 signature covers a canonical transcript of all fields except the signature itself

### Requirement 7: Transport Negotiation Messages

**User Story:** As a developer, I want transport negotiation messages defined, so that endpoints can agree on connection parameters.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define QuicParamsV1 containing server_addr, server_port, server_cert_der, and alpn_protocols
2. THE Proto_Schema SHALL define RelayTokenV1 containing relay_id, allocation_id, expires_at, bandwidth_limit, and signature
3. THE Proto_Schema SHALL define TransportNegotiationV1 containing preferred_transport, quic_params, relay_tokens, and ice_candidates
4. THE Proto_Schema SHALL define IceCandidateV1 containing candidate_type, protocol, address, port, priority, and foundation
5. WHEN transport negotiation occurs, THE Proto_Schema SHALL support multiple fallback options in priority order

### Requirement 8: Error and Audit Messages

**User Story:** As a developer, I want error and audit messages defined, so that failures can be communicated clearly and sessions can be audited.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define ErrorV1 containing error_code, error_message, details, and timestamp
2. THE Proto_Schema SHALL define ErrorCodeV1 enumeration including AUTH_FAILED, PERMISSION_DENIED, TICKET_EXPIRED, RATE_LIMITED, TRANSPORT_FAILED, and INTERNAL_ERROR
3. THE Proto_Schema SHALL define AuditEventV1 containing event_type, session_id, operator_id, device_id, timestamp, permissions_used, and signature
4. THE Proto_Schema SHALL define AuditEventTypeV1 enumeration including SESSION_START, SESSION_END, PERMISSION_CHANGE, and PAIRING_REVOKED
5. WHEN an error occurs, THE ErrorV1 message SHALL include sufficient detail for debugging without exposing sensitive data

### Requirement 9: File Transfer Messages

**User Story:** As a developer, I want file transfer messages defined, so that files can be transferred securely during sessions.

#### Acceptance Criteria

1. THE Proto_Schema SHALL define FileManifestV1 containing transfer_id, filename, size, hash, chunk_size, and total_chunks
2. THE Proto_Schema SHALL define FileChunkV1 containing transfer_id, chunk_index, data, and chunk_hash
3. THE Proto_Schema SHALL define FileTransferControlV1 containing transfer_id, action, and progress
4. THE Proto_Schema SHALL define FileActionV1 enumeration including START, PAUSE, RESUME, CANCEL, and COMPLETE
5. WHEN a file transfer is initiated, THE FileManifestV1 SHALL include a SHA-256 hash of the complete file

### Requirement 10: Versioning and Compatibility

**User Story:** As a developer, I want clear versioning rules, so that protocol evolution doesn't break existing deployments.

#### Acceptance Criteria

1. THE Proto_Schema SHALL use V1 suffix on all message names for the initial version
2. THE Proto_Schema SHALL never reuse protobuf field tags once assigned
3. THE Proto_Schema SHALL only add new fields as optional to maintain backward compatibility
4. WHEN a breaking change is required, THE Proto_Schema SHALL define a new V2 message alongside the V1 message
5. THE Proto_Schema SHALL include a version field in EnvelopeHeaderV1 to enable version negotiation
6. FOR ALL message types, THE Proto_Schema SHALL maintain golden test vectors for cross-platform verification
