# Requirements Document: zrc-transport

## Introduction

The zrc-transport crate defines transport abstractions and common framing for the Zippy Remote Control (ZRC) system. This crate provides traits and utilities for different transport mechanisms (mesh, rendezvous, direct, relay) without containing OS-specific dependencies. It enables pluggable transport implementations while ensuring consistent behavior.

## Glossary

- **Transport**: Mechanism for delivering messages between endpoints
- **Control_Plane**: Channel for signaling and session management messages
- **Media_Plane**: Channel for screen frames and real-time data
- **Discovery**: Finding and connecting to remote endpoints
- **Framing**: Structuring data for transmission
- **Backpressure**: Flow control when receiver is slower than sender
- **Connection_Migration**: Maintaining session across network changes

## Requirements

### Requirement 1: Transport Trait Definition

**User Story:** As a developer, I want transport abstractions, so that different transports can be used interchangeably.

#### Acceptance Criteria

1. THE Transport_Module SHALL define ControlPlaneTransport trait for signaling
2. THE Transport_Module SHALL define MediaPlaneTransport trait for real-time data
3. THE Transport_Module SHALL define DiscoveryTransport trait for endpoint discovery
4. THE ControlPlaneTransport SHALL support send_envelope(recipient, envelope) method
5. THE ControlPlaneTransport SHALL support recv_envelope() -> (sender, envelope) method
6. THE MediaPlaneTransport SHALL support send_frame(data) method
7. THE MediaPlaneTransport SHALL support recv_frame() -> data method
8. THE Transport_Module SHALL define common error types for all transports

### Requirement 2: Message Framing

**User Story:** As a developer, I want consistent framing, so that messages are reliably delimited.

#### Acceptance Criteria

1. THE Framing_Module SHALL define length-prefixed framing format
2. THE Framing_Module SHALL use 4-byte big-endian length prefix
3. THE Framing_Module SHALL enforce maximum message size (default: 64KB for control, 1MB for media)
4. THE Framing_Module SHALL implement frame encoder: data -> framed bytes
5. THE Framing_Module SHALL implement frame decoder: framed bytes -> data
6. THE Framing_Module SHALL handle partial reads/writes
7. THE Framing_Module SHALL detect and report framing errors
8. FOR ALL valid data, encoding then decoding SHALL return original data

### Requirement 3: Transport Priority and Fallback

**User Story:** As a developer, I want transport fallback logic, so that connections succeed across network conditions.

#### Acceptance Criteria

1. THE Transport_Module SHALL define transport priority order: mesh → direct → rendezvous → relay
2. THE Transport_Module SHALL support configurable priority order
3. THE Transport_Module SHALL attempt transports in priority order
4. THE Transport_Module SHALL fall back to next transport on failure
5. THE Transport_Module SHALL support parallel transport attempts (optional)
6. THE Transport_Module SHALL report which transport succeeded
7. THE Transport_Module SHALL support transport restrictions (e.g., no relay)
8. THE Transport_Module SHALL timeout transport attempts (configurable)

### Requirement 4: Connection State Management

**User Story:** As a developer, I want connection state tracking, so that connection lifecycle is managed correctly.

#### Acceptance Criteria

1. THE Transport_Module SHALL define connection states: Disconnected, Connecting, Connected, Reconnecting, Failed
2. THE Transport_Module SHALL emit state change events
3. THE Transport_Module SHALL track connection duration
4. THE Transport_Module SHALL track bytes sent/received
5. THE Transport_Module SHALL track connection quality metrics
6. THE Transport_Module SHALL support connection close with reason
7. THE Transport_Module SHALL handle unexpected disconnection
8. THE Transport_Module SHALL support connection state queries

### Requirement 5: Reconnection Logic

**User Story:** As a developer, I want automatic reconnection, so that transient failures don't break sessions.

#### Acceptance Criteria

1. THE Transport_Module SHALL attempt reconnection on unexpected disconnect
2. THE Transport_Module SHALL use exponential backoff for reconnection attempts
3. THE Transport_Module SHALL limit maximum reconnection attempts (configurable)
4. THE Transport_Module SHALL preserve session state during reconnection
5. THE Transport_Module SHALL notify application of reconnection attempts
6. THE Transport_Module SHALL support reconnection cancellation
7. THE Transport_Module SHALL try alternative transports during reconnection
8. THE Transport_Module SHALL report final failure after exhausting attempts

### Requirement 6: Backpressure Handling

**User Story:** As a developer, I want backpressure handling, so that slow receivers don't cause memory exhaustion.

#### Acceptance Criteria

1. THE Transport_Module SHALL implement send buffer limits
2. THE Transport_Module SHALL block or drop when send buffer full (configurable)
3. THE Transport_Module SHALL prefer dropping frames over blocking for media
4. THE Transport_Module SHALL prefer blocking over dropping for control
5. THE Transport_Module SHALL report backpressure events
6. THE Transport_Module SHALL track dropped frame statistics
7. THE Transport_Module SHALL support configurable buffer sizes
8. THE Transport_Module SHALL implement receive buffer limits

### Requirement 7: Multiplexing Support

**User Story:** As a developer, I want stream multiplexing, so that different data types use separate channels.

#### Acceptance Criteria

1. THE Transport_Module SHALL define channel types: Control, Frames, Clipboard, Files, Audio
2. THE Transport_Module SHALL support opening multiple channels per connection
3. THE Transport_Module SHALL support channel prioritization
4. THE Transport_Module SHALL support channel-specific flow control
5. THE Transport_Module SHALL support channel close independent of connection
6. THE Transport_Module SHALL route data to correct channel
7. THE Transport_Module SHALL handle channel errors independently
8. THE Transport_Module SHALL support bidirectional and unidirectional channels

### Requirement 8: Encryption Integration

**User Story:** As a developer, I want encryption integration, so that transports can use session encryption.

#### Acceptance Criteria

1. THE Transport_Module SHALL support pluggable encryption layer
2. THE Transport_Module SHALL define encrypt/decrypt hooks
3. THE Transport_Module SHALL support per-channel encryption keys
4. THE Transport_Module SHALL handle encryption errors
5. THE Transport_Module SHALL support encryption bypass for already-encrypted data
6. THE Transport_Module SHALL track encryption overhead
7. THE Transport_Module SHALL support key rotation during session
8. THE Transport_Module SHALL zeroize encryption keys on close

### Requirement 9: Metrics and Observability

**User Story:** As a developer, I want transport metrics, so that I can monitor and debug connections.

#### Acceptance Criteria

1. THE Transport_Module SHALL track bytes sent/received per channel
2. THE Transport_Module SHALL track messages sent/received per channel
3. THE Transport_Module SHALL track round-trip time estimates
4. THE Transport_Module SHALL track packet loss estimates
5. THE Transport_Module SHALL track connection uptime
6. THE Transport_Module SHALL expose metrics via trait
7. THE Transport_Module SHALL support metrics export (Prometheus format)
8. THE Transport_Module SHALL track transport-specific metrics

### Requirement 10: Testing Utilities

**User Story:** As a developer, I want testing utilities, so that I can test transport-dependent code.

#### Acceptance Criteria

1. THE Transport_Module SHALL provide mock transport implementation
2. THE Transport_Module SHALL provide loopback transport for testing
3. THE Transport_Module SHALL support simulated latency
4. THE Transport_Module SHALL support simulated packet loss
5. THE Transport_Module SHALL support simulated disconnection
6. THE Transport_Module SHALL provide transport recording for replay
7. THE Transport_Module SHALL support deterministic testing
8. THE Transport_Module SHALL provide transport comparison utilities

### Requirement 11: QUIC Transport Helpers

**User Story:** As a developer, I want QUIC helpers, so that QUIC transport implementation is simplified.

#### Acceptance Criteria

1. THE Transport_Module SHALL provide QUIC stream mapping utilities
2. THE Transport_Module SHALL provide QUIC connection configuration helpers
3. THE Transport_Module SHALL provide certificate generation utilities
4. THE Transport_Module SHALL provide certificate pinning verification
5. THE Transport_Module SHALL provide ALPN protocol constants
6. THE Transport_Module SHALL provide connection migration helpers
7. THE Transport_Module SHALL provide QUIC error mapping
8. THE Transport_Module SHALL provide congestion control configuration

### Requirement 12: HTTP Mailbox Helpers

**User Story:** As a developer, I want HTTP mailbox helpers, so that rendezvous transport is simplified.

#### Acceptance Criteria

1. THE Transport_Module SHALL provide HTTP client configuration
2. THE Transport_Module SHALL provide long-poll implementation helpers
3. THE Transport_Module SHALL provide retry logic with backoff
4. THE Transport_Module SHALL provide request signing utilities
5. THE Transport_Module SHALL provide response parsing utilities
6. THE Transport_Module SHALL provide connection pooling configuration
7. THE Transport_Module SHALL provide timeout configuration
8. THE Transport_Module SHALL provide proxy support utilities
