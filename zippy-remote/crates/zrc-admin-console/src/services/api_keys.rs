use crate::db::store::DbStore;
use crate::db::schema::ApiKey;
use anyhow::{Result, anyhow};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use uuid::Uuid;
use chrono::{Utc, DateTime};
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};

#[derive(Clone)]
pub struct ApiKeyService {
    store: DbStore,
}

impl ApiKeyService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn list_keys(&self, user_id: &str) -> Result<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            "SELECT * FROM api_keys WHERE user_id = ? ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(keys)
    }

    pub async fn create_key(
        &self,
        user_id: &str,
        name: &str,
        permissions: &str, // JSON string
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(ApiKey, String)> {
        // Generate random key
        let key_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let full_key = format!("zrc_{}", key_string);
        
        let prefix = &full_key[0..8];

        // Hash key
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let key_hash = argon2.hash_password(full_key.as_bytes(), &salt)
            .map_err(|e| anyhow!("Hashing failed: {}", e))?
            .to_string();

        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();

        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (id, user_id, key_hash, prefix, name, permissions, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id, user_id, key_hash, prefix, name, permissions, created_at, expires_at, last_used_at
            "#
        )
        .bind(id)
        .bind(user_id)
        .bind(key_hash)
        .bind(prefix)
        .bind(name)
        .bind(permissions)
        .bind(created_at)
        .bind(expires_at)
        .fetch_one(self.store.get_pool())
        .await?;

        Ok((api_key, full_key))
    }

    pub async fn revoke_key(&self, id: &str, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(self.store.get_pool())
            .await?;
        Ok(())
    }
    
    // Validate key would assume we lookup by prefix (optimization) or iterate
    // Since we don't store the key, we can't lookup by key directly without prefix hinting
    // But hashing entire key is safer.
    // For now, let's assume validation happens elsewhere or we might need it for middleware. 
    // Actually, normally you send `zrc_PREFIX...`. We can exact match prefix to narrow down candidates.
}
