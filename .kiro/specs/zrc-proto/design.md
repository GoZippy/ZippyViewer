# Design Document: zrc-proto

## Overview

The zrc-proto crate defines all wire formats for the ZRC system using Protocol Buffers v3. This crate generates Rust types via prost and serves as the canonical definition for all inter-component communication. The design prioritizes backward compatibility, security, and cross-platform consistency.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      zrc-proto                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │  proto/         │    │  src/           │                │
│  │  └─zrc_v1.proto │───▶│  └─lib.rs       │                │
│  │                 │    │    (generated)  │                │
│  └─────────────────┘    └─────────────────┘                │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐                                        │
│  │  build.rs       │                                        │
│  │  (prost-build)  │                                        │
│  └─────────────────┘                                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### Message Groups

```protobuf
// Group 1: Identity & Keys
message DeviceIdV1 { bytes id = 1; }  // 32 bytes
message OperatorIdV1 { bytes id = 1; }  // 32 bytes
message PublicKeyBundleV1 {
  bytes sign_pub = 1;  // Ed25519, 32 bytes
  bytes kex_pub = 2;   // X25519, 32 bytes
}

// Group 2: Pairing
message InviteV1 {
  bytes device_id = 1;
  bytes device_sign_pub = 2;
  bytes invite_secret_hash = 3;  // SHA256(invite_secret)
  uint64 expires_at = 4;  // Unix timestamp
  EndpointHintsV1 transport_hints = 5;
}

message PairRequestV1 {
  bytes operator_id = 1;
  bytes operator_sign_pub = 2;
  bytes operator_kex_pub = 3;
  bytes invite_proof = 4;  // HMAC(secret, transcript)
  uint32 requested_permissions = 5;
  bytes nonce = 6;  // 32 bytes
  uint64 timestamp = 7;
}

message PairReceiptV1 {
  bytes device_id = 1;
  bytes operator_id = 2;
  uint32 permissions_granted = 3;
  uint64 paired_at = 4;
  bytes session_binding = 5;  // For future sessions
  bytes device_signature = 6;
}

// Group 3: Session
message SessionInitRequestV1 {
  bytes operator_id = 1;
  bytes device_id = 2;
  bytes session_id = 3;  // 32 bytes random
  uint32 requested_capabilities = 4;
  TransportPreferenceV1 transport_preference = 5;
  bytes operator_signature = 6;
}

message SessionInitResponseV1 {
  bytes session_id = 1;
  uint32 granted_capabilities = 2;
  TransportNegotiationV1 transport_params = 3;
  SessionTicketV1 issued_ticket = 4;
  bytes device_signature = 5;
}

message SessionTicketV1 {
  bytes ticket_id = 1;  // 16 bytes
  bytes session_id = 2;
  bytes operator_id = 3;
  bytes device_id = 4;
  uint32 permissions = 5;
  uint64 expires_at = 6;
  bytes session_binding = 7;
  bytes device_signature = 8;
}

// Group 4: Envelope
message EnvelopeV1 {
  EnvelopeHeaderV1 header = 1;
  bytes sender_kex_pub = 2;  // Ephemeral X25519
  bytes encrypted_payload = 3;
  bytes signature = 4;  // Ed25519 over header+kex+aad+ciphertext
  bytes aad = 5;  // Additional authenticated data
}

message EnvelopeHeaderV1 {
  uint32 version = 1;
  MsgTypeV1 msg_type = 2;
  bytes sender_id = 3;
  bytes recipient_id = 4;
  uint64 timestamp = 5;
  bytes nonce = 6;  // 24 bytes for ChaCha20Poly1305
}

// Group 5: Control Messages
message ControlMsgV1 {
  ControlMsgTypeV1 msg_type = 1;
  uint64 sequence_number = 2;
  uint64 timestamp = 3;
  oneof payload {
    InputEventV1 input = 10;
    ClipboardMsgV1 clipboard = 11;
    FrameMetadataV1 frame_meta = 12;
    FileTransferControlV1 file_control = 13;
    SessionControlV1 session_control = 14;
  }
}

message InputEventV1 {
  InputEventTypeV1 event_type = 1;
  int32 mouse_x = 2;
  int32 mouse_y = 3;
  uint32 button = 4;
  uint32 key_code = 5;
  uint32 modifiers = 6;
  string text = 7;  // For character input
  int32 scroll_delta = 8;
}

// Group 6: Directory
message DirRecordV1 {
  bytes subject_id = 1;
  bytes device_sign_pub = 2;
  EndpointHintsV1 endpoints = 3;
  uint32 ttl_seconds = 4;
  uint64 timestamp = 5;
  bytes signature = 6;
}

message DiscoveryTokenV1 {
  bytes token_id = 1;  // 16 bytes random
  bytes subject_id = 2;
  uint64 expires_at = 3;
  DiscoveryScopeV1 scope = 4;
  bytes signature = 5;
}

// Group 7: Transport
message EndpointHintsV1 {
  repeated string direct_addrs = 1;
  repeated RelayTokenV1 relay_tokens = 2;
  repeated string rendezvous_urls = 3;
  repeated string mesh_hints = 4;
}

message QuicParamsV1 {
  string server_addr = 1;
  uint32 server_port = 2;
  bytes server_cert_der = 3;
  repeated string alpn_protocols = 4;
}

// Group 8: Errors
message ErrorV1 {
  ErrorCodeV1 error_code = 1;
  string error_message = 2;
  map<string, string> details = 3;
  uint64 timestamp = 4;
}
```

### Enumerations

```protobuf
enum MsgTypeV1 {
  MSG_TYPE_UNSPECIFIED = 0;
  PAIR_REQUEST = 1;
  PAIR_RECEIPT = 2;
  SESSION_INIT_REQUEST = 3;
  SESSION_INIT_RESPONSE = 4;
  CONTROL_MSG = 5;
  ERROR = 6;
  DIR_RECORD = 7;
  DISCOVERY_TOKEN = 8;
}

enum PermissionsV1 {
  PERM_NONE = 0;
  PERM_VIEW = 1;
  PERM_CONTROL = 2;
  PERM_CLIPBOARD = 4;
  PERM_FILE_TRANSFER = 8;
  PERM_AUDIO = 16;
  PERM_UNATTENDED = 32;
}

enum InputEventTypeV1 {
  INPUT_UNSPECIFIED = 0;
  MOUSE_MOVE = 1;
  MOUSE_DOWN = 2;
  MOUSE_UP = 3;
  KEY_DOWN = 4;
  KEY_UP = 5;
  KEY_CHAR = 6;
  SCROLL = 7;
}

enum ErrorCodeV1 {
  ERROR_UNSPECIFIED = 0;
  AUTH_FAILED = 1;
  PERMISSION_DENIED = 2;
  TICKET_EXPIRED = 3;
  RATE_LIMITED = 4;
  TRANSPORT_FAILED = 5;
  INTERNAL_ERROR = 6;
  INVALID_MESSAGE = 7;
  SESSION_NOT_FOUND = 8;
}

enum TransportPreferenceV1 {
  TRANSPORT_AUTO = 0;
  MESH_PREFERRED = 1;
  DIRECT_PREFERRED = 2;
  RELAY_ALLOWED = 3;
  RELAY_ONLY = 4;
}
```

## Data Models

### Field Tag Allocation Strategy

| Range | Purpose |
|-------|---------|
| 1-15 | Frequently used fields (1-byte encoding) |
| 16-99 | Standard fields |
| 100-199 | Optional/extension fields |
| 200-299 | Reserved for future versions |
| 1000+ | Custom/experimental |

### Versioning Rules

1. **Message Names**: Always suffixed with version (e.g., `FooV1`)
2. **Field Tags**: Never reuse, never renumber
3. **New Fields**: Always optional, use default values
4. **Breaking Changes**: Create new message version (e.g., `FooV2`)
5. **Deprecation**: Mark with `[deprecated = true]`, keep for 2 major versions

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Round-trip Encoding Consistency
*For any* valid protobuf message, encoding to bytes then decoding back SHALL produce a semantically equivalent message.
**Validates: Requirements 2.7, 10.6**

### Property 2: Field Tag Uniqueness
*For any* message definition, all field tags within that message SHALL be unique and never reused across versions.
**Validates: Requirements 10.2, 10.3**

### Property 3: Version Suffix Consistency
*For any* message type name, it SHALL end with a version suffix matching the pattern `V[0-9]+`.
**Validates: Requirements 10.1, 10.4**

### Property 4: Signature Coverage Completeness
*For any* signed message (Envelope, Ticket, DirRecord), the signature SHALL cover all fields except the signature field itself.
**Validates: Requirements 4.4, 6.6, 7.1**

### Property 5: Timestamp Validity
*For any* message containing expires_at, the value SHALL be a valid Unix timestamp greater than the message creation time.
**Validates: Requirements 3.5, 6.5**

## Error Handling

| Error Condition | Response |
|-----------------|----------|
| Unknown field tag | Ignore (forward compatibility) |
| Missing required field | Return parsing error with field name |
| Invalid enum value | Use UNSPECIFIED (0) value |
| Oversized message | Reject before parsing |
| Invalid UTF-8 in string | Return parsing error |

## Testing Strategy

### Unit Tests
- Parse/encode each message type with valid data
- Verify field presence and types
- Test enum value handling
- Test default value behavior

### Property-Based Tests
- Round-trip encoding for all message types (100+ iterations)
- Field tag uniqueness verification
- Version suffix pattern matching
- Golden vector compatibility tests

### Integration Tests
- Cross-platform encoding compatibility (Rust, TypeScript, Python)
- Backward compatibility with previous versions
- Forward compatibility with unknown fields
