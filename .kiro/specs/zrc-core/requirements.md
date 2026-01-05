# Requirements Document: zrc-core

## Introduction

The zrc-core crate implements the business logic and state machines for the Zippy Remote Control (ZRC) system. This crate handles pairing workflows, session management, policy enforcement, message dispatch, and transport negotiation. It serves as the shared foundation used by both the agent (host) and controller applications, ensuring consistent behavior across all endpoints.

## Glossary

- **Pairing_Host**: State machine managing pairing from the device/host perspective
- **Pairing_Controller**: State machine managing pairing from the operator/controller perspective
- **Session_Host**: State machine managing active sessions on the device being controlled
- **Session_Controller**: State machine managing active sessions from the operator's perspective
- **Policy**: Rules governing consent, permissions, and access control
- **Dispatch**: Routing incoming messages to appropriate handlers based on message type
- **Transport_Negotiation**: Process of agreeing on connection parameters between endpoints
- **Store**: Abstraction for persisting pairings, tickets, and configuration
- **Consent**: User approval required before allowing remote access
- **Ticket**: Short-lived capability token authorizing session access
- **Invite**: Out-of-band data enabling initial pairing between devices

## Requirements

### Requirement 1: Pairing Host State Machine

**User Story:** As a device owner, I want the host to manage pairing requests securely, so that only authorized operators can establish trust with my device.

#### Acceptance Criteria

1. THE Pairing_Host SHALL transition through states: Idle → InviteGenerated → AwaitingRequest → AwaitingApproval → Paired
2. WHEN generate_invite() is called, THE Pairing_Host SHALL create an InviteV1 with random invite_secret and configurable expiry
3. WHEN a PairRequestV1 is received, THE Pairing_Host SHALL verify the invite_proof using the stored invite_secret
4. IF invite_proof verification fails, THEN THE Pairing_Host SHALL reject the request and return to Idle state
5. WHEN in AwaitingApproval state, THE Pairing_Host SHALL invoke the consent callback and wait for user decision
6. IF consent is granted, THEN THE Pairing_Host SHALL generate a PairReceiptV1 with granted permissions and device signature
7. WHEN pairing completes, THE Pairing_Host SHALL store the operator's pinned public keys and permissions
8. THE Pairing_Host SHALL enforce rate limiting of 3 failed pairing attempts per minute per source

### Requirement 2: Pairing Controller State Machine

**User Story:** As an operator, I want the controller to manage pairing workflows, so that I can establish trust with devices I need to access.

#### Acceptance Criteria

1. THE Pairing_Controller SHALL transition through states: Idle → InviteImported → RequestSent → AwaitingSAS → Paired
2. WHEN import_invite(invite_data) is called, THE Pairing_Controller SHALL parse and validate the InviteV1
3. WHEN send_pair_request() is called, THE Pairing_Controller SHALL generate a PairRequestV1 with valid invite_proof
4. WHEN a PairReceiptV1 is received, THE Pairing_Controller SHALL verify the device signature
5. IF signature verification fails, THEN THE Pairing_Controller SHALL reject the receipt and return to Idle state
6. WHEN in AwaitingSAS state, THE Pairing_Controller SHALL compute and display the SAS code for user verification
7. WHEN pairing completes, THE Pairing_Controller SHALL store the device's pinned public keys and granted permissions
8. THE Pairing_Controller SHALL timeout pairing attempts after 5 minutes of inactivity

### Requirement 3: Session Host State Machine

**User Story:** As a device owner, I want the host to manage session requests securely, so that only authorized operators can control my device.

#### Acceptance Criteria

1. THE Session_Host SHALL transition through states: Idle → RequestReceived → AwaitingConsent → Negotiating → Active → Ended
2. WHEN a SessionInitRequestV1 is received, THE Session_Host SHALL verify the operator is paired and has valid permissions
3. IF the operator is not paired, THEN THE Session_Host SHALL reject the request with AUTH_FAILED error
4. WHEN consent policy requires approval, THE Session_Host SHALL invoke the consent callback before proceeding
5. IF consent is denied, THEN THE Session_Host SHALL reject the request with PERMISSION_DENIED error
6. WHEN consent is granted, THE Session_Host SHALL issue a SessionTicketV1 with appropriate permissions and expiry
7. THE Session_Host SHALL include transport negotiation parameters (QUIC, relay tokens) in the response
8. WHEN in Active state, THE Session_Host SHALL process control messages and stream frames
9. THE Session_Host SHALL terminate sessions when ticket expires or disconnect is requested

### Requirement 4: Session Controller State Machine

**User Story:** As an operator, I want the controller to manage session workflows, so that I can establish and maintain remote control sessions.

#### Acceptance Criteria

1. THE Session_Controller SHALL transition through states: Idle → RequestSent → TicketReceived → Connecting → Active → Ended
2. WHEN start_session(device_id) is called, THE Session_Controller SHALL generate a SessionInitRequestV1 with requested capabilities
3. WHEN a SessionInitResponseV1 is received, THE Session_Controller SHALL verify the device signature and extract the ticket
4. IF signature verification fails, THEN THE Session_Controller SHALL reject the response and return to Idle state
5. WHEN a valid ticket is received, THE Session_Controller SHALL initiate transport connection using provided parameters
6. WHEN in Active state, THE Session_Controller SHALL send input events and receive frame data
7. THE Session_Controller SHALL monitor ticket expiry and request renewal before expiration
8. THE Session_Controller SHALL handle transport disconnection and attempt reconnection with valid ticket

### Requirement 5: Consent and Policy Engine

**User Story:** As a device owner, I want configurable consent policies, so that I can control when and how remote access is permitted.

#### Acceptance Criteria

1. THE Policy_Engine SHALL support consent modes: ALWAYS_REQUIRE, UNATTENDED_ALLOWED, and TRUSTED_OPERATORS_ONLY
2. WHEN consent mode is ALWAYS_REQUIRE, THE Policy_Engine SHALL require user approval for every session
3. WHEN consent mode is UNATTENDED_ALLOWED, THE Policy_Engine SHALL allow sessions from paired operators with UNATTENDED permission
4. WHEN consent mode is TRUSTED_OPERATORS_ONLY, THE Policy_Engine SHALL only allow sessions from explicitly trusted operator IDs
5. THE Policy_Engine SHALL support permission scoping: VIEW, CONTROL, CLIPBOARD, FILE_TRANSFER, AUDIO
6. THE Policy_Engine SHALL enforce that granted permissions never exceed paired permissions
7. THE Policy_Engine SHALL support time-based restrictions (allowed hours, days)
8. IF a policy violation occurs, THEN THE Policy_Engine SHALL log the violation and reject the request

### Requirement 6: Message Dispatch

**User Story:** As a developer, I want centralized message dispatch, so that incoming messages are routed to appropriate handlers consistently.

#### Acceptance Criteria

1. THE Dispatcher SHALL decode EnvelopeV1 messages and extract the inner payload
2. THE Dispatcher SHALL route messages based on msg_type to registered handlers
3. THE Dispatcher SHALL verify envelope signatures before dispatching to handlers
4. IF signature verification fails, THEN THE Dispatcher SHALL drop the message and log the failure
5. THE Dispatcher SHALL support handler registration for: PAIR_REQUEST, PAIR_RECEIPT, SESSION_INIT_REQUEST, SESSION_INIT_RESPONSE, CONTROL_MSG, ERROR
6. WHEN an unknown msg_type is received, THE Dispatcher SHALL log a warning and drop the message
7. THE Dispatcher SHALL track message statistics (received, dispatched, dropped) for observability

### Requirement 7: Transport Negotiation

**User Story:** As a developer, I want transport negotiation logic, so that endpoints can agree on the best connection method.

#### Acceptance Criteria

1. THE Transport_Negotiator SHALL evaluate transport options in priority order: MESH → DIRECT → RELAY
2. THE Transport_Negotiator SHALL generate QUIC parameters including self-signed certificate and ALPN protocols
3. THE Transport_Negotiator SHALL generate relay tokens when relay fallback is enabled
4. WHEN mesh transport is available, THE Transport_Negotiator SHALL prefer mesh for session initiation
5. WHEN direct connection fails, THE Transport_Negotiator SHALL automatically fall back to relay
6. THE Transport_Negotiator SHALL include ICE candidates when WebRTC transport is configured
7. THE Transport_Negotiator SHALL respect policy restrictions on allowed transports

### Requirement 8: Persistent Storage

**User Story:** As a developer, I want a storage abstraction, so that pairings and configuration can be persisted across restarts.

#### Acceptance Criteria

1. THE Store SHALL define a trait with methods: save_pairing, get_pairing, list_pairings, delete_pairing
2. THE Store SHALL define methods: save_invite, get_invite, delete_invite, cleanup_expired_invites
3. THE Store SHALL define methods: save_ticket, get_ticket, revoke_ticket, cleanup_expired_tickets
4. THE Store SHALL provide an in-memory implementation for testing and MVP
5. THE Store SHALL provide a SQLite implementation for production persistence
6. WHEN a pairing is saved, THE Store SHALL persist: device_id, operator_id, pinned_keys, permissions, paired_at
7. THE Store SHALL support atomic operations to prevent data corruption
8. FOR ALL store operations, saving then loading SHALL return equivalent data (round-trip property)

### Requirement 9: Audit Event Generation

**User Story:** As a device owner, I want audit events generated, so that I can review who accessed my device and when.

#### Acceptance Criteria

1. THE Audit_Module SHALL emit events for: PAIR_REQUEST_RECEIVED, PAIR_APPROVED, PAIR_DENIED, PAIR_REVOKED
2. THE Audit_Module SHALL emit events for: SESSION_REQUESTED, SESSION_STARTED, SESSION_ENDED, SESSION_DENIED
3. THE Audit_Module SHALL emit events for: PERMISSION_ESCALATION_ATTEMPTED, POLICY_VIOLATION
4. WHEN an audit event is emitted, THE Audit_Module SHALL include: timestamp, event_type, operator_id, device_id, details
5. THE Audit_Module SHALL support pluggable sinks: memory buffer, file, external service
6. THE Audit_Module SHALL sign audit events with the device key for non-repudiation
7. THE Audit_Module SHALL never include sensitive data (keys, secrets) in audit events

### Requirement 10: Rate Limiting

**User Story:** As a device owner, I want rate limiting, so that my device is protected from brute-force and denial-of-service attacks.

#### Acceptance Criteria

1. THE Rate_Limiter SHALL track request counts per source (operator_id or IP address)
2. THE Rate_Limiter SHALL enforce configurable limits: pairing_attempts_per_minute, session_requests_per_minute
3. WHEN a rate limit is exceeded, THE Rate_Limiter SHALL reject requests with RATE_LIMITED error
4. THE Rate_Limiter SHALL implement exponential backoff for repeated violations
5. THE Rate_Limiter SHALL support allowlisting for trusted operators
6. THE Rate_Limiter SHALL reset counters after the configured window expires
7. THE Rate_Limiter SHALL log rate limit violations for security monitoring

### Requirement 11: Error Handling

**User Story:** As a developer, I want consistent error handling, so that failures are communicated clearly and can be debugged.

#### Acceptance Criteria

1. THE Core_Module SHALL define error types: AuthError, PermissionError, TicketError, TransportError, StoreError, PolicyError
2. WHEN an error occurs, THE Core_Module SHALL return a structured error with code, message, and optional details
3. THE Core_Module SHALL map internal errors to appropriate ErrorV1 messages for wire transmission
4. THE Core_Module SHALL never expose internal implementation details in error messages sent to remote peers
5. THE Core_Module SHALL log detailed error information locally for debugging
6. IF an unrecoverable error occurs, THEN THE Core_Module SHALL transition state machines to a safe terminal state
