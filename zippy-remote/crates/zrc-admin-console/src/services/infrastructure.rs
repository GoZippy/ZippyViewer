use crate::db::store::DbStore;
use crate::db::schema::{Relay, Dirnode};
use anyhow::Result;
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone)]
pub struct InfrastructureService {
    store: DbStore,
}

impl InfrastructureService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn health_check(&self) -> Result<String> {
        // Simple DB ping
        sqlx::query("SELECT 1").execute(self.store.get_pool()).await?;
        Ok("healthy".to_string())
    }

    pub async fn list_relays(&self) -> Result<Vec<Relay>> {
        let relays = sqlx::query_as::<_, Relay>(
            "SELECT * FROM relays ORDER BY created_at DESC"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(relays)
    }

    pub async fn add_relay(&self, url: &str, region: Option<&str>) -> Result<Relay> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();
        
        let relay = sqlx::query_as::<_, Relay>(
            r#"
            INSERT INTO relays (id, url, region, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            RETURNING id, url, region, status, capacity, connected_clients, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(url)
        .bind(region)
        .bind(created_at)
        .bind(created_at)
        .fetch_one(self.store.get_pool())
        .await?;
        
        Ok(relay)
    }

    pub async fn list_dirnodes(&self) -> Result<Vec<Dirnode>> {
        let dirnodes = sqlx::query_as::<_, Dirnode>(
            "SELECT * FROM dirnodes ORDER BY created_at DESC"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(dirnodes)
    }

    pub async fn add_dirnode(&self, url: &str, region: Option<&str>, public_key: Option<&str>) -> Result<Dirnode> {
         let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();
        
        let dirnode = sqlx::query_as::<_, Dirnode>(
            r#"
            INSERT INTO dirnodes (id, url, region, public_key, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id, url, public_key, status, region, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(url)
        .bind(region)
        .bind(public_key)
        .bind(created_at)
        .bind(created_at)
        .fetch_one(self.store.get_pool())
        .await?;
        
        Ok(dirnode)       
    }
}
