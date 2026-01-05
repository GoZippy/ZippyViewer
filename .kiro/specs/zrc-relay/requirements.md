# Requirements Document: zrc-relay

## Introduction

The zrc-relay crate implements an optional QUIC relay server for the Zippy Remote Control (ZRC) system. This relay provides last-resort connectivity when NAT traversal fails and direct peer-to-peer connections cannot be established. The relay forwards encrypted QUIC datagrams without access to plaintext content, maintaining end-to-end encryption guarantees.

## Glossary

- **Relay**: A server that forwards encrypted traffic between endpoints that cannot connect directly
- **Allocation**: A temporary relay resource assigned to a session
- **Relay_Token**: A signed capability token authorizing relay usage
- **Bandwidth_Cap**: Maximum data transfer rate allowed per allocation
- **Quota**: Total data transfer limit for an allocation
- **QUIC**: A UDP-based transport protocol providing multiplexed streams
- **Forwarding**: Passing packets between endpoints without inspection or modification

## Requirements

### Requirement 1: Relay Token Validation

**User Story:** As a relay operator, I want token-based authentication, so that only authorized sessions can use relay resources.

#### Acceptance Criteria

1. THE Relay_Server SHALL require a valid RelayTokenV1 to create an allocation
2. THE Relay_Server SHALL verify the token signature using the device's pinned public key
3. THE Relay_Server SHALL verify the token has not expired (expires_at > now)
4. THE Relay_Server SHALL verify the allocation_id in the token matches the requested allocation
5. IF token validation fails, THEN THE Relay_Server SHALL reject the allocation request with appropriate error
6. THE Relay_Server SHALL cache validated tokens to avoid repeated signature verification
7. THE Relay_Server SHALL support token refresh for long-running sessions

### Requirement 2: Allocation Lifecycle

**User Story:** As a session participant, I want relay allocations, so that I can establish connectivity when direct connection fails.

#### Acceptance Criteria

1. THE Relay_Server SHALL accept allocation requests containing: relay_token, client_addr, peer_id
2. WHEN allocation is created, THE Relay_Server SHALL assign a unique allocation_id and relay_addr
3. THE Relay_Server SHALL return allocation details: relay_addr, relay_port, allocation_id, expires_at
4. THE Relay_Server SHALL maintain allocation state: active, bytes_transferred, last_activity
5. WHEN allocation expires, THE Relay_Server SHALL terminate the allocation and release resources
6. WHEN either endpoint disconnects, THE Relay_Server SHALL terminate the allocation after idle timeout (default: 30 seconds)
7. THE Relay_Server SHALL support explicit allocation release requests

### Requirement 3: QUIC Datagram Forwarding

**User Story:** As a session participant, I want efficient packet forwarding, so that remote control latency is minimized.

#### Acceptance Criteria

1. THE Relay_Server SHALL forward QUIC datagrams between allocated endpoints without modification
2. THE Relay_Server SHALL NOT inspect or decrypt forwarded packet contents
3. THE Relay_Server SHALL forward packets with minimal added latency (target: <5ms processing time)
4. THE Relay_Server SHALL handle packet sizes up to 1500 bytes (standard MTU)
5. THE Relay_Server SHALL drop packets that exceed the allocation's bandwidth cap
6. THE Relay_Server SHALL track bytes transferred per allocation for quota enforcement
7. WHEN quota is exceeded, THE Relay_Server SHALL terminate the allocation

### Requirement 4: Bandwidth and Quota Limits

**User Story:** As a relay operator, I want resource limits, so that the relay is protected from abuse and resource exhaustion.

#### Acceptance Criteria

1. THE Relay_Server SHALL enforce per-allocation bandwidth limits (default: 10 Mbps)
2. THE Relay_Server SHALL enforce per-allocation data quota (default: 1 GB per session)
3. THE Relay_Server SHALL enforce global bandwidth limit across all allocations
4. THE Relay_Server SHALL enforce maximum concurrent allocations (default: 1000)
5. WHEN bandwidth limit is exceeded, THE Relay_Server SHALL drop excess packets and log the event
6. WHEN quota limit is approached (90%), THE Relay_Server SHALL notify endpoints via control message
7. THE Relay_Server SHALL support configurable limits per token tier (free, paid, unlimited)

### Requirement 5: Connection Management

**User Story:** As a relay operator, I want robust connection handling, so that the relay remains stable under load.

#### Acceptance Criteria

1. THE Relay_Server SHALL accept QUIC connections on the configured listen address
2. THE Relay_Server SHALL support multiple concurrent connections per allocation (for reconnection)
3. THE Relay_Server SHALL detect and handle connection migration (IP address changes)
4. THE Relay_Server SHALL implement connection keepalive (default: 15 second interval)
5. WHEN a connection is idle beyond timeout, THE Relay_Server SHALL close it gracefully
6. THE Relay_Server SHALL handle connection errors without crashing or affecting other allocations
7. THE Relay_Server SHALL log connection events for debugging and monitoring

### Requirement 6: Security Controls

**User Story:** As a relay operator, I want security controls, so that the relay is protected from attacks and misuse.

#### Acceptance Criteria

1. THE Relay_Server SHALL rate limit allocation requests per source IP (default: 10/minute)
2. THE Relay_Server SHALL rate limit connection attempts per source IP (default: 30/minute)
3. THE Relay_Server SHALL support IP allowlisting and blocklisting
4. THE Relay_Server SHALL detect and block amplification attack patterns
5. THE Relay_Server SHALL log security events: rate limits, blocked IPs, invalid tokens
6. THE Relay_Server SHALL support integration with external abuse detection systems
7. THE Relay_Server SHALL never log packet contents (only metadata)

### Requirement 7: Metrics and Monitoring

**User Story:** As a relay operator, I want comprehensive metrics, so that I can monitor relay health and usage.

#### Acceptance Criteria

1. THE Relay_Server SHALL expose GET /health returning HTTP 200 when healthy
2. THE Relay_Server SHALL expose GET /metrics in Prometheus format
3. THE Relay_Server SHALL track metrics: active_allocations, total_allocations, bytes_forwarded, packets_forwarded
4. THE Relay_Server SHALL track metrics: allocation_duration_histogram, bandwidth_usage, quota_usage
5. THE Relay_Server SHALL track metrics: connection_count, error_count, rate_limit_hits
6. THE Relay_Server SHALL support real-time allocation listing for admin purposes
7. THE Relay_Server SHALL include geographic distribution metrics when multiple relays are deployed

### Requirement 8: Configuration

**User Story:** As a relay operator, I want flexible configuration, so that I can tune the relay for my deployment.

#### Acceptance Criteria

1. THE Relay_Server SHALL accept configuration via environment variables
2. THE Relay_Server SHALL accept configuration via command-line arguments
3. THE Relay_Server SHALL accept configuration via TOML config file
4. THE Relay_Server SHALL support configuring: listen_addr, listen_port, quic_cert_path, quic_key_path
5. THE Relay_Server SHALL support configuring: max_allocations, default_bandwidth_limit, default_quota
6. THE Relay_Server SHALL support configuring: allocation_timeout, idle_timeout, keepalive_interval
7. THE Relay_Server SHALL validate configuration on startup and exit with clear error if invalid

### Requirement 9: High Availability

**User Story:** As a relay operator, I want high availability support, so that relay failures don't disrupt sessions.

#### Acceptance Criteria

1. THE Relay_Server SHALL support running multiple instances behind a load balancer
2. THE Relay_Server SHALL support allocation state sharing via external store (Redis) for failover
3. WHEN an instance fails, THE Relay_Server SHALL allow clients to reconnect to another instance
4. THE Relay_Server SHALL support graceful shutdown with allocation migration
5. THE Relay_Server SHALL expose readiness probe for load balancer health checks
6. THE Relay_Server SHALL support geographic distribution for latency optimization

### Requirement 10: Admin API

**User Story:** As a relay operator, I want admin capabilities, so that I can manage allocations and troubleshoot issues.

#### Acceptance Criteria

1. THE Relay_Server SHALL expose admin API on separate port (default: disabled)
2. THE Relay_Server SHALL support GET /admin/allocations to list active allocations
3. THE Relay_Server SHALL support DELETE /admin/allocations/{id} to terminate an allocation
4. THE Relay_Server SHALL support GET /admin/stats for detailed statistics
5. THE Relay_Server SHALL require authentication for admin API access
6. THE Relay_Server SHALL log all admin API actions for audit
7. THE Relay_Server SHALL support admin API over Unix socket for local-only access
