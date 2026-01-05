use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use hex;

/// Device manager for paired devices
pub struct DeviceManager {
    devices: RwLock<HashMap<String, DeviceInfo>>,
    groups: RwLock<Vec<DeviceGroup>>,
    search_filter: RwLock<String>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            groups: RwLock::new(Vec::new()),
            search_filter: RwLock::new(String::new()),
        }
    }

    /// Load devices from pairings store
    /// This integrates with zrc-core's Store trait
    pub async fn load_from_store(&self, store: Arc<dyn zrc_core::store::Store>, operator_id: &[u8]) {
        if let Ok(pairings) = store.list_pairings().await {
            let mut devices = self.devices.write().unwrap();
            
            for p in pairings {
                // Filter for pairings where we are the operator
                if p.operator_id != operator_id { continue; }
                
                let id_hex = hex::encode(&p.device_id);
                
                // Convert u64 timestamps to SystemTime
                let paired_at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(p.issued_at);
                let last_seen_time = p.last_session.map(|t| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t));
                let status = if let Some(ls) = last_seen_time {
                     DeviceStatus::Offline { last_seen: ls }
                } else {
                     DeviceStatus::Unknown
                };

                devices.insert(
                    id_hex.clone(),
                    DeviceInfo {
                        id: id_hex.clone(),
                        display_name: format!("Device {}", &id_hex[..8]), // Short ID as name by default
                        status,
                        permissions: Permissions {
                            view: true,  // Simplified mapping for MVP
                            control: true,
                            clipboard: true,
                            file_transfer: true,
                        },
                        paired_at,
                        last_seen: last_seen_time,
                        group_id: None,
                    },
                );
            }
        }
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        self.devices.read().unwrap().values().cloned().collect()
    }

    /// List devices filtered by search query
    pub fn search_devices(&self, query: &str) -> Vec<DeviceInfo> {
        let devices = self.devices.read().unwrap();
        let query_lower = query.to_lowercase();
        
        devices.values()
            .filter(|device| {
                device.display_name.to_lowercase().contains(&query_lower) ||
                device.id.to_lowercase().contains(&query_lower) ||
                hex::encode(&device.id).contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Get device by ID
    pub fn get_device(&self, id: &str) -> Option<DeviceInfo> {
        self.devices.read().unwrap().get(id).cloned()
    }

    /// Update device display name
    pub fn set_device_name(&self, id: &str, name: String) -> Result<(), DeviceError> {
        let mut devices = self.devices.write().unwrap();
        if let Some(device) = devices.get_mut(id) {
            device.display_name = name;
            Ok(())
        } else {
            Err(DeviceError::NotFound(id.to_string()))
        }
    }

    /// Move device to group
    pub fn move_to_group(&self, id: &str, group_id: Option<String>) -> Result<(), DeviceError> {
        let mut devices = self.devices.write().unwrap();
        if let Some(device) = devices.get_mut(id) {
            device.group_id = group_id;
            Ok(())
        } else {
            Err(DeviceError::NotFound(id.to_string()))
        }
    }

    /// Remove device (revoke pairing)
    pub fn remove_device(&self, id: &str) -> Result<(), DeviceError> {
        let mut devices = self.devices.write().unwrap();
        devices.remove(id)
            .ok_or_else(|| DeviceError::NotFound(id.to_string()))?;
        Ok(())
    }

    /// Update device status
    pub fn update_status(&self, id: &str, status: DeviceStatus) {
        let mut devices = self.devices.write().unwrap();
        if let Some(device) = devices.get_mut(id) {
            device.status = status;
            device.last_seen = Some(SystemTime::now());
        }
    }

    /// Get devices by group
    pub fn list_by_group(&self, group_id: &str) -> Vec<DeviceInfo> {
        self.devices.read().unwrap()
            .values()
            .filter(|d| d.group_id.as_ref().map(|g| g == group_id).unwrap_or(false))
            .cloned()
            .collect()
    }

    /// Add or update device
    pub fn add_device(&self, device: DeviceInfo) {
        let mut devices = self.devices.write().unwrap();
        devices.insert(device.id.clone(), device);
    }

    /// Set search filter
    pub fn set_search_filter(&self, filter: String) {
        *self.search_filter.write().unwrap() = filter;
    }

    /// Get filtered devices (applies search filter)
    pub fn get_filtered_devices(&self) -> Vec<DeviceInfo> {
        let filter = self.search_filter.read().unwrap().clone();
        if filter.is_empty() {
            self.list_devices()
        } else {
            self.search_devices(&filter)
        }
    }
}

/// Device information
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub display_name: String,
    pub status: DeviceStatus,
    pub permissions: Permissions,
    pub paired_at: SystemTime,
    pub last_seen: Option<SystemTime>,
    pub group_id: Option<String>,
}

/// Device status
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceStatus {
    Online { latency_ms: u32 },
    Offline { last_seen: SystemTime },
    Connecting,
    Unknown,
}

/// Device permissions
#[derive(Clone, Debug, Default)]
pub struct Permissions {
    pub view: bool,
    pub control: bool,
    pub clipboard: bool,
    pub file_transfer: bool,
}

/// Device group
#[derive(Clone, Debug)]
pub struct DeviceGroup {
    pub id: String,
    pub name: String,
    pub color: egui::Color32,
    pub expanded: bool,
}

/// Device manager errors
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    
    #[error("Invalid device ID")]
    InvalidId,
}
