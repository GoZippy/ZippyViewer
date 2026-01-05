use crate::db::store::DbStore;
use crate::db::schema::Device;
use anyhow::Result;

#[derive(Clone)]
pub struct DeviceService {
    store: DbStore,
}

impl DeviceService {
    pub fn new(store: DbStore) -> Self {
        Self { store }
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        let devices = sqlx::query_as::<_, Device>(
            "SELECT * FROM devices ORDER BY last_seen DESC"
        )
        .fetch_all(self.store.get_pool())
        .await?;
        Ok(devices)
    }

    pub async fn get_device(&self, id: &str) -> Result<Option<Device>> {
        let device = sqlx::query_as::<_, Device>(
            "SELECT * FROM devices WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.store.get_pool())
        .await?;
        Ok(device)
    }
    
    // TODO: Add device registration logic when we define how they connect
    pub async fn delete_device(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM devices WHERE id = ?")
            .bind(id)
            .execute(self.store.get_pool())
            .await?;
        // Also revoke pairings? Assuming Cascade delete or manual cleanup?
        // SQLite FKs usually handle cascade if enabled, but let's be safe:
        sqlx::query("UPDATE pairings SET status = 'revoked' WHERE device_id = ?")
            .bind(id)
            .execute(self.store.get_pool())
            .await?;
            
        Ok(())
    }

    pub async fn update_device(&self, id: &str, group_name: Option<String>, tags: Option<Vec<String>>, channel_id: Option<String>) -> Result<()> {
        let tags_json = tags.and_then(|t| serde_json::to_string(&t).ok());
        
        // We use COALESCE to only update fields that are provided (handled by Option in Rust, but SQL needs logic)
        // Actually, typical PATCH behavior is: if provided (Some), update. If None, ignore.
        // It's cleaner to build the query dynamically or just always update if the struct implies replacement.
        // But here we are passing Options. Let's do a simple 3 query update or a smart query.
        // For SQLite: UPDATE devices SET group_name = COALESCE(?, group_name), ... works if we bind NULLs.
        // But binding Option<T> as NULL in SQLx works.
        // Wait, if I want to set it to NULL (erase), I need a way to distinguish "erase" from "don't change".
        // For simplicity in this task, let's assume "Some" means update, "None" means no change.
        // To strictly follow that, we'd need a dynamic query builder or separate queries.
        // Let's use a query builder pattern for simplicity and correctness.
        
        let mut query = "UPDATE devices SET ".to_string();
        let mut updates = Vec::new();
        
        if group_name.is_some() { updates.push("group_name = ?"); }
        if tags_json.is_some() { updates.push("tags = ?"); }
        if channel_id.is_some() { updates.push("channel_id = ?"); }
        
        if updates.is_empty() { return Ok(()); }
        
        query.push_str(&updates.join(", "));
        query.push_str(" WHERE id = ?");
        
        let mut q = sqlx::query(&query);
        if let Some(g) = group_name { q = q.bind(g); }
        if let Some(t) = tags_json { q = q.bind(t); }
        if let Some(c) = channel_id { q = q.bind(c); }
        q = q.bind(id);
        
        q.execute(self.store.get_pool()).await?;
        
        Ok(())
    }
}
