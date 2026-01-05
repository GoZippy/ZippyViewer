use crate::db::store::DbStore;
use crate::db::schema::{User, UserRole};
use anyhow::{Result, anyhow};
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use uuid::Uuid;
use rand::Rng;

#[derive(Clone)]
pub struct AuthService {
    store: DbStore,
}

impl AuthService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn get_user(&self, id: &str) -> Result<User> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(self.store.get_pool())
            .await?;
        user.ok_or_else(|| anyhow!("User not found"))
    }

    pub async fn create_user(&self, username: &str, password: &str, role: UserRole) -> Result<User> {
        // Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Hashing failed: {}", e))?
            .to_string();

        let id = Uuid::new_v4().to_string();
        let role_str = role.to_string();

        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (id, username, password_hash, role)
            VALUES (?, ?, ?, ?)
            RETURNING id, username, password_hash, role, totp_secret, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(role_str)
        .fetch_one(self.store.get_pool())
        .await?;

        Ok(user)
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<User> {
        let user: Option<User> = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE username = ?"
        )
        .bind(username)
        .fetch_optional(self.store.get_pool())
        .await?;

        let user = user.ok_or_else(|| anyhow!("Invalid username or password"))?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| anyhow!("Invalid password hash: {}", e))?;

        Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow!("Invalid username or password"))?;

        Ok(user)
    }
    
    // TOTP Methods
    pub async fn generate_totp_secret(&self, user_id: &str, username: &str) -> Result<(String, String)> {
        use totp_rs::{Algorithm, TOTP, Secret};
        
        let mut secret_bytes = [0u8; 20];
        rand::thread_rng().fill(&mut secret_bytes);
        let secret = Secret::Raw(secret_bytes.to_vec());

        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap(),
            Some("ZRC Admin".to_string()),
            username.to_string(),
        ).unwrap();
        
        let qr = totp.get_qr_base64().map_err(|e| anyhow!("QR Gen error: {}", e))?;
        let secret_str = secret.to_encoded().to_string();

        // Save secret to DB (but don't enable it yet)
        sqlx::query("UPDATE users SET totp_secret = ? WHERE id = ?")
            .bind(&secret_str)
            .bind(user_id)
            .execute(self.store.get_pool())
            .await?;

        Ok((secret_str, qr))
    }

    pub async fn verify_and_enable_totp(&self, user_id: &str, code: &str) -> Result<bool> {
        use totp_rs::{Algorithm, TOTP, Secret};

        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(self.store.get_pool())
            .await?;
        
        let user = user.ok_or_else(|| anyhow!("User not found"))?;
        let secret_str = user.totp_secret.ok_or_else(|| anyhow!("TOTP not setup"))?;
        
        let secret = Secret::Encoded(secret_str).to_bytes().map_err(|e| anyhow!("Invalid secret: {}", e))?;
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret,
            Some("ZRC Admin".to_string()),
            user.username,
        ).unwrap();

        let valid = totp.check_current(code).unwrap_or(false);
        if valid {
             sqlx::query("UPDATE users SET totp_enabled = TRUE WHERE id = ?")
                .bind(user_id)
                .execute(self.store.get_pool())
                .await?;
             Ok(true)
        } else {
             Ok(false)
        }
    }
    pub async fn request_password_reset(&self, username: &str) -> Result<String> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(self.store.get_pool())
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        let token = Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);

        sqlx::query("INSERT INTO password_resets (token, user_id, expires_at) VALUES (?, ?, ?)")
            .bind(&token)
            .bind(user.id)
            .bind(expires_at)
            .execute(self.store.get_pool())
            .await?;
            
        // In a real app, send email here.
        
        Ok(token)
    }

    pub async fn reset_password(&self, token: &str, new_password: &str) -> Result<()> {
        use sqlx::Row;
        let record = sqlx::query("SELECT user_id, expires_at FROM password_resets WHERE token = ?")
            .bind(token)
            .fetch_optional(self.store.get_pool())
            .await?;
            
        let row = record.ok_or_else(|| anyhow!("Invalid token"))?;
        let user_id: String = row.try_get("user_id")?;
        let expires_at: chrono::DateTime<chrono::Utc> = row.try_get("expires_at")?;
            
        if chrono::Utc::now() > expires_at {
             return Err(anyhow!("Token expired"));
        }
        
        // Hash new password
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default().hash_password(new_password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Hashing failed: {}", e))?
            .to_string();
            
        // Update user
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(password_hash)
            .bind(user_id)
            .execute(self.store.get_pool())
            .await?;
            
        // Delete token
        sqlx::query("DELETE FROM password_resets WHERE token = ?")
            .bind(token)
            .execute(self.store.get_pool())
            .await?;
            
        Ok(())
    }
}
