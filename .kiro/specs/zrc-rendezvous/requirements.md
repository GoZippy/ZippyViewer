# Requirements Document: zrc-rendezvous

## Introduction

The zrc-rendezvous crate implements a self-hostable HTTP mailbox server for the Zippy Remote Control (ZRC) system. This server provides untrusted byte-forwarding between endpoints, enabling session initiation when direct peer-to-peer connectivity is not immediately available. The rendezvous server never has access to plaintext message content due to end-to-end encryption.

## Glossary

- **Mailbox**: A queue of encrypted messages addressed to a specific recipient
- **Recipient_ID**: A 32-byte identifier (typically derived from device public key) used to address mailboxes
- **Envelope**: An encrypted message container posted to a mailbox
- **Long_Poll**: HTTP request that waits for new messages before returning
- **TTL**: Time-to-live, the duration before a message expires and is evicted
- **Rate_Limit**: Restriction on request frequency to prevent abuse
- **Bearer_Token**: Optional authentication token for accessing mailboxes

## Requirements

### Requirement 1: Mailbox POST API

**User Story:** As a sender, I want to post encrypted messages to a recipient's mailbox, so that I can initiate communication even when the recipient is behind NAT.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL accept POST requests to /v1/mailbox/{recipient_id_hex}
2. THE Rendezvous_Server SHALL accept raw bytes in the request body (Content-Type: application/octet-stream)
3. WHEN a valid message is posted, THE Rendezvous_Server SHALL return HTTP 202 Accepted
4. WHEN the message exceeds the size limit (default: 64KB), THE Rendezvous_Server SHALL return HTTP 413 Payload Too Large
5. WHEN the sender is rate limited, THE Rendezvous_Server SHALL return HTTP 429 Too Many Requests with Retry-After header
6. WHEN the mailbox queue is full (default: 100 messages), THE Rendezvous_Server SHALL return HTTP 507 Insufficient Storage
7. THE Rendezvous_Server SHALL assign a monotonically increasing sequence number to each posted message
8. THE Rendezvous_Server SHALL record the message timestamp for TTL expiration

### Requirement 2: Mailbox GET API

**User Story:** As a recipient, I want to retrieve messages from my mailbox, so that I can receive session initiation requests.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL accept GET requests to /v1/mailbox/{recipient_id_hex}
2. THE Rendezvous_Server SHALL support optional query parameter: wait_ms (long-poll timeout, default: 30000, max: 60000)
3. WHEN messages are available, THE Rendezvous_Server SHALL return HTTP 200 with the oldest message body
4. WHEN no messages are available and wait_ms > 0, THE Rendezvous_Server SHALL hold the connection until a message arrives or timeout
5. WHEN no messages are available and timeout expires, THE Rendezvous_Server SHALL return HTTP 204 No Content
6. WHEN a message is returned, THE Rendezvous_Server SHALL remove it from the queue (consume-on-read)
7. THE Rendezvous_Server SHALL include X-Message-Sequence header with the message sequence number
8. THE Rendezvous_Server SHALL include X-Queue-Length header with remaining messages count

### Requirement 3: Message Retention and Eviction

**User Story:** As a server operator, I want automatic message cleanup, so that storage doesn't grow unbounded.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL evict messages older than the configured TTL (default: 5 minutes)
2. THE Rendezvous_Server SHALL run eviction checks periodically (default: every 30 seconds)
3. WHEN a mailbox has no messages and no recent activity, THE Rendezvous_Server SHALL remove the empty mailbox after idle timeout (default: 1 hour)
4. THE Rendezvous_Server SHALL enforce per-mailbox message count limits
5. THE Rendezvous_Server SHALL enforce global memory usage limits
6. WHEN memory limit is approached, THE Rendezvous_Server SHALL evict oldest messages first (LRU)
7. THE Rendezvous_Server SHALL log eviction events for monitoring

### Requirement 4: Rate Limiting

**User Story:** As a server operator, I want rate limiting, so that the server is protected from abuse and denial-of-service.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL track request counts per source IP address
2. THE Rendezvous_Server SHALL enforce configurable POST rate limit (default: 60 requests/minute per IP)
3. THE Rendezvous_Server SHALL enforce configurable GET rate limit (default: 120 requests/minute per IP)
4. WHEN rate limit is exceeded, THE Rendezvous_Server SHALL return HTTP 429 with Retry-After header
5. THE Rendezvous_Server SHALL support IP allowlisting for trusted sources
6. THE Rendezvous_Server SHALL support IP blocklisting for known bad actors
7. THE Rendezvous_Server SHALL reset rate limit counters after the configured window (default: 1 minute)

### Requirement 5: Optional Authentication

**User Story:** As a server operator, I want optional authentication, so that I can restrict access to authorized clients.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL support optional bearer token authentication via Authorization header
2. WHEN auth is enabled and token is missing, THE Rendezvous_Server SHALL return HTTP 401 Unauthorized
3. WHEN auth is enabled and token is invalid, THE Rendezvous_Server SHALL return HTTP 403 Forbidden
4. THE Rendezvous_Server SHALL support per-mailbox tokens (recipient controls access to their mailbox)
5. THE Rendezvous_Server SHALL support server-wide tokens (operator controls all access)
6. THE Rendezvous_Server SHALL support token rotation without service interruption
7. WHEN auth is disabled, THE Rendezvous_Server SHALL allow anonymous access (default for self-hosted)

### Requirement 6: Health and Metrics

**User Story:** As a server operator, I want health checks and metrics, so that I can monitor server status and performance.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL expose GET /health returning HTTP 200 when healthy
2. THE Rendezvous_Server SHALL expose GET /metrics in Prometheus format (optional feature)
3. THE Rendezvous_Server SHALL track metrics: active_mailboxes, total_messages, messages_posted, messages_delivered, messages_evicted
4. THE Rendezvous_Server SHALL track metrics: request_latency_histogram, rate_limit_hits, error_counts
5. THE Rendezvous_Server SHALL include uptime and version in health response
6. WHEN the server is overloaded, THE Rendezvous_Server SHALL return HTTP 503 from /health

### Requirement 7: Configuration

**User Story:** As a server operator, I want flexible configuration, so that I can tune the server for my deployment.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL accept configuration via environment variables
2. THE Rendezvous_Server SHALL accept configuration via command-line arguments
3. THE Rendezvous_Server SHALL accept configuration via TOML config file (optional)
4. THE Rendezvous_Server SHALL support configuring: listen_addr, listen_port, max_message_size, max_queue_length, message_ttl_seconds
5. THE Rendezvous_Server SHALL support configuring: rate_limit_posts_per_minute, rate_limit_gets_per_minute, long_poll_max_ms
6. THE Rendezvous_Server SHALL support configuring: auth_enabled, auth_tokens, tls_cert_path, tls_key_path
7. THE Rendezvous_Server SHALL validate configuration on startup and exit with clear error if invalid

### Requirement 8: TLS Support

**User Story:** As a server operator, I want TLS support, so that transport is encrypted even though payloads are already E2E encrypted.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL support running with TLS enabled (HTTPS)
2. THE Rendezvous_Server SHALL support running without TLS (HTTP) for reverse-proxy deployments
3. WHEN TLS is enabled, THE Rendezvous_Server SHALL require valid certificate and key paths
4. THE Rendezvous_Server SHALL support automatic certificate reload on SIGHUP (Unix) or config change
5. THE Rendezvous_Server SHALL enforce TLS 1.2+ when TLS is enabled
6. THE Rendezvous_Server SHALL log TLS handshake failures for debugging

### Requirement 9: Graceful Shutdown

**User Story:** As a server operator, I want graceful shutdown, so that in-flight requests complete before the server stops.

#### Acceptance Criteria

1. WHEN SIGTERM is received, THE Rendezvous_Server SHALL stop accepting new connections
2. WHEN SIGTERM is received, THE Rendezvous_Server SHALL wait for in-flight requests to complete (up to 30 seconds)
3. WHEN graceful shutdown timeout expires, THE Rendezvous_Server SHALL forcefully terminate remaining connections
4. THE Rendezvous_Server SHALL wake all long-polling clients with HTTP 503 during shutdown
5. THE Rendezvous_Server SHALL log shutdown progress and final statistics

### Requirement 10: Deployment Simplicity

**User Story:** As a self-hoster, I want simple deployment, so that I can run the server on minimal hardware.

#### Acceptance Criteria

1. THE Rendezvous_Server SHALL be distributed as a single static binary (no runtime dependencies)
2. THE Rendezvous_Server SHALL run with minimal memory footprint (target: <50MB for 1000 active mailboxes)
3. THE Rendezvous_Server SHALL support running as a systemd service
4. THE Rendezvous_Server SHALL support running in Docker (optional, not required)
5. THE Rendezvous_Server SHALL provide example systemd unit file and config
6. THE Rendezvous_Server SHALL work on x86_64 Linux, ARM64 Linux, and Windows
7. THE Rendezvous_Server SHALL start and be ready to serve within 1 second
