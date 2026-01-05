# Requirements Document: zrc-controller

## Introduction

The zrc-controller crate implements a command-line interface (CLI) for the Zippy Remote Control (ZRC) system. This power-user tool enables pairing with devices, initiating sessions, and debugging transport and cryptography. The controller serves as both a standalone tool and a reference implementation for the controller-side protocol flows.

## Glossary

- **Controller**: The CLI tool used by operators to connect to remote devices
- **Operator**: The user initiating remote control sessions
- **Invite**: Out-of-band data enabling initial pairing with a device
- **Pairing**: Establishing mutual trust between operator and device
- **Session**: An active remote control connection
- **SAS**: Short Authentication String for MITM verification
- **Transport_Ladder**: Ordered list of transport methods to attempt

## Requirements

### Requirement 1: Invite Import

**User Story:** As an operator, I want to import device invites, so that I can initiate pairing with new devices.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller pair --invite <base64_or_file>
2. THE Controller SHALL parse InviteV1 from base64-encoded string
3. THE Controller SHALL parse InviteV1 from file path (JSON or binary)
4. THE Controller SHALL parse InviteV1 from QR code image file
5. WHEN invite is invalid or expired, THE Controller SHALL display clear error message
6. THE Controller SHALL display invite details: device_id, expires_at, transport_hints
7. THE Controller SHALL store imported invite for subsequent pairing
8. THE Controller SHALL support --dry-run to validate invite without storing

### Requirement 2: Pairing Flow

**User Story:** As an operator, I want to pair with devices, so that I can establish trust for future sessions.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller pair --device <device_id>
2. THE Controller SHALL generate PairRequestV1 with operator identity and invite proof
3. THE Controller SHALL send pair request via configured transport (mesh/rendezvous)
4. THE Controller SHALL wait for PairReceiptV1 response with configurable timeout
5. WHEN pairing requires SAS verification, THE Controller SHALL display SAS code and prompt for confirmation
6. WHEN pairing succeeds, THE Controller SHALL store device's pinned keys and granted permissions
7. WHEN pairing fails, THE Controller SHALL display error reason and suggestions
8. THE Controller SHALL support --permissions to request specific permissions

### Requirement 3: Session Initiation

**User Story:** As an operator, I want to start remote sessions, so that I can access paired devices.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller session start --device <device_id>
2. THE Controller SHALL verify device is paired before initiating session
3. THE Controller SHALL generate SessionInitRequestV1 with requested capabilities
4. THE Controller SHALL send session request via configured transport
5. THE Controller SHALL wait for SessionInitResponseV1 with ticket and transport params
6. WHEN session is approved, THE Controller SHALL display session details and proceed to connect
7. WHEN session is denied, THE Controller SHALL display denial reason
8. THE Controller SHALL support --capabilities to request specific capabilities

### Requirement 4: QUIC Connection

**User Story:** As an operator, I want to connect via QUIC, so that I can establish the data channel for remote control.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller session connect --quic <host:port> --cert <fingerprint> --ticket <base64>
2. THE Controller SHALL establish QUIC connection to specified endpoint
3. THE Controller SHALL verify server certificate matches provided fingerprint
4. THE Controller SHALL send ticket for session authentication
5. THE Controller SHALL establish control stream for bidirectional messaging
6. THE Controller SHALL establish frame stream for receiving screen data
7. WHEN connection fails, THE Controller SHALL display error and retry suggestions
8. THE Controller SHALL support --relay to connect via relay server

### Requirement 5: Input Commands

**User Story:** As an operator, I want to send input commands, so that I can control the remote device from CLI.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller input mouse --x <x> --y <y> [--click left|right|middle]
2. THE Controller SHALL support command: zrc-controller input key --code <vk_code> [--down|--up]
3. THE Controller SHALL support command: zrc-controller input text --string <text>
4. THE Controller SHALL support command: zrc-controller input scroll --delta <delta>
5. THE Controller SHALL send InputEventV1 messages over control stream
6. THE Controller SHALL validate input parameters before sending
7. THE Controller SHALL display confirmation of sent input
8. THE Controller SHALL support --session to specify active session

### Requirement 6: Frame Reception

**User Story:** As an operator, I want to receive screen frames, so that I can view the remote display.

#### Acceptance Criteria

1. THE Controller SHALL receive frame data over dedicated QUIC stream
2. THE Controller SHALL decode frame metadata (dimensions, format, timestamp)
3. THE Controller SHALL support saving frames to file: zrc-controller frames save --output <path>
4. THE Controller SHALL support frame statistics: zrc-controller frames stats
5. THE Controller SHALL display frame rate, resolution, and bandwidth usage
6. THE Controller SHALL handle frame drops gracefully
7. THE Controller SHALL support --format to specify output format (raw, png)

### Requirement 7: Pairing Management

**User Story:** As an operator, I want to manage my pairings, so that I can view and revoke device trust.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller pairings list
2. THE Controller SHALL display: device_id, paired_at, permissions, last_session
3. THE Controller SHALL support command: zrc-controller pairings show <device_id>
4. THE Controller SHALL support command: zrc-controller pairings revoke <device_id>
5. THE Controller SHALL support command: zrc-controller pairings export --output <file>
6. THE Controller SHALL support command: zrc-controller pairings import --input <file>
7. THE Controller SHALL confirm before revoking pairings
8. THE Controller SHALL support --force to skip confirmation

### Requirement 8: Transport Configuration

**User Story:** As an operator, I want to configure transports, so that I can control how connections are established.

#### Acceptance Criteria

1. THE Controller SHALL support --transport flag to specify transport preference
2. THE Controller SHALL support transport values: mesh, rendezvous, direct, relay, auto
3. WHEN transport is auto, THE Controller SHALL try transports in ladder order
4. THE Controller SHALL support --rendezvous-url to specify rendezvous server
5. THE Controller SHALL support --relay-url to specify relay server
6. THE Controller SHALL support --mesh-node to specify mesh entry point
7. THE Controller SHALL display transport negotiation progress in verbose mode
8. THE Controller SHALL timeout transport attempts after configurable duration

### Requirement 9: Output Formats

**User Story:** As an operator, I want structured output, so that I can integrate the controller with scripts and automation.

#### Acceptance Criteria

1. THE Controller SHALL support --output json for JSON output
2. THE Controller SHALL support --output table for human-readable tables (default)
3. THE Controller SHALL support --output quiet for minimal output (exit codes only)
4. THE Controller SHALL use consistent JSON schema across all commands
5. THE Controller SHALL include timestamps in JSON output
6. THE Controller SHALL return appropriate exit codes: 0=success, 1=error, 2=auth_failed, 3=timeout
7. THE Controller SHALL support --verbose for detailed progress output
8. THE Controller SHALL support --debug for protocol-level debugging

### Requirement 10: Configuration

**User Story:** As an operator, I want persistent configuration, so that I don't need to specify common options repeatedly.

#### Acceptance Criteria

1. THE Controller SHALL read configuration from ~/.config/zrc/controller.toml (Unix) or %APPDATA%\zrc\controller.toml (Windows)
2. THE Controller SHALL support configuring: default_transport, rendezvous_urls, relay_urls
3. THE Controller SHALL support configuring: timeout_seconds, output_format, log_level
4. THE Controller SHALL support configuring: identity_key_path, pairings_db_path
5. THE Controller SHALL allow command-line arguments to override config file
6. THE Controller SHALL support --config to specify alternate config file
7. THE Controller SHALL create default config on first run
8. THE Controller SHALL validate config and warn about invalid values

### Requirement 11: Identity Management

**User Story:** As an operator, I want to manage my identity, so that devices can verify who I am.

#### Acceptance Criteria

1. THE Controller SHALL generate operator keypair on first run
2. THE Controller SHALL store private key securely (OS keystore where available)
3. THE Controller SHALL support command: zrc-controller identity show
4. THE Controller SHALL display: operator_id, public_key_fingerprint, created_at
5. THE Controller SHALL support command: zrc-controller identity export --output <file>
6. THE Controller SHALL support command: zrc-controller identity rotate
7. THE Controller SHALL warn before rotating identity (breaks existing pairings)
8. THE Controller SHALL support --identity-file to use alternate identity

### Requirement 12: Debugging and Diagnostics

**User Story:** As a developer, I want debugging tools, so that I can troubleshoot protocol and transport issues.

#### Acceptance Criteria

1. THE Controller SHALL support command: zrc-controller debug envelope --decode <base64>
2. THE Controller SHALL support command: zrc-controller debug transcript --compute <inputs>
3. THE Controller SHALL support command: zrc-controller debug sas --compute <transcript>
4. THE Controller SHALL support command: zrc-controller debug transport --test <url>
5. THE Controller SHALL display detailed error information in debug mode
6. THE Controller SHALL support packet capture to file for analysis
7. THE Controller SHALL support --trace for wire-level protocol tracing
8. THE Controller SHALL include version and build info in debug output
