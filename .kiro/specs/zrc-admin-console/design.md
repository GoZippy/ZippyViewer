# Design Document: zrc-admin-console

## Overview

The zrc-admin-console crate implements a web-based administration interface for self-hosted ZRC deployments. Built with Rust (Axum) backend and a modern frontend (React/TypeScript), it provides centralized management of pairings, audit logs, infrastructure components, and update channels.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         zrc-admin-console                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Frontend (React/TypeScript)                      │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Dashboard  │  │   Device    │  │   Audit     │                  │   │
│  │  │    View     │  │   Manager   │  │    Logs     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Pairing    │  │   Infra     │  │  Settings   │                  │   │
│  │  │  Manager    │  │   Status    │  │    View     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │ REST API                                │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Backend (Axum/Rust)                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │    Auth     │  │    API      │  │   RBAC      │                  │   │
│  │  │  Middleware │  │   Routes    │  │   Engine    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Pairing   │  │   Audit     │  │   Infra     │                  │   │
│  │  │   Service   │  │   Service   │  │   Service   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Data Layer                                       │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   SQLite    │  │  PostgreSQL │  │   Redis     │                  │   │
│  │  │  (default)  │  │  (optional) │  │  (sessions) │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```


## Components and Interfaces

### Backend API Routes

```rust
use axum::{Router, routing::{get, post, put, delete}};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Authentication
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/refresh", post(auth::refresh_token))
        .route("/api/auth/totp/setup", post(auth::setup_totp))
        .route("/api/auth/totp/verify", post(auth::verify_totp))
        
        // Users (admin only)
        .route("/api/users", get(users::list).post(users::create))
        .route("/api/users/:id", get(users::get).put(users::update).delete(users::delete))
        .route("/api/users/:id/roles", put(users::update_roles))
        
        // Devices
        .route("/api/devices", get(devices::list))
        .route("/api/devices/:id", get(devices::get).put(devices::update).delete(devices::delete))
        .route("/api/devices/:id/pairings", get(devices::list_pairings))
        
        // Pairings
        .route("/api/pairings", get(pairings::list))
        .route("/api/pairings/:id", get(pairings::get).delete(pairings::revoke))
        .route("/api/pairings/bulk-revoke", post(pairings::bulk_revoke))
        
        // Audit logs
        .route("/api/audit", get(audit::list))
        .route("/api/audit/export", get(audit::export))
        
        // Infrastructure
        .route("/api/infra/dirnodes", get(infra::list_dirnodes).post(infra::add_dirnode))
        .route("/api/infra/dirnodes/:id", delete(infra::remove_dirnode))
        .route("/api/infra/relays", get(infra::list_relays).post(infra::add_relay))
        .route("/api/infra/relays/:id", delete(infra::remove_relay))
        .route("/api/infra/health", get(infra::health_check))
        
        // Updates
        .route("/api/updates/channels", get(updates::list_channels))
        .route("/api/updates/devices", get(updates::device_versions))
        .route("/api/updates/assign", post(updates::assign_channel))
        
        // Dashboard
        .route("/api/dashboard/summary", get(dashboard::summary))
        .route("/api/dashboard/metrics", get(dashboard::metrics))
        
        .with_state(state)
}
```

### Authentication Service

```rust
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use totp_rs::{TOTP, Algorithm};

pub struct AuthService {
    db: Pool<Sqlite>,
    session_store: SessionStore,
    config: AuthConfig,
}

impl AuthService {
    /// Authenticate user with password
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthResult, AuthError> {
        let user = self.db.get_user_by_username(username).await?
            .ok_or(AuthError::InvalidCredentials)?;
        
        // Check account lockout
        if user.locked_until.map(|t| t > Utc::now()).unwrap_or(false) {
            return Err(AuthError::AccountLocked);
        }
        
        // Verify password
        let parsed_hash = PasswordHash::new(&user.password_hash)?;
        if Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_err() {
            self.record_failed_attempt(&user).await?;
            return Err(AuthError::InvalidCredentials);
        }
        
        // Check if TOTP required
        if user.totp_enabled {
            return Ok(AuthResult::TotpRequired { user_id: user.id });
        }
        
        // Create session
        let session = self.create_session(&user).await?;
        Ok(AuthResult::Success { session })
    }
    
    /// Verify TOTP code
    pub async fn verify_totp(
        &self,
        user_id: Uuid,
        code: &str,
    ) -> Result<Session, AuthError> {
        let user = self.db.get_user(user_id).await?
            .ok_or(AuthError::UserNotFound)?;
        
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            user.totp_secret.as_ref().ok_or(AuthError::TotpNotConfigured)?,
        )?;
        
        if !totp.check_current(code)? {
            return Err(AuthError::InvalidTotp);
        }
        
        self.create_session(&user).await
    }
    
    /// Hash password with Argon2id
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        Ok(argon2.hash_password(password.as_bytes(), &salt)?.to_string())
    }
}

pub enum AuthResult {
    Success { session: Session },
    TotpRequired { user_id: Uuid },
}
```

### RBAC Engine

```rust
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    SuperAdmin,
    Admin,
    Operator,
    Viewer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    // User management
    UsersRead,
    UsersWrite,
    UsersDelete,
    
    // Device management
    DevicesRead,
    DevicesWrite,
    DevicesDelete,
    
    // Pairing management
    PairingsRead,
    PairingsRevoke,
    
    // Audit logs
    AuditRead,
    AuditExport,
    
    // Infrastructure
    InfraRead,
    InfraWrite,
    
    // Updates
    UpdatesRead,
    UpdatesWrite,
    
    // Settings
    SettingsRead,
    SettingsWrite,
}

pub struct RbacEngine {
    role_permissions: HashMap<Role, HashSet<Permission>>,
}

impl RbacEngine {
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();
        
        // SuperAdmin: all permissions
        role_permissions.insert(Role::SuperAdmin, Permission::all());
        
        // Admin: most permissions except user deletion
        let admin_perms: HashSet<_> = Permission::all()
            .into_iter()
            .filter(|p| *p != Permission::UsersDelete)
            .collect();
        role_permissions.insert(Role::Admin, admin_perms);
        
        // Operator: read + limited write
        role_permissions.insert(Role::Operator, hashset![
            Permission::DevicesRead,
            Permission::PairingsRead,
            Permission::AuditRead,
            Permission::InfraRead,
        ]);
        
        // Viewer: read only
        role_permissions.insert(Role::Viewer, hashset![
            Permission::DevicesRead,
            Permission::PairingsRead,
        ]);
        
        Self { role_permissions }
    }
    
    pub fn has_permission(&self, role: Role, permission: Permission) -> bool {
        self.role_permissions
            .get(&role)
            .map(|perms| perms.contains(&permission))
            .unwrap_or(false)
    }
    
    pub fn check_permission(&self, user: &User, permission: Permission) -> Result<(), AuthError> {
        if self.has_permission(user.role, permission) {
            Ok(())
        } else {
            Err(AuthError::PermissionDenied)
        }
    }
}
```

### Pairing Service

```rust
pub struct PairingService {
    db: Pool<Sqlite>,
    audit: AuditService,
}

impl PairingService {
    /// List all pairings with filtering
    pub async fn list(
        &self,
        filter: PairingFilter,
        pagination: Pagination,
    ) -> Result<PaginatedResult<PairingInfo>, ServiceError> {
        let query = sqlx::query_as!(
            PairingInfo,
            r#"
            SELECT 
                p.id, p.device_id, p.operator_id, p.permissions,
                p.created_at, p.revoked_at, p.last_session_at,
                d.name as device_name, o.name as operator_name
            FROM pairings p
            JOIN devices d ON p.device_id = d.id
            JOIN operators o ON p.operator_id = o.id
            WHERE ($1 IS NULL OR p.device_id = $1)
              AND ($2 IS NULL OR p.operator_id = $2)
              AND ($3 IS NULL OR p.revoked_at IS NULL)
            ORDER BY p.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            filter.device_id,
            filter.operator_id,
            filter.active_only,
            pagination.limit,
            pagination.offset,
        );
        
        let items = query.fetch_all(&self.db).await?;
        let total = self.count_pairings(&filter).await?;
        
        Ok(PaginatedResult { items, total, pagination })
    }
    
    /// Revoke a pairing
    pub async fn revoke(
        &self,
        pairing_id: Uuid,
        actor: &User,
        reason: &str,
    ) -> Result<(), ServiceError> {
        let pairing = self.get(pairing_id).await?
            .ok_or(ServiceError::NotFound)?;
        
        sqlx::query!(
            "UPDATE pairings SET revoked_at = $1, revoked_by = $2, revoke_reason = $3 WHERE id = $4",
            Utc::now(),
            actor.id,
            reason,
            pairing_id,
        )
        .execute(&self.db)
        .await?;
        
        // Audit log
        self.audit.log(AuditEvent {
            event_type: AuditEventType::PairingRevoked,
            actor_id: actor.id,
            target_id: Some(pairing_id),
            details: json!({ "reason": reason }),
            timestamp: Utc::now(),
        }).await?;
        
        Ok(())
    }
    
    /// Bulk revoke pairings
    pub async fn bulk_revoke(
        &self,
        pairing_ids: Vec<Uuid>,
        actor: &User,
        reason: &str,
    ) -> Result<BulkResult, ServiceError> {
        let mut success = 0;
        let mut failed = 0;
        
        for id in pairing_ids {
            match self.revoke(id, actor, reason).await {
                Ok(_) => success += 1,
                Err(_) => failed += 1,
            }
        }
        
        Ok(BulkResult { success, failed })
    }
}
```

### Audit Service

```rust
pub struct AuditService {
    db: Pool<Sqlite>,
}

impl AuditService {
    /// Log audit event
    pub async fn log(&self, event: AuditEvent) -> Result<(), ServiceError> {
        sqlx::query!(
            r#"
            INSERT INTO audit_log (id, event_type, actor_id, target_id, details, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            Uuid::new_v4(),
            event.event_type.as_str(),
            event.actor_id,
            event.target_id,
            serde_json::to_string(&event.details)?,
            event.timestamp,
        )
        .execute(&self.db)
        .await?;
        
        Ok(())
    }
    
    /// Query audit logs
    pub async fn query(
        &self,
        filter: AuditFilter,
        pagination: Pagination,
    ) -> Result<PaginatedResult<AuditEntry>, ServiceError> {
        // Build dynamic query based on filter
        let entries = sqlx::query_as!(
            AuditEntry,
            r#"
            SELECT * FROM audit_log
            WHERE ($1 IS NULL OR timestamp >= $1)
              AND ($2 IS NULL OR timestamp <= $2)
              AND ($3 IS NULL OR event_type = $3)
              AND ($4 IS NULL OR actor_id = $4)
              AND ($5 IS NULL OR details LIKE '%' || $5 || '%')
            ORDER BY timestamp DESC
            LIMIT $6 OFFSET $7
            "#,
            filter.start_date,
            filter.end_date,
            filter.event_type,
            filter.actor_id,
            filter.search_text,
            pagination.limit,
            pagination.offset,
        )
        .fetch_all(&self.db)
        .await?;
        
        Ok(PaginatedResult { items: entries, ..Default::default() })
    }
    
    /// Export audit logs
    pub async fn export(
        &self,
        filter: AuditFilter,
        format: ExportFormat,
    ) -> Result<Vec<u8>, ServiceError> {
        let entries = self.query(filter, Pagination::unlimited()).await?;
        
        match format {
            ExportFormat::Csv => self.to_csv(&entries.items),
            ExportFormat::Json => Ok(serde_json::to_vec_pretty(&entries.items)?),
        }
    }
}

#[derive(Clone, Copy)]
pub enum AuditEventType {
    UserLogin,
    UserLogout,
    UserCreated,
    UserDeleted,
    PairingCreated,
    PairingRevoked,
    DeviceRegistered,
    DeviceRemoved,
    SessionStarted,
    SessionEnded,
    SettingsChanged,
    InfraChanged,
}
```

### Infrastructure Service

```rust
pub struct InfraService {
    db: Pool<Sqlite>,
    http_client: reqwest::Client,
}

impl InfraService {
    /// List directory nodes with health status
    pub async fn list_dirnodes(&self) -> Result<Vec<DirnodeInfo>, ServiceError> {
        let nodes = sqlx::query_as!(DirnodeRecord, "SELECT * FROM dirnodes")
            .fetch_all(&self.db)
            .await?;
        
        let mut result = Vec::new();
        for node in nodes {
            let health = self.check_dirnode_health(&node.url).await;
            result.push(DirnodeInfo {
                id: node.id,
                url: node.url,
                name: node.name,
                priority: node.priority,
                health,
            });
        }
        
        Ok(result)
    }
    
    /// Check directory node health
    async fn check_dirnode_health(&self, url: &str) -> HealthStatus {
        let health_url = format!("{}/health", url);
        
        match self.http_client.get(&health_url).timeout(Duration::from_secs(5)).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(health) = resp.json::<HealthResponse>().await {
                    HealthStatus::Healthy {
                        records: health.record_count,
                        uptime: health.uptime_seconds,
                    }
                } else {
                    HealthStatus::Degraded { reason: "Invalid response".into() }
                }
            }
            Ok(resp) => HealthStatus::Unhealthy { 
                reason: format!("HTTP {}", resp.status()) 
            },
            Err(e) => HealthStatus::Unreachable { 
                reason: e.to_string() 
            },
        }
    }
}

pub enum HealthStatus {
    Healthy { records: u64, uptime: u64 },
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unreachable { reason: String },
}
```

## Data Models

### Database Schema

```sql
-- Users table
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'viewer',
    totp_enabled INTEGER DEFAULT 0,
    totp_secret TEXT,
    failed_attempts INTEGER DEFAULT 0,
    locked_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Sessions table
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    token_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    ip_address TEXT,
    user_agent TEXT
);

-- Audit log table
CREATE TABLE audit_log (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    actor_id TEXT REFERENCES users(id),
    target_id TEXT,
    details TEXT,
    timestamp TEXT NOT NULL,
    ip_address TEXT
);

CREATE INDEX idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_event_type ON audit_log(event_type);
CREATE INDEX idx_audit_actor ON audit_log(actor_id);
```

### Configuration

```toml
# admin-console.toml

[server]
host = "0.0.0.0"
port = 8080
tls_cert = ""  # Empty = HTTP only
tls_key = ""

[database]
type = "sqlite"  # "sqlite" | "postgres"
url = "data/admin.db"

[auth]
session_timeout_hours = 24
max_failed_attempts = 5
lockout_duration_minutes = 30
password_min_length = 12
require_totp = false

[logging]
level = "info"
audit_retention_days = 365
```

## Correctness Properties

### Property 1: Authentication Security
*For any* login attempt, the password SHALL be verified using Argon2id with timing-safe comparison.
**Validates: Requirements 1.1, 1.8**

### Property 2: RBAC Enforcement
*For any* API request, the user's role SHALL be checked against required permissions before processing.
**Validates: Requirements 3.2, 3.3, 3.4, 3.5, 3.6**

### Property 3: Audit Completeness
*For any* security-relevant operation, an audit log entry SHALL be created with actor, target, and timestamp.
**Validates: Requirements 6.1, 6.2, 6.3**

### Property 4: Session Security
*For any* session, the token SHALL be hashed before storage and validated on each request.
**Validates: Requirement 1.4**

## Error Handling

| Error Condition | HTTP Status | Response |
|-----------------|-------------|----------|
| Invalid credentials | 401 | `{"error": "invalid_credentials"}` |
| Account locked | 403 | `{"error": "account_locked", "until": "..."}` |
| Permission denied | 403 | `{"error": "permission_denied"}` |
| Resource not found | 404 | `{"error": "not_found"}` |
| Validation error | 400 | `{"error": "validation", "details": [...]}` |

## Testing Strategy

### Unit Tests
- Password hashing and verification
- RBAC permission checks
- Audit event creation
- Filter query building

### Integration Tests
- Full authentication flow
- API endpoint authorization
- Database operations
- Health check endpoints

### Security Tests
- SQL injection prevention
- XSS prevention
- CSRF protection
- Rate limiting
