use crate::db::store::DbStore;
use crate::db::schema::AuditLog;
use anyhow::Result;
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone)]
pub struct AuditService {
    store: DbStore,
}

impl AuditService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn log_event(
        &self,
        user_id: Option<&str>,
        action: &str,
        target: &str,
        details: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, target, details, ip_address, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id)
        .bind(user_id)
        .bind(action)
        .bind(target)
        .bind(details)
        .bind(ip_address)
        .bind(Utc::now())
        .execute(self.store.get_pool())
        .await?;
        
        Ok(())
    }

    pub async fn query_logs(&self) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 100"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(logs)
    }
    pub async fn export_logs_csv(&self) -> Result<String> {
        let logs = self.query_logs().await?;
        let mut wtr = String::new();
        wtr.push_str("id,user_id,action,target,details,ip_address,created_at\n");
        
        for log in logs {
            wtr.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                log.id,
                log.user_id.unwrap_or_default(),
                log.action,
                log.target,
                log.details.unwrap_or_default().replace(",", ";").replace("\n", " "), // Basic CSV escaping
                log.ip_address.unwrap_or_default(),
                log.created_at
            ));
        }
        Ok(wtr)
    }

    pub async fn cleanup_old_logs(&self, retention_days: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM audit_logs WHERE created_at < date('now', '-' || ? || ' days')")
            .bind(retention_days)
            .execute(self.store.get_pool())
            .await?;
        Ok(result.rows_affected())
    }
}
