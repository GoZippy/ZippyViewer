use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use anyhow::Result;
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct DbStore {
    pool: SqlitePool,
}

impl DbStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Ensure directory exists if it's a file path
        if let Some(path_str) = database_url.strip_prefix("sqlite://") {
             if let Some(path) = Path::new(path_str).parent() {
                 if !path.exists() {
                     std::fs::create_dir_all(path)?;
                 }
             }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        info!("Database migrations applied successfully");
        Ok(())
    }

    pub fn get_pool(&self) -> &SqlitePool {
        &self.pool
    }
}
