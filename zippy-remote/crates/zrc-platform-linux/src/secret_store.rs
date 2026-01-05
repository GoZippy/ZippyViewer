#![cfg(target_os = "linux")]

use zeroize::{Zeroize, ZeroizeOnDrop};
use thiserror::Error;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("Secret Service error: {0}")]
    SecretService(String),
    #[error("Key not found")]
    NotFound,
    #[error("Keyring locked")]
    KeyringLocked,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
}

#[derive(ZeroizeOnDrop)]
pub struct KeyData {
    data: Vec<u8>,
}

impl KeyData {
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Secret Service-based storage
#[cfg(feature = "secret-service")]
pub struct SecretStore {
    service: secret_service::SecretService,
    collection: secret_service::Collection<'static>,
}

#[cfg(feature = "secret-service")]
impl SecretStore {
    /// Create secret store
    pub async fn new(collection_name: String) -> Result<Self, SecretStoreError> {
        use secret_service::{EncryptionType, SecretService};

        let service = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Connection failed: {}", e)))?;

        let collection = service
            .get_default_collection()
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Get collection failed: {}", e)))?;

        // Unlock if needed
        if collection.is_locked().await
            .map_err(|e| SecretStoreError::SecretService(format!("Lock check failed: {}", e)))?
        {
            collection
                .unlock()
                .await
                .map_err(|e| SecretStoreError::KeyringLocked)?;
        }

        Ok(Self {
            service,
            collection,
        })
    }

    /// Store key in Secret Service
    pub async fn store_key(&self, key_id: &str, data: &[u8]) -> Result<(), SecretStoreError> {
        use secret_service::Secret;

        let attributes = HashMap::from([
            ("application".to_string(), "zrc-agent".to_string()),
            ("key-name".to_string(), key_id.to_string()),
        ]);

        let secret = Secret::new(
            &self.collection,
            &format!("ZRC Key: {}", key_id),
            &attributes,
            data,
            "application/octet-stream",
        );

        self.collection
            .create_item(&secret, true, "plain")
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Create item failed: {}", e)))?;

        Ok(())
    }

    /// Load key from Secret Service
    pub async fn load_key(&self, key_id: &str) -> Result<KeyData, SecretStoreError> {
        use secret_service::SearchItems;

        let attributes = HashMap::from([
            ("application".to_string(), "zrc-agent".to_string()),
            ("key-name".to_string(), key_id.to_string()),
        ]);

        let items = self
            .collection
            .search_items(&attributes)
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Search failed: {}", e)))?;

        if items.is_empty() {
            return Err(SecretStoreError::NotFound);
        }

        let item = &items[0];
        let secret = item
            .get_secret()
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Get secret failed: {}", e)))?;

        Ok(KeyData {
            data: secret.to_vec(),
        })
    }

    /// Delete key
    pub async fn delete_key(&self, key_id: &str) -> Result<(), SecretStoreError> {
        use secret_service::SearchItems;

        let attributes = HashMap::from([
            ("application".to_string(), "zrc-agent".to_string()),
            ("key-name".to_string(), key_id.to_string()),
        ]);

        let items = self
            .collection
            .search_items(&attributes)
            .await
            .map_err(|e| SecretStoreError::SecretService(format!("Search failed: {}", e)))?;

        for item in items {
            item.delete()
                .await
                .map_err(|e| SecretStoreError::SecretService(format!("Delete failed: {}", e)))?;
        }

        Ok(())
    }
}

/// File-based key store fallback
pub struct FileKeyStore {
    key_dir: PathBuf,
}

impl FileKeyStore {
    /// Create file key store
    pub fn new(key_dir: PathBuf) -> Self {
        // Ensure directory exists
        let _ = std::fs::create_dir_all(&key_dir);
        Self { key_dir }
    }

    /// Store key in encrypted file
    pub fn store_key(&self, key_id: &str, data: &[u8]) -> Result<(), SecretStoreError> {
        let key_path = self.key_dir.join(format!("{}.key", key_id));
        
        // For now, store unencrypted (TODO: add encryption)
        // In production, you'd use a proper encryption library like aes-gcm
        std::fs::write(key_path, data)?;
        Ok(())
    }

    /// Load key from file
    pub fn load_key(&self, key_id: &str) -> Result<KeyData, SecretStoreError> {
        let key_path = self.key_dir.join(format!("{}.key", key_id));
        
        if !key_path.exists() {
            return Err(SecretStoreError::NotFound);
        }

        let data = std::fs::read(key_path)?;
        Ok(KeyData { data })
    }

    /// Delete key file
    pub fn delete_key(&self, key_id: &str) -> Result<(), SecretStoreError> {
        let key_path = self.key_dir.join(format!("{}.key", key_id));
        
        if key_path.exists() {
            std::fs::remove_file(key_path)?;
        }
        
        Ok(())
    }

    /// Zeroize key file
    pub fn zeroize_key(&self, key_id: &str) -> Result<(), SecretStoreError> {
        let key_path = self.key_dir.join(format!("{}.key", key_id));
        
        if key_path.exists() {
            // Overwrite with zeros before deleting
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .write(true)
                .open(&key_path)
            {
                if let Ok(metadata) = file.metadata() {
                    let size = metadata.len() as usize;
                    let zeros = vec![0u8; size];
                    let _ = std::io::Write::write_all(&mut file, &zeros);
                }
            }
            std::fs::remove_file(key_path)?;
        }
        
        Ok(())
    }
}
