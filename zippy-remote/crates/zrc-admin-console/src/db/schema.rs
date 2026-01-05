use sqlx::FromRow;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    SuperAdmin,
    Admin,
    Operator,
    Viewer,
}

impl ToString for UserRole {
    fn to_string(&self) -> String {
        match self {
            UserRole::SuperAdmin => "SuperAdmin".to_string(),
            UserRole::Admin => "Admin".to_string(),
            UserRole::Operator => "Operator".to_string(),
            UserRole::Viewer => "Viewer".to_string(),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SuperAdmin" => Ok(UserRole::SuperAdmin),
            "Admin" => Ok(UserRole::Admin),
            "Operator" => Ok(UserRole::Operator),
            "Viewer" => Ok(UserRole::Viewer),
            _ => Err(format!("Invalid role: {}", s)),
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub role: String, // Stored as string, mapped to enum in logic
    pub totp_secret: Option<String>,
    #[sqlx(default)]
    pub totp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub target: String,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub status: String, // 'online', 'offline'
    pub group_name: Option<String>,
    pub tags: Option<String>, // JSON array
    pub channel_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Pairing {
    pub id: String,
    pub device_id: String,
    pub user_id: String,
    pub status: String, // 'active', 'revoked', 'expired'
    pub permissions: String, // JSON
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Relay {
    pub id: String,
    pub url: String,
    pub region: Option<String>,
    pub status: String, // 'active', 'maintenance', 'offline'
    pub capacity: Option<i64>,
    pub connected_clients: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Dirnode {
    pub id: String,
    pub url: String,
    pub public_key: Option<String>,
    pub status: String, // 'active', 'maintenance', 'offline'
    pub region: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UpdateChannel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub channel_id: String,
    pub url: String,
    pub checksum: String,
    pub changelog: Option<String>,
    pub published_at: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub key_hash: String,
    pub prefix: String,
    pub name: String,
    pub permissions: String, // JSON
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}
