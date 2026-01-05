use crate::db::store::DbStore;
use crate::db::schema::Session;
use anyhow::Result;
use chrono::{Duration, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionService {
    store: DbStore,
}

impl SessionService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn create_session(&self, user_id: &str) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let timeout_minutes = std::env::var("SESSION_TIMEOUT_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(1440); // Default 24 hours
        let expires_at = Utc::now() + Duration::minutes(timeout_minutes);

        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)"
        )
        .bind(&session_id)
        .bind(user_id)
        .bind(expires_at)
        .execute(self.store.get_pool())
        .await?;

        Ok(session_id)
    }

    pub async fn validate_session(&self, session_id: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = ? AND expires_at > ?"
        )
        .bind(session_id)
        .bind(Utc::now())
        .fetch_optional(self.store.get_pool())
        .await?;

        Ok(session)
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(self.store.get_pool())
            .await?;
        Ok(())
    }
}
