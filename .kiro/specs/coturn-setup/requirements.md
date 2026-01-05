# Requirements Document: coturn-setup

## Introduction

coturn-setup provides deployment and configuration for coturn (CoTURN), a TURN/STUN server used as a fallback relay for WebRTC connections when direct peer-to-peer connectivity fails due to NAT traversal issues. This setup enables ZRC sessions to work across different networks.

## Glossary

- **TURN**: Traversal Using Relays around NAT - protocol for relaying media traffic
- **STUN**: Session Traversal Utilities for NAT - protocol for NAT discovery
- **coturn**: Open-source TURN/STUN server implementation
- **Relay**: Server that forwards traffic between endpoints
- **Allocation**: TURN resource assigned to a session
- **Realm**: Authentication domain in TURN
- **Static Auth Secret**: Shared secret for TURN authentication

## Requirements

### Requirement 1: Docker Deployment

**User Story:** As a system administrator, I want to deploy coturn via Docker, so that setup is consistent and portable.

#### Acceptance Criteria

1. THE coturn-setup SHALL provide a docker-compose.yml file
2. THE docker-compose.yml SHALL configure coturn with appropriate settings
3. THE docker-compose.yml SHALL expose TURN ports (UDP 3478, TCP 3478, TLS 5349)
4. THE docker-compose.yml SHALL expose STUN port (UDP 3478)
5. THE docker-compose.yml SHALL support environment variable configuration
6. THE docker-compose.yml SHALL include health check configuration
7. THE docker-compose.yml SHALL support volume mounts for persistent configuration

### Requirement 2: Configuration Template

**User Story:** As a system administrator, I want a configuration template, so that I can customize coturn for my deployment.

#### Acceptance Criteria

1. THE coturn-setup SHALL provide a turnserver.conf template
2. THE Configuration SHALL support configurable listening addresses
3. THE Configuration SHALL support configurable listening ports
4. THE Configuration SHALL support realm configuration
5. THE Configuration SHALL support static authentication secret
6. THE Configuration SHALL support TLS certificate paths
7. THE Configuration SHALL support log file configuration
8. THE Configuration SHALL support user quota and bandwidth limits
9. THE Configuration SHALL include security best practices (no-cli, no-loopback-peers)
10. THE Configuration SHALL support external IP configuration for NAT scenarios

### Requirement 3: TLS Setup

**User Story:** As a system administrator, I want TLS support, so that TURN traffic is encrypted.

#### Acceptance Criteria

1. THE coturn-setup SHALL support TLS certificate configuration
2. THE coturn-setup SHALL provide instructions for obtaining certificates (Let's Encrypt or self-signed)
3. THE coturn-setup SHALL support certificate renewal automation
4. THE coturn-setup SHALL configure TLS listening on port 5349
5. THE coturn-setup SHALL validate certificate paths on startup
6. THE coturn-setup SHALL log TLS connection status

### Requirement 4: Authentication

**User Story:** As a system administrator, I want secure authentication, so that only authorized clients can use the TURN server.

#### Acceptance Criteria

1. THE coturn-setup SHALL support static authentication secret configuration
2. THE coturn-setup SHALL support time-limited credentials (TURN REST API)
3. THE coturn-setup SHALL support IP-based access control
4. THE coturn-setup SHALL support user-based authentication (optional)
5. THE coturn-setup SHALL document credential generation for ZRC clients
6. THE coturn-setup SHALL support credential rotation

### Requirement 5: Network Configuration

**User Story:** As a system administrator, I want proper network configuration, so that TURN works correctly behind NAT/firewalls.

#### Acceptance Criteria

1. THE coturn-setup SHALL support external IP address configuration
2. THE coturn-setup SHALL support relay IP range configuration
3. THE coturn-setup SHALL document firewall port requirements
4. THE coturn-setup SHALL support multiple network interfaces
5. THE coturn-setup SHALL validate network connectivity on startup
6. THE coturn-setup SHALL support IPv4 and IPv6

### Requirement 6: Monitoring and Logging

**User Story:** As a system administrator, I want monitoring and logging, so that I can troubleshoot issues and track usage.

#### Acceptance Criteria

1. THE coturn-setup SHALL configure log file output
2. THE coturn-setup SHALL support log rotation
3. THE coturn-setup SHALL expose metrics endpoint (if available)
4. THE coturn-setup SHALL log allocation events
5. THE coturn-setup SHALL log authentication failures
6. THE coturn-setup SHALL support structured logging (JSON format)

### Requirement 7: Resource Limits

**User Story:** As a system administrator, I want resource limits, so that the TURN server is protected from abuse.

#### Acceptance Criteria

1. THE coturn-setup SHALL support per-user bandwidth limits
2. THE coturn-setup SHALL support total bandwidth limits
3. THE coturn-setup SHALL support maximum allocation duration
4. THE coturn-setup SHALL support maximum concurrent allocations
5. THE coturn-setup SHALL support quota limits per user
6. THE coturn-setup SHALL log when limits are exceeded

### Requirement 8: Documentation

**User Story:** As a system administrator, I want comprehensive documentation, so that I can deploy and maintain coturn successfully.

#### Acceptance Criteria

1. THE coturn-setup SHALL provide README with deployment instructions
2. THE coturn-setup SHALL document all configuration options
3. THE coturn-setup SHALL provide troubleshooting guide
4. THE coturn-setup SHALL document integration with ZRC clients
5. THE coturn-setup SHALL provide example configurations for common scenarios
6. THE coturn-setup SHALL document security considerations
7. THE coturn-setup SHALL provide performance tuning guidelines

### Requirement 9: High Availability (Optional)

**User Story:** As a system administrator, I want high availability, so that TURN service remains available during failures.

#### Acceptance Criteria

1. THE coturn-setup SHALL support multiple coturn instances behind load balancer
2. THE coturn-setup SHALL document load balancer configuration
3. THE coturn-setup SHALL support health check endpoints
4. THE coturn-setup SHALL document failover procedures
5. THE coturn-setup SHALL support geographic distribution

### Requirement 10: Security Hardening

**User Story:** As a system administrator, I want security hardening, so that the TURN server is protected from attacks.

#### Acceptance Criteria

1. THE coturn-setup SHALL disable unnecessary features (CLI, loopback peers)
2. THE coturn-setup SHALL configure rate limiting
3. THE coturn-setup SHALL support IP allowlisting/blocklisting
4. THE coturn-setup SHALL document DDoS mitigation strategies
5. THE coturn-setup SHALL support running as non-root user
6. THE coturn-setup SHALL document security best practices
