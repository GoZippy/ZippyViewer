//! High Availability support for relay server

use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::allocation::{AllocationManager, AllocationInfo};

#[derive(Debug, Error)]
pub enum HAError {
    #[error("Redis connection error: {0}")]
    Redis(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("State sync error: {0}")]
    StateSync(String),
}

/// High Availability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HAConfig {
    /// Instance ID (unique per instance)
    pub instance_id: String,
    /// Geographic region identifier
    pub region: Option<String>,
    /// Redis connection string (optional)
    pub redis_url: Option<String>,
    /// State sync interval in seconds
    pub state_sync_interval_secs: u64,
    /// Enable state sharing
    pub enable_state_sharing: bool,
}

impl Default for HAConfig {
    fn default() -> Self {
        Self {
            instance_id: format!("relay-{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            region: None,
            redis_url: None,
            state_sync_interval_secs: 30,
            enable_state_sharing: false,
        }
    }
}

/// State store trait for HA support
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    /// Save allocation state
    async fn save_allocation(&self, instance_id: &str, allocation: &AllocationInfo) -> Result<(), HAError>;
    
    /// Load all allocations for an instance
    async fn load_allocations(&self, instance_id: &str) -> Result<Vec<AllocationInfo>, HAError>;
    
    /// Remove allocation state
    async fn remove_allocation(&self, instance_id: &str, allocation_id: &[u8; 16]) -> Result<(), HAError>;
    
    /// List all active instances
    async fn list_instances(&self) -> Result<Vec<String>, HAError>;
    
    /// Register instance heartbeat
    async fn heartbeat(&self, instance_id: &str, region: Option<&str>) -> Result<(), HAError>;
}

/// In-memory state store (for single-instance or testing)
pub struct MemoryStateStore {
    allocations: Arc<dashmap::DashMap<String, Vec<AllocationInfo>>>,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(dashmap::DashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl StateStore for MemoryStateStore {
    async fn save_allocation(&self, instance_id: &str, allocation: &AllocationInfo) -> Result<(), HAError> {
        let mut allocs = self.allocations.entry(instance_id.to_string())
            .or_insert_with(Vec::new);
        // Update or insert allocation
        if let Some(pos) = allocs.iter().position(|a| a.id == allocation.id) {
            allocs[pos] = allocation.clone();
        } else {
            allocs.push(allocation.clone());
        }
        Ok(())
    }

    async fn load_allocations(&self, instance_id: &str) -> Result<Vec<AllocationInfo>, HAError> {
        Ok(self.allocations
            .get(instance_id)
            .map(|entry| entry.value().clone())
            .unwrap_or_default())
    }

    async fn remove_allocation(&self, instance_id: &str, allocation_id: &[u8; 16]) -> Result<(), HAError> {
        if let Some(mut allocs) = self.allocations.get_mut(instance_id) {
            allocs.retain(|a| a.id != *allocation_id);
        }
        Ok(())
    }

    async fn list_instances(&self) -> Result<Vec<String>, HAError> {
        Ok(self.allocations.iter().map(|entry| entry.key().clone()).collect())
    }

    async fn heartbeat(&self, _instance_id: &str, _region: Option<&str>) -> Result<(), HAError> {
        // No-op for memory store
        Ok(())
    }
}

/// Redis state store (optional, feature-gated)
#[cfg(feature = "redis")]
pub struct RedisStateStore {
    client: redis::Client,
    instance_id: String,
}

#[cfg(feature = "redis")]
impl RedisStateStore {
    pub async fn new(redis_url: &str, instance_id: String) -> Result<Self, HAError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| HAError::Redis(e.to_string()))?;
        Ok(Self { client, instance_id })
    }
}

#[cfg(feature = "redis")]
#[async_trait::async_trait]
impl StateStore for RedisStateStore {
    async fn save_allocation(&self, instance_id: &str, allocation: &AllocationInfo) -> Result<(), HAError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let key = format!("zrc:relay:{}:allocations", instance_id);
        let allocation_json = serde_json::to_string(allocation)
            .map_err(|e| HAError::Serialization(e.to_string()))?;
        
        redis::cmd("HSET")
            .arg(&key)
            .arg(hex::encode(allocation.id))
            .arg(&allocation_json)
            .query_async(&mut conn)
            .await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        Ok(())
    }

    async fn load_allocations(&self, instance_id: &str) -> Result<Vec<AllocationInfo>, HAError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let key = format!("zrc:relay:{}:allocations", instance_id);
        let allocations: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let mut result = Vec::new();
        for (_, json_str) in allocations {
            let allocation: AllocationInfo = serde_json::from_str(&json_str)
                .map_err(|e| HAError::Serialization(e.to_string()))?;
            result.push(allocation);
        }
        
        Ok(result)
    }

    async fn remove_allocation(&self, instance_id: &str, allocation_id: &[u8; 16]) -> Result<(), HAError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let key = format!("zrc:relay:{}:allocations", instance_id);
        redis::cmd("HDEL")
            .arg(&key)
            .arg(hex::encode(allocation_id))
            .query_async(&mut conn)
            .await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        Ok(())
    }

    async fn list_instances(&self) -> Result<Vec<String>, HAError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let pattern = "zrc:relay:*:allocations";
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let instances: Vec<String> = keys.iter()
            .filter_map(|key| {
                key.strip_prefix("zrc:relay:")?
                    .strip_suffix(":allocations")
                    .map(|s| s.to_string())
            })
            .collect();
        
        Ok(instances)
    }

    async fn heartbeat(&self, instance_id: &str, region: Option<&str>) -> Result<(), HAError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        let key = format!("zrc:relay:{}:heartbeat", instance_id);
        let value = serde_json::json!({
            "instance_id": instance_id,
            "region": region,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
        
        redis::cmd("SET")
            .arg(&key)
            .arg(serde_json::to_string(&value).unwrap())
            .arg("EX")
            .arg(60) // Expire after 60 seconds
            .query_async(&mut conn)
            .await
            .map_err(|e| HAError::Redis(e.to_string()))?;
        
        Ok(())
    }
}

/// High Availability manager
pub struct HAManager {
    config: HAConfig,
    state_store: Arc<dyn StateStore>,
    allocation_mgr: Arc<AllocationManager>,
}

impl HAManager {
    /// Create new HA manager (synchronous version for non-Redis)
    pub fn new(
        config: HAConfig,
        allocation_mgr: Arc<AllocationManager>,
    ) -> Result<Self, HAError> {
        let state_store: Arc<dyn StateStore> = if config.enable_state_sharing {
            #[cfg(feature = "redis")]
            {
                // For Redis, we'll need to initialize it asynchronously
                // This is a limitation - in practice, you'd want to use tokio::runtime::Handle
                Arc::new(MemoryStateStore::new()) // Fallback for now
            }
            #[cfg(not(feature = "redis"))]
            {
                Arc::new(MemoryStateStore::new())
            }
        } else {
            Arc::new(MemoryStateStore::new())
        };

        Ok(Self {
            config,
            state_store,
            allocation_mgr,
        })
    }

    /// Start state synchronization (async version)
    pub async fn start_sync_async(&self) -> Result<(), HAError> {
        if !self.config.enable_state_sharing {
            return Ok(());
        }

        let state_store = self.state_store.clone();
        let allocation_mgr = self.allocation_mgr.clone();
        let instance_id = self.config.instance_id.clone();
        let region = self.config.region.clone();
        let interval = Duration::from_secs(self.config.state_sync_interval_secs);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                
                // Send heartbeat
                if let Err(e) = state_store.heartbeat(&instance_id, region.as_deref()).await {
                    tracing::warn!("Heartbeat failed: {}", e);
                }

                // Sync allocations to state store
                let allocations = allocation_mgr.list();
                for allocation in allocations {
                    if let Err(e) = state_store.save_allocation(&instance_id, &allocation).await {
                        tracing::warn!("Failed to save allocation: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start state synchronization (synchronous wrapper)
    pub fn start_sync(&self) {
        let state_store = self.state_store.clone();
        let allocation_mgr = self.allocation_mgr.clone();
        let instance_id = self.config.instance_id.clone();
        let region = self.config.region.clone();
        let interval = Duration::from_secs(self.config.state_sync_interval_secs);
        let enable_sharing = self.config.enable_state_sharing;

        tokio::spawn(async move {
            if !enable_sharing {
                return;
            }

            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                
                // Send heartbeat
                if let Err(e) = state_store.heartbeat(&instance_id, region.as_deref()).await {
                    tracing::warn!("Heartbeat failed: {}", e);
                }

                // Sync allocations to state store
                let allocations = allocation_mgr.list();
                for allocation in allocations {
                    if let Err(e) = state_store.save_allocation(&instance_id, &allocation).await {
                        tracing::warn!("Failed to save allocation: {}", e);
                    }
                }
            }
        });
    }

    /// Get instance ID
    pub fn instance_id(&self) -> &str {
        &self.config.instance_id
    }

    /// Get region
    pub fn region(&self) -> Option<&str> {
        self.config.region.as_deref()
    }
}
