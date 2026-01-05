use crate::db::store::DbStore;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct DashboardStats {
    pub total_users: i64,
    pub total_devices: i64,
    pub active_pairings: i64,
    pub total_relays: i64,
}

#[derive(Serialize)]
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub uptime_seconds: u64,
    pub active_connections: u64,
}

#[derive(Clone)]
pub struct DashboardService {
    store: DbStore,
}

impl DashboardService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn get_stats(&self) -> Result<DashboardStats> {
        let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(self.store.get_pool())
            .await?;

        let total_devices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
            .fetch_one(self.store.get_pool())
            .await?;

        let active_pairings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pairings WHERE status = 'active'")
            .fetch_one(self.store.get_pool())
            .await?;
            
        let total_relays: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relays")
            .fetch_one(self.store.get_pool())
            .await?;

        Ok(DashboardStats {
            total_users,
            total_devices,
            active_pairings,
            total_relays,
        })
    }
    
    pub async fn get_metrics(&self) -> Result<SystemMetrics> {
        // In a real app, this would query system stats or a metrics crate.
        // For now, we stub it with mock values or basic DB health.
        
        // Mock values for "demo" purpose as requested by task 9.2
        Ok(SystemMetrics {
            cpu_usage: 12.5,
            memory_usage: 1024 * 1024 * 128, // 128MB
            uptime_seconds: 3600,
            active_connections: 42,
        })
    }
}
