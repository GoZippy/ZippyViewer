# Requirements Document: zrc-agent

## Introduction

The zrc-agent crate implements the host daemon/service for the Zippy Remote Control (ZRC) system. This agent runs on machines being remotely controlled, handling pairing requests, session management, screen capture, input injection, and connectivity. The agent enforces local security policies and provides consent mechanisms for attended access.

## Glossary

- **Agent**: The daemon/service running on the host machine being controlled
- **Host**: The machine running the agent (being remotely accessed)
- **Operator**: The user/machine initiating remote control
- **Consent**: User approval required before allowing remote access
- **Capture**: Screen/audio capture from the host machine
- **Input_Injection**: Sending mouse/keyboard events to the host
- **Unattended_Access**: Remote access without requiring user presence
- **Session**: An active remote control connection
- **Transport_Adapter**: Abstraction for different connectivity methods

## Requirements

### Requirement 1: Service Lifecycle

**User Story:** As a device owner, I want the agent to run reliably as a system service, so that remote access is available when needed.

#### Acceptance Criteria

1. THE Agent SHALL run as a Windows Service on Windows platforms
2. THE Agent SHALL run as a systemd service on Linux platforms
3. THE Agent SHALL run as a launchd daemon on macOS platforms
4. WHEN the service starts, THE Agent SHALL initialize configuration, keys, and transport adapters
5. WHEN the service stops, THE Agent SHALL gracefully terminate active sessions and release resources
6. THE Agent SHALL automatically restart after crashes (configurable restart policy)
7. THE Agent SHALL log startup, shutdown, and error events to system log
8. THE Agent SHALL support running in foreground mode for debugging

### Requirement 2: Identity and Key Management

**User Story:** As a device owner, I want secure identity management, so that my device has a unique cryptographic identity.

#### Acceptance Criteria

1. THE Agent SHALL generate Ed25519 signing keypair on first run if none exists
2. THE Agent SHALL generate X25519 key exchange keypair on first run if none exists
3. THE Agent SHALL store private keys in the OS keystore where available (DPAPI/Keychain/Secret Service)
4. THE Agent SHALL derive device_id from the signing public key
5. THE Agent SHALL support key rotation with configurable retention of old keys
6. THE Agent SHALL protect key material from unauthorized access
7. IF key storage fails, THEN THE Agent SHALL log error and refuse to start

### Requirement 3: Pairing Management

**User Story:** As a device owner, I want to control which operators can access my device, so that only trusted people can connect.

#### Acceptance Criteria

1. THE Agent SHALL generate invites containing device identity and transport hints
2. THE Agent SHALL process incoming PairRequestV1 messages and verify invite proofs
3. THE Agent SHALL display consent prompt for attended pairing requests
4. WHEN pairing is approved, THE Agent SHALL store operator's pinned keys and granted permissions
5. THE Agent SHALL support revoking existing pairings
6. THE Agent SHALL enforce maximum concurrent pairings limit (default: 100)
7. THE Agent SHALL rate limit pairing attempts (default: 3 per minute per source)
8. THE Agent SHALL log all pairing events for audit

### Requirement 4: Session Management

**User Story:** As a device owner, I want controlled session access, so that remote sessions follow my security policies.

#### Acceptance Criteria

1. THE Agent SHALL process incoming SessionInitRequestV1 messages
2. THE Agent SHALL verify the operator is paired and has valid permissions
3. WHEN consent policy requires approval, THE Agent SHALL display consent prompt
4. WHEN consent is granted, THE Agent SHALL issue a SessionTicketV1 with appropriate permissions
5. THE Agent SHALL enforce session timeout (default: 8 hours maximum)
6. THE Agent SHALL support multiple concurrent sessions (configurable, default: 1)
7. THE Agent SHALL terminate sessions when ticket expires or operator disconnects
8. THE Agent SHALL log session start, end, and permission usage for audit

### Requirement 5: Consent and Policy

**User Story:** As a device owner, I want configurable consent policies, so that I control when remote access requires my approval.

#### Acceptance Criteria

1. THE Agent SHALL support consent modes: ALWAYS_REQUIRE, UNATTENDED_ALLOWED, TRUSTED_ONLY
2. WHEN consent mode is ALWAYS_REQUIRE, THE Agent SHALL display prompt for every session
3. WHEN consent mode is UNATTENDED_ALLOWED, THE Agent SHALL allow sessions from operators with UNATTENDED permission
4. THE Agent SHALL display visible indicator during active sessions (system tray icon, overlay)
5. THE Agent SHALL provide "panic button" to immediately terminate all sessions
6. THE Agent SHALL support time-based access restrictions (allowed hours)
7. THE Agent SHALL support permission scoping per operator (VIEW, CONTROL, CLIPBOARD, FILES)
8. IF no user is logged in, THEN THE Agent SHALL only allow unattended access if explicitly enabled

### Requirement 6: Screen Capture

**User Story:** As an operator, I want to see the remote screen, so that I can provide support or access the machine.

#### Acceptance Criteria

1. THE Agent SHALL capture the primary display by default
2. THE Agent SHALL support multi-monitor capture with monitor selection
3. THE Agent SHALL capture at configurable frame rate (default: 30 fps, max: 60 fps)
4. THE Agent SHALL support capture resolution scaling for bandwidth optimization
5. THE Agent SHALL detect and handle display configuration changes (resolution, monitor add/remove)
6. THE Agent SHALL use platform-appropriate capture API (DXGI/WGC on Windows, ScreenCaptureKit on macOS, PipeWire on Linux)
7. THE Agent SHALL fall back to slower capture methods if preferred API unavailable
8. THE Agent SHALL respect DRM/protected content restrictions where applicable

### Requirement 7: Input Injection

**User Story:** As an operator, I want to control the remote machine, so that I can perform tasks on behalf of the user.

#### Acceptance Criteria

1. THE Agent SHALL inject mouse move, click, and scroll events
2. THE Agent SHALL inject keyboard key down, key up, and character events
3. THE Agent SHALL support modifier keys (Shift, Ctrl, Alt, Meta/Win)
4. THE Agent SHALL map input coordinates from viewer space to display space
5. THE Agent SHALL clamp coordinates to valid display bounds
6. THE Agent SHALL release all held keys on session end (prevent stuck keys)
7. THE Agent SHALL support special key sequences (Ctrl+Alt+Del on Windows)
8. WHEN input permission is not granted, THE Agent SHALL ignore input events

### Requirement 8: Clipboard Synchronization

**User Story:** As an operator, I want clipboard sync, so that I can copy/paste between local and remote machines.

#### Acceptance Criteria

1. THE Agent SHALL detect clipboard changes on the host
2. THE Agent SHALL send clipboard content to the controller when changed
3. THE Agent SHALL receive clipboard content from the controller and set local clipboard
4. THE Agent SHALL support text clipboard format
5. THE Agent SHALL support image clipboard format (PNG)
6. THE Agent SHALL enforce clipboard size limit (default: 10MB)
7. WHEN clipboard permission is not granted, THE Agent SHALL not sync clipboard
8. THE Agent SHALL log clipboard sync events for audit (without content)

### Requirement 9: File Transfer

**User Story:** As an operator, I want to transfer files, so that I can upload/download files to/from the remote machine.

#### Acceptance Criteria

1. THE Agent SHALL support file download requests from controller
2. THE Agent SHALL support file upload from controller
3. THE Agent SHALL enforce file size limits (configurable, default: 1GB)
4. THE Agent SHALL support transfer resume for interrupted transfers
5. THE Agent SHALL verify file integrity via hash comparison
6. THE Agent SHALL respect file system permissions
7. WHEN file transfer permission is not granted, THE Agent SHALL reject transfer requests
8. THE Agent SHALL log file transfer events for audit (filename, size, direction)

### Requirement 10: Transport Connectivity

**User Story:** As a device owner, I want reliable connectivity, so that operators can reach my device through various network conditions.

#### Acceptance Criteria

1. THE Agent SHALL support mesh transport adapter for ZippyCoin mesh connectivity
2. THE Agent SHALL support rendezvous transport adapter for HTTP mailbox connectivity
3. THE Agent SHALL support direct QUIC listener for LAN/direct connections
4. THE Agent SHALL support relay transport for NAT traversal fallback
5. THE Agent SHALL maintain persistent connection to configured rendezvous servers
6. THE Agent SHALL publish presence to configured directory nodes
7. THE Agent SHALL handle transport failures and automatic reconnection
8. THE Agent SHALL prefer transports in order: mesh → direct → rendezvous → relay

### Requirement 11: Media Transport (WebRTC-first)

**User Story:** As an operator, I want efficient data transfer, so that remote control is responsive and reliable.

#### Acceptance Criteria

1. THE Agent SHALL use WebRTC for video/audio streaming and data channels
2. THE Agent SHALL generate DTLS certificate and sign fingerprint with device identity key
3. THE Agent SHALL include signed certificate fingerprint in session negotiation for identity binding
4. THE Agent SHALL support WebRTC DataChannels for: control, clipboard, files
5. THE Agent SHALL implement backpressure when send buffers are congested
6. THE Agent SHALL drop frames rather than block when behind
7. THE Agent SHALL support ICE for NAT traversal with TURN fallback
8. THE Agent SHALL enforce session-level encryption (DTLS + identity binding)
9. THE Agent SHALL alert operator if DTLS cert changes for same device identity
10. THE Agent SHALL support coturn as self-hostable TURN relay

### Requirement 11a: Replay Protection

**User Story:** As a device owner, I want protection against replay attacks, so that captured packets cannot be reused maliciously.

#### Acceptance Criteria

1. THE Agent SHALL use deterministic nonces: stream_id (32-bit) || counter (64-bit)
2. THE Agent SHALL include per-stream counters in AEAD AAD
3. THE Agent SHALL implement sliding window replay filter per stream
4. THE Agent SHALL reject duplicate packets reliably
5. IF a replayed packet is detected, THEN THE Agent SHALL log the event and drop the packet

### Requirement 12: Configuration

**User Story:** As a device owner, I want flexible configuration, so that I can customize agent behavior for my needs.

#### Acceptance Criteria

1. THE Agent SHALL accept configuration via config file (TOML)
2. THE Agent SHALL accept configuration via command-line arguments
3. THE Agent SHALL support configuring: consent_mode, allowed_operators, max_sessions
4. THE Agent SHALL support configuring: capture_fps, capture_quality, capture_monitors
5. THE Agent SHALL support configuring: rendezvous_urls, directory_urls, relay_urls
6. THE Agent SHALL support configuring: log_level, log_path, audit_log_path
7. THE Agent SHALL validate configuration on startup and log warnings for invalid values
8. THE Agent SHALL support configuration reload via signal (SIGHUP on Unix)

### Requirement 13: Logging and Audit

**User Story:** As a device owner, I want comprehensive logging, so that I can troubleshoot issues and audit access.

#### Acceptance Criteria

1. THE Agent SHALL log to system log (Event Log on Windows, syslog on Unix)
2. THE Agent SHALL support file-based logging with rotation
3. THE Agent SHALL log: startup, shutdown, pairing events, session events, errors
4. THE Agent SHALL maintain separate audit log for security events
5. THE Agent SHALL sign audit log entries with device key for non-repudiation
6. THE Agent SHALL support log level configuration (error, warn, info, debug, trace)
7. THE Agent SHALL never log sensitive data (keys, passwords, clipboard content)
8. THE Agent SHALL support log export for support/debugging
