# Requirements Document: zrc-security

## Introduction

The zrc-security module defines the security architecture, threat model, and security controls for the Zippy Remote Control (ZRC) system. This document establishes security requirements that all components must satisfy and provides guidance for security-conscious implementation.

## Glossary

- **Threat_Model**: Analysis of potential attacks and mitigations
- **E2EE**: End-to-End Encryption, ensuring only endpoints can read data
- **Identity_Pinning**: Binding trust to specific cryptographic keys
- **Transport_Pinning**: Binding trust to specific TLS certificates
- **MITM**: Man-in-the-Middle attack, intercepting communications
- **Replay_Attack**: Reusing captured messages to impersonate
- **Downgrade_Attack**: Forcing use of weaker security mechanisms
- **SAS**: Short Authentication String for MITM detection

## Requirements

### Requirement 1: Threat Model Documentation

**User Story:** As a security reviewer, I want a documented threat model, so that I can understand the security posture.

#### Acceptance Criteria

1. THE Security_Doc SHALL document all trust boundaries in the system
2. THE Security_Doc SHALL enumerate threat actors: malicious directory, malicious relay, network attacker, compromised endpoint
3. THE Security_Doc SHALL document data flow diagrams showing encryption boundaries
4. THE Security_Doc SHALL list all cryptographic operations and their purposes
5. THE Security_Doc SHALL document assumptions about trusted components
6. THE Security_Doc SHALL rate threats by likelihood and impact
7. THE Security_Doc SHALL map mitigations to each identified threat
8. THE Security_Doc SHALL be updated with each major release

### Requirement 2: MITM Protection

**User Story:** As a user, I want MITM protection, so that attackers cannot intercept my sessions.

#### Acceptance Criteria

1. THE System SHALL use identity pinning after initial pairing
2. THE System SHALL verify device/operator public keys on every connection
3. THE System SHALL provide SAS verification for discoverable pairing
4. THE System SHALL reject connections with mismatched identity keys
5. THE System SHALL log and alert on identity mismatch attempts
6. THE System SHALL support out-of-band fingerprint verification
7. THE System SHALL never trust directory-provided keys without verification
8. THE System SHALL use transport pinning for QUIC connections

### Requirement 3: Replay Attack Prevention

**User Story:** As a user, I want replay protection, so that captured messages cannot be reused.

#### Acceptance Criteria

1. THE System SHALL include nonces in all encrypted messages
2. THE System SHALL include timestamps in session tickets
3. THE System SHALL reject tickets with expired timestamps
4. THE System SHALL implement monotonic sequence numbers per channel
5. THE System SHALL reject out-of-window sequence numbers
6. THE System SHALL bind tickets to specific session tuples
7. THE System SHALL use unique session_binding per session
8. THE System SHALL log replay attempt detections

### Requirement 4: Downgrade Attack Prevention

**User Story:** As a user, I want downgrade protection, so that attackers cannot force weak encryption.

#### Acceptance Criteria

1. THE System SHALL enforce minimum cipher suite requirements
2. THE System SHALL reject connections with unsupported algorithms
3. THE System SHALL not negotiate weaker algorithms than configured minimum
4. THE System SHALL include algorithm identifiers in signed handshakes
5. THE System SHALL verify algorithm consistency across handshake
6. THE System SHALL log downgrade attempt detections
7. THE System SHALL support algorithm deprecation with version updates
8. THE System SHALL document supported algorithm versions

### Requirement 5: Key Compromise Recovery

**User Story:** As a user, I want key compromise recovery, so that I can recover from security incidents.

#### Acceptance Criteria

1. THE System SHALL support device key rotation
2. THE System SHALL support operator key rotation
3. THE System SHALL propagate key rotation to paired endpoints
4. THE System SHALL revoke all sessions on key rotation
5. THE System SHALL support emergency key revocation
6. THE System SHALL maintain key rotation history for audit
7. THE System SHALL guide users through key rotation process
8. THE System SHALL support re-pairing after key rotation

### Requirement 6: Secure Key Storage

**User Story:** As a user, I want secure key storage, so that my keys are protected at rest.

#### Acceptance Criteria

1. THE System SHALL use OS keystore where available (DPAPI, Keychain, Secret Service)
2. THE System SHALL encrypt keys at rest when OS keystore unavailable
3. THE System SHALL zeroize key material after use
4. THE System SHALL protect keys with appropriate access controls
5. THE System SHALL support hardware security modules (optional)
6. THE System SHALL audit key access operations
7. THE System SHALL handle keystore unavailability gracefully
8. THE System SHALL support key backup with user-provided password

### Requirement 7: Session Security

**User Story:** As a user, I want secure sessions, so that my remote access is protected.

#### Acceptance Criteria

1. THE System SHALL encrypt all session data with session-specific keys
2. THE System SHALL derive separate keys for each direction
3. THE System SHALL derive separate keys for each channel type
4. THE System SHALL enforce session timeout (configurable, max 8 hours)
5. THE System SHALL require re-authentication for session extension
6. THE System SHALL terminate sessions on security events
7. THE System SHALL log session lifecycle events
8. THE System SHALL support immediate session termination

### Requirement 8: Consent and Authorization

**User Story:** As a device owner, I want consent controls, so that I authorize all access.

#### Acceptance Criteria

1. THE System SHALL require explicit consent for attended sessions
2. THE System SHALL display visible indicator during active sessions
3. THE System SHALL provide "panic button" to terminate all sessions
4. THE System SHALL enforce permission scoping (view, control, clipboard, files)
5. THE System SHALL never grant more permissions than paired
6. THE System SHALL log all permission usage
7. THE System SHALL support time-based access restrictions
8. THE System SHALL require re-consent for permission escalation

### Requirement 9: Audit and Logging

**User Story:** As an administrator, I want comprehensive audit logs, so that I can investigate incidents.

#### Acceptance Criteria

1. THE System SHALL log all authentication events
2. THE System SHALL log all pairing events
3. THE System SHALL log all session events
4. THE System SHALL log all permission changes
5. THE System SHALL log all security-relevant errors
6. THE System SHALL sign audit log entries for integrity
7. THE System SHALL protect audit logs from tampering
8. THE System SHALL support audit log export for analysis

### Requirement 10: Rate Limiting and Abuse Prevention

**User Story:** As a system operator, I want abuse prevention, so that the system resists attacks.

#### Acceptance Criteria

1. THE System SHALL rate limit authentication attempts
2. THE System SHALL rate limit pairing requests
3. THE System SHALL rate limit session requests
4. THE System SHALL implement exponential backoff for failures
5. THE System SHALL support IP-based blocking
6. THE System SHALL detect and block enumeration attempts
7. THE System SHALL alert on abuse patterns
8. THE System SHALL support allowlisting for trusted sources

### Requirement 11: Secure Updates

**User Story:** As a user, I want secure updates, so that I'm protected from malicious updates.

#### Acceptance Criteria

1. THE System SHALL verify update signatures before installation
2. THE System SHALL pin update signing keys in binaries
3. THE System SHALL verify update manifest integrity
4. THE System SHALL support rollback on update failure
5. THE System SHALL verify update source authenticity
6. THE System SHALL log update events
7. THE System SHALL support update channel selection
8. THE System SHALL alert on update verification failures

### Requirement 12: Privacy Protection

**User Story:** As a user, I want privacy protection, so that my usage is not tracked unnecessarily.

#### Acceptance Criteria

1. THE System SHALL minimize metadata collection
2. THE System SHALL not log session content
3. THE System SHALL support anonymous usage (no account required)
4. THE System SHALL encrypt all data in transit
5. THE System SHALL document data retention policies
6. THE System SHALL support data export for users
7. THE System SHALL support account/data deletion
8. THE System SHALL not share data with third parties

### Requirement 13: Security Testing

**User Story:** As a developer, I want security testing, so that vulnerabilities are found early.

#### Acceptance Criteria

1. THE System SHALL include fuzzing targets for parsers
2. THE System SHALL include property tests for crypto code
3. THE System SHALL run security-focused static analysis
4. THE System SHALL support penetration testing
5. THE System SHALL have vulnerability disclosure process
6. THE System SHALL track and remediate security issues
7. THE System SHALL conduct periodic security reviews
8. THE System SHALL maintain security regression tests
