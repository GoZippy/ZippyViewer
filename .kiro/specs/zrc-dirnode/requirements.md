# Requirements Document: zrc-dirnode

## Introduction

The zrc-dirnode crate implements a home-hostable directory node for the Zippy Remote Control (ZRC) system. This decentralized directory provides device discovery and presence information while maintaining privacy-first defaults. The directory supports invite-only access by default with optional time-bounded discoverability for easier pairing workflows.

## Glossary

- **Directory_Node**: A server that stores and serves signed device presence records
- **DirRecord**: A signed record containing device identity and connectivity hints
- **Discovery_Token**: A time-bounded token enabling temporary device discoverability
- **Invite_Only**: Default mode where devices are only findable via explicit invites
- **Ephemeral_Discovery**: Optional mode where devices can be searched for a brief period
- **Transport_Hints**: Connectivity information including addresses, relay tokens, and mesh hints
- **TTL**: Time-to-live, the validity period of a directory record
- **Subject_ID**: The device identifier that a directory record describes

## Requirements

### Requirement 1: Directory Record Storage

**User Story:** As a device owner, I want to publish my device's presence, so that authorized operators can find connectivity information.

#### Acceptance Criteria

1. THE Dirnode SHALL accept signed DirRecordV1 submissions via POST /v1/records
2. THE Dirnode SHALL verify the record signature using the subject's public key
3. THE Dirnode SHALL verify subject_id matches the signing key's derived ID
4. IF signature verification fails, THEN THE Dirnode SHALL reject the record with HTTP 403
5. THE Dirnode SHALL store records indexed by subject_id
6. THE Dirnode SHALL replace existing records for the same subject_id (upsert behavior)
7. THE Dirnode SHALL enforce maximum record size (default: 4KB)
8. THE Dirnode SHALL enforce maximum TTL (default: 24 hours)

### Requirement 2: Directory Record Retrieval

**User Story:** As an operator, I want to look up device presence, so that I can establish connectivity with paired devices.

#### Acceptance Criteria

1. THE Dirnode SHALL serve GET /v1/records/{subject_id_hex} requests
2. WHEN a valid record exists, THE Dirnode SHALL return HTTP 200 with the DirRecordV1 body
3. WHEN no record exists, THE Dirnode SHALL return HTTP 404 Not Found
4. WHEN the record has expired (timestamp + ttl < now), THE Dirnode SHALL return HTTP 404
5. THE Dirnode SHALL include X-Record-Expires header with expiration timestamp
6. THE Dirnode SHALL support batch lookups via POST /v1/records/batch with list of subject_ids
7. THE Dirnode SHALL return records in the same order as requested subject_ids

### Requirement 3: Invite-Only Access Control

**User Story:** As a device owner, I want invite-only access by default, so that my device is not publicly discoverable.

#### Acceptance Criteria

1. THE Dirnode SHALL require an invite token for record lookups by default
2. THE Dirnode SHALL accept invite tokens via Authorization: Bearer header
3. THE Dirnode SHALL verify invite tokens are signed by the subject device
4. THE Dirnode SHALL verify invite tokens have not expired
5. IF invite token is missing or invalid, THEN THE Dirnode SHALL return HTTP 401 Unauthorized
6. THE Dirnode SHALL support invite tokens that grant access to specific subject_ids only
7. THE Dirnode SHALL log access attempts with invite token IDs for audit

### Requirement 4: Ephemeral Discoverability

**User Story:** As a device owner, I want temporary discoverability, so that I can enable easier pairing for a brief period.

#### Acceptance Criteria

1. THE Dirnode SHALL support DiscoveryTokenV1 creation via POST /v1/discovery/tokens
2. THE Dirnode SHALL generate discovery tokens with configurable TTL (default: 10 minutes, max: 1 hour)
3. THE Dirnode SHALL associate discovery tokens with a subject_id and scope
4. WHEN a discovery token is active, THE Dirnode SHALL allow lookups without invite token
5. THE Dirnode SHALL support discovery token revocation via DELETE /v1/discovery/tokens/{token_id}
6. THE Dirnode SHALL automatically expire discovery tokens after TTL
7. THE Dirnode SHALL limit concurrent active discovery tokens per device (default: 3)
8. THE Dirnode SHALL NOT expose device in any search/listing even with discovery token (lookup by ID only)

### Requirement 5: Search Protection

**User Story:** As a device owner, I want protection from enumeration, so that attackers cannot discover my devices.

#### Acceptance Criteria

1. THE Dirnode SHALL NOT provide any search or listing endpoints
2. THE Dirnode SHALL NOT allow wildcard or partial subject_id lookups
3. THE Dirnode SHALL rate limit lookup requests per source IP (default: 60/minute)
4. THE Dirnode SHALL implement timing-safe responses to prevent timing attacks
5. THE Dirnode SHALL return identical error responses for "not found" and "access denied"
6. THE Dirnode SHALL log and alert on enumeration attempt patterns
7. THE Dirnode SHALL support temporary IP blocking for detected enumeration attempts

### Requirement 6: Record Signature Verification

**User Story:** As an operator, I want verified records, so that I can trust the directory hasn't tampered with device information.

#### Acceptance Criteria

1. THE Dirnode SHALL include the original signature in all returned records
2. THE Dirnode SHALL NOT modify any signed fields in stored records
3. THE Dirnode SHALL verify signatures on record submission
4. THE Dirnode SHALL support signature algorithm extensibility (Ed25519 now, PQC later)
5. WHEN returning a record, THE Dirnode SHALL include X-Signature-Verified: true header
6. THE Dirnode SHALL reject records with signature algorithm not in allowed list

### Requirement 7: Transport Hints

**User Story:** As an operator, I want connectivity hints, so that I can establish the best connection to a device.

#### Acceptance Criteria

1. THE Dirnode SHALL store and return EndpointHintsV1 containing: direct_addrs, relay_tokens, rendezvous_urls, mesh_hints
2. THE Dirnode SHALL support multiple addresses per hint type
3. THE Dirnode SHALL NOT validate or verify transport hints (device responsibility)
4. THE Dirnode SHALL enforce maximum hints count per record (default: 10 per type)
5. THE Dirnode SHALL preserve hint ordering as submitted by device
6. THE Dirnode SHALL support hint updates without full record replacement

### Requirement 8: Persistence and Backup

**User Story:** As a directory operator, I want data persistence, so that records survive restarts and can be backed up.

#### Acceptance Criteria

1. THE Dirnode SHALL persist records to SQLite database by default
2. THE Dirnode SHALL support configurable database path
3. THE Dirnode SHALL implement write-ahead logging for crash safety
4. THE Dirnode SHALL support database backup via VACUUM INTO command
5. THE Dirnode SHALL support export of all records in JSON format for migration
6. THE Dirnode SHALL support import of records from JSON backup
7. THE Dirnode SHALL automatically cleanup expired records (default: hourly)

### Requirement 9: Web UI for Token Generation

**User Story:** As a device owner, I want a simple web interface, so that I can generate invite tokens and QR codes easily.

#### Acceptance Criteria

1. THE Dirnode SHALL serve a minimal web UI at /ui (optional, can be disabled)
2. THE Dirnode SHALL provide UI for generating invite tokens for owned devices
3. THE Dirnode SHALL provide UI for generating discovery tokens
4. THE Dirnode SHALL display QR codes for generated tokens
5. THE Dirnode SHALL provide UI for revoking active tokens
6. THE Dirnode SHALL require authentication for web UI access
7. THE Dirnode SHALL support disabling web UI via configuration

### Requirement 10: Federation (Future)

**User Story:** As a network participant, I want directory federation, so that devices can be discovered across multiple directory nodes.

#### Acceptance Criteria

1. THE Dirnode SHALL support optional federation with other directory nodes
2. THE Dirnode SHALL support record replication to configured peer nodes
3. THE Dirnode SHALL support record lookup forwarding to peer nodes
4. THE Dirnode SHALL verify peer node identity via mutual TLS
5. THE Dirnode SHALL respect record TTL during replication
6. THE Dirnode SHALL support federation enable/disable via configuration
7. THE Dirnode SHALL log federation events for debugging

### Requirement 11: Configuration and Deployment

**User Story:** As a self-hoster, I want simple deployment, so that I can run a directory node on minimal hardware.

#### Acceptance Criteria

1. THE Dirnode SHALL be distributed as a single static binary
2. THE Dirnode SHALL accept configuration via environment variables
3. THE Dirnode SHALL accept configuration via TOML config file
4. THE Dirnode SHALL support configuring: listen_addr, listen_port, database_path, web_ui_enabled
5. THE Dirnode SHALL support configuring: max_record_ttl, max_discovery_ttl, rate_limits
6. THE Dirnode SHALL run with minimal memory footprint (target: <100MB for 10000 records)
7. THE Dirnode SHALL provide example systemd unit file and config
8. THE Dirnode SHALL work on Raspberry Pi class hardware (ARM64)
