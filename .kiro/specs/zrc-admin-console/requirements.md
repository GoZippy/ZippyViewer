# Requirements Document: zrc-admin-console

## Introduction

The zrc-admin-console crate implements a web-based administration interface for self-hosted Zippy Remote Control (ZRC) deployments. This console provides centralized management of pairings, audit logs, directory nodes, relay servers, and update channels. It is designed for small to medium deployments with privacy-first defaults.

## Glossary

- **Admin_Console**: Web UI for managing ZRC infrastructure
- **Tenant**: An organizational unit (single tenant for self-hosted)
- **RBAC**: Role-Based Access Control for admin permissions
- **Audit_Log**: Immutable record of security-relevant events
- **Pairing**: Trust relationship between operator and device
- **Revocation**: Terminating a pairing or access token
- **SSO**: Single Sign-On integration (optional)

## Requirements

### Requirement 1: Authentication

**User Story:** As an administrator, I want secure authentication, so that only authorized users can access the console.

#### Acceptance Criteria

1. THE Admin_Console SHALL support local username/password authentication
2. THE Admin_Console SHALL enforce strong password requirements (min 12 chars, complexity)
3. THE Admin_Console SHALL support TOTP-based two-factor authentication
4. THE Admin_Console SHALL implement session management with configurable timeout
5. THE Admin_Console SHALL lock accounts after configurable failed login attempts
6. THE Admin_Console SHALL log all authentication events
7. THE Admin_Console SHALL support password reset via secure token
8. THE Admin_Console SHALL hash passwords using Argon2id

### Requirement 2: SSO Integration (Optional)

**User Story:** As an enterprise administrator, I want SSO integration, so that users can authenticate with existing identity providers.

#### Acceptance Criteria

1. THE Admin_Console SHALL support OIDC authentication (optional feature)
2. THE Admin_Console SHALL support SAML 2.0 authentication (optional feature)
3. THE Admin_Console SHALL map SSO groups to console roles
4. THE Admin_Console SHALL support JIT (Just-In-Time) user provisioning
5. THE Admin_Console SHALL handle SSO session expiration
6. THE Admin_Console SHALL support multiple identity providers
7. THE Admin_Console SHALL fall back to local auth if SSO unavailable
8. THE Admin_Console SHALL log SSO authentication events

### Requirement 3: Role-Based Access Control

**User Story:** As an administrator, I want role-based permissions, so that users have appropriate access levels.

#### Acceptance Criteria

1. THE Admin_Console SHALL define roles: Super Admin, Admin, Operator, Viewer
2. THE Admin_Console SHALL enforce role permissions on all operations
3. THE Super_Admin role SHALL have full access to all features
4. THE Admin role SHALL manage pairings, users, and view logs
5. THE Operator role SHALL view devices and initiate sessions
6. THE Viewer role SHALL have read-only access to dashboards
7. THE Admin_Console SHALL support custom role definitions (optional)
8. THE Admin_Console SHALL log permission denied events

### Requirement 4: Pairing Management

**User Story:** As an administrator, I want to manage device pairings, so that I can control access to devices.

#### Acceptance Criteria

1. THE Admin_Console SHALL display list of all pairings
2. THE Admin_Console SHALL show pairing details: device, operator, permissions, created date
3. THE Admin_Console SHALL support filtering and searching pairings
4. THE Admin_Console SHALL support revoking individual pairings
5. THE Admin_Console SHALL support bulk revocation
6. THE Admin_Console SHALL show pairing status (active, revoked)
7. THE Admin_Console SHALL display last session time per pairing
8. THE Admin_Console SHALL require confirmation for revocation

### Requirement 5: Device Management

**User Story:** As an administrator, I want to manage devices, so that I can monitor and control the device fleet.

#### Acceptance Criteria

1. THE Admin_Console SHALL display list of registered devices
2. THE Admin_Console SHALL show device details: name, ID, last seen, version
3. THE Admin_Console SHALL show device online/offline status
4. THE Admin_Console SHALL support device grouping and tagging
5. THE Admin_Console SHALL support device search and filtering
6. THE Admin_Console SHALL support device removal (revokes all pairings)
7. THE Admin_Console SHALL display device session history
8. THE Admin_Console SHALL support device notes/comments

### Requirement 6: Audit Log Viewing

**User Story:** As an administrator, I want to view audit logs, so that I can investigate security events.

#### Acceptance Criteria

1. THE Admin_Console SHALL display audit log entries in chronological order
2. THE Admin_Console SHALL show: timestamp, event type, actor, target, details
3. THE Admin_Console SHALL support filtering by: date range, event type, actor, device
4. THE Admin_Console SHALL support full-text search in audit logs
5. THE Admin_Console SHALL support audit log export (CSV, JSON)
6. THE Admin_Console SHALL paginate large result sets
7. THE Admin_Console SHALL highlight security-critical events
8. THE Admin_Console SHALL retain audit logs for configurable period

### Requirement 7: Directory Node Management

**User Story:** As an administrator, I want to manage directory nodes, so that I can configure device discovery.

#### Acceptance Criteria

1. THE Admin_Console SHALL display configured directory nodes
2. THE Admin_Console SHALL show node status (online, offline, error)
3. THE Admin_Console SHALL support adding/removing directory nodes
4. THE Admin_Console SHALL display node statistics (records, queries)
5. THE Admin_Console SHALL support node health checks
6. THE Admin_Console SHALL configure node federation settings
7. THE Admin_Console SHALL display node configuration
8. THE Admin_Console SHALL support node priority ordering

### Requirement 8: Relay Server Management

**User Story:** As an administrator, I want to manage relay servers, so that I can ensure connectivity.

#### Acceptance Criteria

1. THE Admin_Console SHALL display configured relay servers
2. THE Admin_Console SHALL show relay status and health
3. THE Admin_Console SHALL display relay statistics (connections, bandwidth)
4. THE Admin_Console SHALL support adding/removing relay servers
5. THE Admin_Console SHALL configure relay quotas and limits
6. THE Admin_Console SHALL display geographic distribution
7. THE Admin_Console SHALL support relay priority configuration
8. THE Admin_Console SHALL alert on relay capacity issues

### Requirement 9: Update Channel Management

**User Story:** As an administrator, I want to manage updates, so that I can control software versions.

#### Acceptance Criteria

1. THE Admin_Console SHALL display available update channels
2. THE Admin_Console SHALL show current version per device
3. THE Admin_Console SHALL support assigning devices to update channels
4. THE Admin_Console SHALL display update rollout status
5. THE Admin_Console SHALL support pausing/resuming updates
6. THE Admin_Console SHALL show update history
7. THE Admin_Console SHALL support rollback to previous version
8. THE Admin_Console SHALL alert on failed updates

### Requirement 10: Dashboard and Monitoring

**User Story:** As an administrator, I want a dashboard, so that I can see system health at a glance.

#### Acceptance Criteria

1. THE Admin_Console SHALL display summary dashboard on login
2. THE Admin_Console SHALL show: active sessions, online devices, recent events
3. THE Admin_Console SHALL display system health indicators
4. THE Admin_Console SHALL show bandwidth and connection trends
5. THE Admin_Console SHALL highlight alerts and warnings
6. THE Admin_Console SHALL support dashboard customization
7. THE Admin_Console SHALL auto-refresh dashboard data
8. THE Admin_Console SHALL support multiple dashboard views

### Requirement 11: API Access

**User Story:** As an administrator, I want API access, so that I can integrate with other systems.

#### Acceptance Criteria

1. THE Admin_Console SHALL provide REST API for all management functions
2. THE Admin_Console SHALL support API key authentication
3. THE Admin_Console SHALL support OAuth2 client credentials flow
4. THE Admin_Console SHALL document API with OpenAPI specification
5. THE Admin_Console SHALL rate limit API requests
6. THE Admin_Console SHALL log API access for audit
7. THE Admin_Console SHALL support API key rotation
8. THE Admin_Console SHALL provide API versioning

### Requirement 12: Deployment and Configuration

**User Story:** As a self-hoster, I want simple deployment, so that I can run the console easily.

#### Acceptance Criteria

1. THE Admin_Console SHALL be distributed as single binary + static assets
2. THE Admin_Console SHALL support SQLite database (default)
3. THE Admin_Console SHALL support PostgreSQL database (optional)
4. THE Admin_Console SHALL accept configuration via environment variables
5. THE Admin_Console SHALL support running behind reverse proxy
6. THE Admin_Console SHALL provide Docker image (optional)
7. THE Admin_Console SHALL support HTTPS with configurable certificates
8. THE Admin_Console SHALL run on same host as dirnode/rendezvous
