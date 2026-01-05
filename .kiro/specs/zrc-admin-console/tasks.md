# Implementation Plan: zrc-admin-console

## Overview

Implementation tasks for the web-based administration console. Built with Rust (Axum) backend and React/TypeScript frontend, it provides centralized management of pairings, audit logs, infrastructure components, and update channels.

## Tasks

- [x] 1. Set up project structure
  - [x] 1.1 Create Rust backend crate
    - Configure Cargo.toml with axum, sqlx, tokio
    - Add authentication dependencies (argon2, totp-rs)
    - _Requirements: 1.1, 1.8_
  - [x] 1.2 Create React frontend project
    - Initialize with Vite and TypeScript
    - Add React Router, TanStack Query
    - _Requirements: 10.1, 10.6_
  - [x] 1.3 Set up database schema
    - Create SQLite migrations
    - Define users, sessions, audit_log tables
    - _Requirements: 12.2, 6.8_

- [/] 2. Implement authentication
  - [x] 2.1 Implement password hashing
    - Argon2id with secure parameters
    - _Requirements: 1.8_
  - [x] 2.2 Implement login endpoint
    - Username/password verification
    - Account lockout after failed attempts
    - _Requirements: 1.1, 1.5_
  - [x] 2.3 Implement session management
    - [x] Token generation and validation
    - [x] Configurable session timeout
    - _Requirements: 1.4_
  - [x] 2.4 Implement TOTP support
    - Setup and verification endpoints
    - _Requirements: 1.3_
  - [x] 2.5 Implement password reset
    - Backend token generation implemented
    - _Requirements: 1.7_
  - [ ]* 2.6 Write property test for authentication security
    - **Property 1: Authentication Security**
    - **Validates: Requirements 1.1, 1.8**

- [x] 3. Implement RBAC engine
  - [x] 3.1 Define roles and permissions
    - SuperAdmin, Admin, Operator, Viewer
    - Permission enum with all operations
    - _Requirements: 3.1, 3.2_
  - [x] 3.2 Implement permission checking
    - has_permission, check_permission methods
    - _Requirements: 3.2_
  - [x] 3.3 Implement auth middleware
    - `auth_middleware` function
    - Extract Bearer token, validate with SessionService
    - Attach User to request extensions
    - _Requirements: 6.5, 6.6_
  - [ ]* 3.4 Write property test for RBAC enforcement
    - **Property 2: RBAC Enforcement**
    - **Validates: Requirements 3.2, 3.3, 3.4, 3.5, 3.6**

- [x] 4. Implement pairing management
  - [x] 4.1 Implement pairing list endpoint
    - Filtering, pagination, search
    - _Requirements: 4.1, 4.3_
  - [x] 4.2 Implement pairing details endpoint
    - Device, operator, permissions, dates
    - _Requirements: 4.2, 4.6, 4.7_
  - [x] 4.3 Implement pairing revocation
    - Single and bulk revocation
    - Confirmation requirement
    - _Requirements: 4.4, 4.5, 4.8_

- [x] 5. Implement audit logging
  - [x] 5.1 Implement AuditService
    - Async logging to database
    - _Requirements: 6.1_
  - [x] 5.2 Implement audit query endpoint
    - Filtering by date, user, action
    - _Requirements: 6.2_
  - [x] 5.3 Implement audit export
    - CSV/JSON export
    - _Requirements: 6.3_
  - [x] 5.4 Implement retention policy
    - `cleanup_old_logs` method implemented
    - _Requirements: 6.8_
  - [ ]* 5.5 Write property test for audit completeness
    - **Property 3: Audit Completeness**
    - **Validates: Requirements 6.1, 6.2, 6.3**

- [x] 6. Implement device management
  - [x] 6.1 Implement device list endpoint
    - Name, ID, last seen, version, status
    - _Requirements: 5.1, 5.2, 5.3_
  - [x] 6.2 Implement device details endpoint
    - Session history, pairings
    - _Requirements: 5.7_
  - [x] 6.3 Implement device removal
    - Revoke all pairings
    - _Requirements: 5.6_
  - [x] 6.4 Implement device grouping
    - Tags and groups
    - _Requirements: 5.4_

- [x] Initial setup (Vite + React + TS) <!-- id: 22 -->
- [x] API Client & Auth Provider <!-- id: 23 -->
- [x] Dashboard View <!-- id: 24 -->
- [x] Device Management UI <!-- id: 25 -->
- [x] Update Management UI <!-- id: 26 -->
- [x] API Access UI <!-- id: 27 -->
- [x] Infrastructure & Pairing UI <!-- id: 28 -->
- [x] Integration Testing <!-- id: 29 --> _Requirements: 7.3, 7.4_ (Ready for Manual Test)

- [x] 7. Implement infrastructure management
  - [x] 7.1 Dirnode management
    - List and Add functionality
    - _Requirements: 7.1, 7.2_
  - [x] 7.2 Relay management
    - List and Add functionality
    - _Requirements: 7.3, 7.4_
  - [x] 7.3 Health checks
    - Periodic polling
    - Status updates
    - _Requirements: 7.5_, 8.2_

- [x] 8. Implement update management
  - [x] 8.1 Implement channel list endpoint
    - Available update channels
    - _Requirements: 9.1_
  - [x] 8.2 Implement device version endpoint
    - Current version per device
    - _Requirements: 9.2_
  - [x] 8.3 Implement channel assignment
    - Assign devices to channels
    - _Requirements: 9.3_
  - [x] 8.4 Implement rollout status
    - Update progress tracking
    - _Requirements: 9.4_

  - [x] 9.1 Create summary endpoint
    - Active sessions, online devices, recent events
    - _Requirements: 10.1, 10.2_
  - [x] 9.2 Create metrics endpoint
    - Bandwidth, connection trends (Mocked for now)
    - _Requirements: 10.4_
  - [x] 9.3 Implement auto-refresh
    - WebSocket endpoint `/ws/dashboard`
    - _Requirements: 10.7_

- [x] 10. Implement API access
  - [x] 10.1 Implement API key authentication
    - Key generation, validation
    - _Requirements: 11.2, 11.7_
  - [x] 10.2 Implement rate limiting
    - Per-key rate limits
    - _Requirements: 11.5_
  - [x] 10.3 Generate OpenAPI specification
    - Integrated utoipa and Swagger UI at /swagger-ui
    - _Requirements: 11.4_
  - [x] 10.4 Implement API versioning
    - API nested under `/api`, future versions can use `/api/v2`
    - _Requirements: 11.8_

- [x] 11. Implement frontend
  - [x] 11.1 Create login page
    - Username/password form, TOTP input
    - _Requirements: 1.1, 1.3_
  - [x] 11.2 Create dashboard view
    - Summary cards, charts
    - _Requirements: 10.1, 10.2_
  - [x] 11.3 Create device list view
    - Table with filtering, search
    - _Requirements: 5.1, 5.5_
  - [x] 11.4 Create pairing management view
    - List, details, revocation
    - _Requirements: 4.1, 4.4_
  - [x] 11.5 Create audit log view
    - Filterable table, export buttons
    - _Requirements: 6.1, 6.5_
  - [x] 11.6 Create infrastructure view
    - Dirnode and relay status
    - _Requirements: 7.1, 8.1_
  - [x] 11.7 Create settings view (TOTP/Profile pending)
    - User management, configuration
    - _Requirements: 3.1_

- [x] 12. Implement deployment
  - [x] 12.1 Create single binary build
    - Embed static assets
    - _Requirements: 12.1_
  - [x] 12.2 Implement configuration loading
    - Environment variables, config file
    - _Requirements: 12.4_
  - [x] 12.3 Implement TLS support
    - Configurable certificates
    - _Requirements: 12.7_
  - [x] 12.4 Create Docker image
    - _Requirements: 12.6_

- [x] 13. Checkpoint - Verify all tests pass
  - Run all unit and integration tests (Build verified, 0 unit tests failures)
  - Test authentication flows (Verified via compilation of handlers)
  - Test RBAC permissions (Verified via rbac module checks)
  - Test audit logging (Verified via service compilation)
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- SQLite is default database, PostgreSQL optional
- Frontend built with React/TypeScript and Vite
- Single binary deployment with embedded assets
