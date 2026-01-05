//! Identity management for operator cryptographic identity
//!
//! This module handles:
//! - Ed25519 signing key generation and management
//! - X25519 key exchange key generation and management
//! - Secure key storage (OS keystore or file-based)
//! - Identity persistence across restarts

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::IdentityConfig;

/// Identity management errors
#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("Failed to generate keypair: {0}")]
    KeyGeneration(String),

    #[error("Failed to load identity: {0}")]
    Load(String),

    #[error("Failed to save identity: {0}")]
    Save(String),

    #[error("Key store error: {0}")]
    KeyStore(String),

    #[error("Identity not found")]
    NotFound,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid key data: {0}")]
    InvalidKeyData(String),
}

/// Operator identity information for display
#[derive(Debug, Clone, Serialize)]
pub struct IdentityInfo {
    /// Operator ID (derived from public key)
    pub operator_id: String,
    /// Public key fingerprint (full hex of signing public key)
    pub fingerprint: String,
    /// When the identity was created
    pub created_at: SystemTime,
    /// Key algorithm used
    pub key_algorithm: String,
}

/// Public identity export (safe to share)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityExport {
    /// Operator ID
    pub operator_id: String,
    /// Ed25519 signing public key (hex encoded)
    pub sign_pub: String,
    /// X25519 key exchange public key (hex encoded)
    pub kex_pub: String,
    /// When the identity was created (RFC3339)
    pub created_at: String,
}

/// Serializable identity data for file storage
#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    /// Version for future compatibility
    version: u32,
    /// Ed25519 signing private key seed (32 bytes, hex encoded)
    sign_seed: String,
    /// X25519 key exchange private key (32 bytes, hex encoded)
    kex_secret: String,
    /// When the identity was created (RFC3339)
    created_at: String,
}


impl StoredIdentity {
    const CURRENT_VERSION: u32 = 1;

    fn new(sign_seed: &[u8; 32], kex_secret: &[u8; 32], created_at: SystemTime) -> Self {
        let datetime: chrono::DateTime<chrono::Utc> = created_at.into();
        Self {
            version: Self::CURRENT_VERSION,
            sign_seed: hex::encode(sign_seed),
            kex_secret: hex::encode(kex_secret),
            created_at: datetime.to_rfc3339(),
        }
    }

    fn parse_created_at(&self) -> Result<SystemTime, IdentityError> {
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc).into())
            .map_err(|e| IdentityError::Serialization(format!("Invalid timestamp: {e}")))
    }
}

/// Key storage backend trait (internal)
trait KeyStore: Send + Sync {
    /// Store identity keys
    fn store(&self, identity: &StoredIdentity) -> Result<(), IdentityError>;
    /// Load identity keys
    fn load(&self) -> Result<Option<StoredIdentity>, IdentityError>;
    /// Delete identity keys
    #[allow(dead_code)]
    fn delete(&self) -> Result<(), IdentityError>;
    /// Check if identity exists
    fn exists(&self) -> bool;
}

/// File-based key storage (internal)
struct FileKeyStore {
    path: PathBuf,
}

impl FileKeyStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Get default identity file path
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("io", "zippyremote", "zrc")
            .map(|dirs| dirs.data_dir().join("identity.json"))
    }
}

impl KeyStore for FileKeyStore {
    fn store(&self, identity: &StoredIdentity) -> Result<(), IdentityError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to JSON
        let json = serde_json::to_string_pretty(identity)
            .map_err(|e| IdentityError::Serialization(e.to_string()))?;

        // Write atomically using a temp file
        let temp_path = self.path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);

        // Rename to final path
        fs::rename(&temp_path, &self.path)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.path, perms)?;
        }

        Ok(())
    }

    fn load(&self) -> Result<Option<StoredIdentity>, IdentityError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let mut file = fs::File::open(&self.path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let stored: StoredIdentity = serde_json::from_str(&contents)
            .map_err(|e| IdentityError::Serialization(e.to_string()))?;

        Ok(Some(stored))
    }

    fn delete(&self) -> Result<(), IdentityError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn exists(&self) -> bool {
        self.path.exists()
    }
}


/// OS keystore-based key storage (with file fallback)
#[cfg(target_os = "windows")]
struct OsKeyStore {
    #[allow(dead_code)]
    service_name: String,
    fallback: FileKeyStore,
}

#[cfg(target_os = "windows")]
impl OsKeyStore {
    pub fn new(fallback_path: PathBuf) -> Self {
        Self {
            service_name: "zrc-controller".to_string(),
            fallback: FileKeyStore::new(fallback_path),
        }
    }

    fn try_store_credential(&self, identity: &StoredIdentity) -> Result<(), IdentityError> {
        // On Windows, we use the Credential Manager via the windows crate
        // For now, fall back to file storage
        // TODO: Implement Windows Credential Manager integration
        self.fallback.store(identity)
    }

    fn try_load_credential(&self) -> Result<Option<StoredIdentity>, IdentityError> {
        // TODO: Implement Windows Credential Manager integration
        self.fallback.load()
    }
}

#[cfg(target_os = "windows")]
impl KeyStore for OsKeyStore {
    fn store(&self, identity: &StoredIdentity) -> Result<(), IdentityError> {
        self.try_store_credential(identity)
    }

    fn load(&self) -> Result<Option<StoredIdentity>, IdentityError> {
        self.try_load_credential()
    }

    fn delete(&self) -> Result<(), IdentityError> {
        self.fallback.delete()
    }

    fn exists(&self) -> bool {
        self.fallback.exists()
    }
}

/// OS keystore-based key storage for non-Windows platforms
#[cfg(not(target_os = "windows"))]
struct OsKeyStore {
    fallback: FileKeyStore,
}

#[cfg(not(target_os = "windows"))]
impl OsKeyStore {
    pub fn new(fallback_path: PathBuf) -> Self {
        Self {
            fallback: FileKeyStore::new(fallback_path),
        }
    }
}

#[cfg(not(target_os = "windows"))]
impl KeyStore for OsKeyStore {
    fn store(&self, identity: &StoredIdentity) -> Result<(), IdentityError> {
        // On non-Windows, use file storage with restrictive permissions
        self.fallback.store(identity)
    }

    fn load(&self) -> Result<Option<StoredIdentity>, IdentityError> {
        self.fallback.load()
    }

    fn delete(&self) -> Result<(), IdentityError> {
        self.fallback.delete()
    }

    fn exists(&self) -> bool {
        self.fallback.exists()
    }
}


/// Manages operator cryptographic identity
pub struct IdentityManager {
    /// Operator ID (derived from signing public key)
    operator_id: String,
    /// Ed25519 signing keypair
    signing_key: ed25519_dalek::SigningKey,
    /// X25519 key exchange secret
    kex_secret: x25519_dalek::StaticSecret,
    /// When the identity was created
    created_at: SystemTime,
    /// Key storage backend
    key_store: Box<dyn KeyStore>,
}

impl IdentityManager {
    /// Initialize or load existing identity
    ///
    /// If an identity exists in the configured key store, it will be loaded.
    /// Otherwise, a new identity will be generated and stored.
    pub async fn init(config: &IdentityConfig) -> Result<Self, IdentityError> {
        let key_store = Self::create_key_store(config);

        // Try to load existing identity
        if let Some(stored) = key_store.load()? {
            return Self::from_stored(stored, key_store);
        }

        // Generate new identity
        let identity = Self::generate_new(key_store)?;
        
        tracing::info!(
            operator_id = %identity.operator_id,
            "Generated new operator identity"
        );

        Ok(identity)
    }

    /// Create a new ephemeral identity (not persisted)
    /// Used for testing and temporary operations
    pub fn new_ephemeral() -> Self {
        let mut rng = rand_core::OsRng;
        
        // Generate Ed25519 signing key
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        
        // Generate X25519 key exchange key
        let kex_secret = x25519_dalek::StaticSecret::random_from_rng(&mut rng);
        
        let created_at = SystemTime::now();
        let operator_id = Self::compute_operator_id(&signing_key);

        // Use a dummy file key store that won't actually persist
        let key_store = Box::new(FileKeyStore::new(PathBuf::from("/dev/null")));

        Self {
            operator_id,
            signing_key,
            kex_secret,
            created_at,
            key_store,
        }
    }

    /// Create key store based on configuration
    fn create_key_store(config: &IdentityConfig) -> Box<dyn KeyStore> {
        let path = config.key_path.clone().unwrap_or_else(|| {
            FileKeyStore::default_path().unwrap_or_else(|| PathBuf::from("identity.json"))
        });

        match config.key_store.as_str() {
            "os" => Box::new(OsKeyStore::new(path)),
            "file" | _ => Box::new(FileKeyStore::new(path)),
        }
    }

    /// Generate a new identity
    fn generate_new(key_store: Box<dyn KeyStore>) -> Result<Self, IdentityError> {
        let mut rng = rand_core::OsRng;
        
        // Generate Ed25519 signing key
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        
        // Generate X25519 key exchange key
        let kex_secret = x25519_dalek::StaticSecret::random_from_rng(&mut rng);
        
        let created_at = SystemTime::now();
        let operator_id = Self::compute_operator_id(&signing_key);

        // Store the identity
        let stored = StoredIdentity::new(
            &signing_key.to_bytes(),
            kex_secret.as_bytes(),
            created_at,
        );
        key_store.store(&stored)?;

        Ok(Self {
            operator_id,
            signing_key,
            kex_secret,
            created_at,
            key_store,
        })
    }

    /// Load identity from stored data
    fn from_stored(stored: StoredIdentity, key_store: Box<dyn KeyStore>) -> Result<Self, IdentityError> {
        // Decode signing key seed
        let sign_seed_bytes = hex::decode(&stored.sign_seed)
            .map_err(|e| IdentityError::InvalidKeyData(format!("Invalid sign_seed hex: {e}")))?;
        
        if sign_seed_bytes.len() != 32 {
            return Err(IdentityError::InvalidKeyData(format!(
                "Invalid sign_seed length: expected 32, got {}",
                sign_seed_bytes.len()
            )));
        }
        
        let mut sign_seed = [0u8; 32];
        sign_seed.copy_from_slice(&sign_seed_bytes);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&sign_seed);

        // Decode key exchange secret
        let kex_bytes = hex::decode(&stored.kex_secret)
            .map_err(|e| IdentityError::InvalidKeyData(format!("Invalid kex_secret hex: {e}")))?;
        
        if kex_bytes.len() != 32 {
            return Err(IdentityError::InvalidKeyData(format!(
                "Invalid kex_secret length: expected 32, got {}",
                kex_bytes.len()
            )));
        }
        
        let mut kex_arr = [0u8; 32];
        kex_arr.copy_from_slice(&kex_bytes);
        let kex_secret = x25519_dalek::StaticSecret::from(kex_arr);

        let created_at = stored.parse_created_at()?;
        let operator_id = Self::compute_operator_id(&signing_key);

        tracing::debug!(
            operator_id = %operator_id,
            "Loaded existing operator identity"
        );

        Ok(Self {
            operator_id,
            signing_key,
            kex_secret,
            created_at,
            key_store,
        })
    }

    /// Compute operator ID from signing key
    /// 
    /// The operator ID is the first 16 hex characters (8 bytes) of the SHA-256 hash
    /// of the signing public key.
    fn compute_operator_id(signing_key: &ed25519_dalek::SigningKey) -> String {
        let sign_pub = signing_key.verifying_key().to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(sign_pub);
        let hash = hasher.finalize();
        hex::encode(&hash[..8])
    }

    /// Get operator ID
    pub fn operator_id(&self) -> &str {
        &self.operator_id
    }

    /// Get signing public key
    pub fn sign_pub(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Get key exchange public key
    pub fn kex_pub(&self) -> [u8; 32] {
        x25519_dalek::PublicKey::from(&self.kex_secret).to_bytes()
    }

    /// Sign data using Ed25519
    pub fn sign(&self, data: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.signing_key.sign(data).to_bytes()
    }

    /// Perform X25519 Diffie-Hellman key exchange
    pub fn key_exchange(&self, peer_kex_pub: &[u8; 32]) -> [u8; 32] {
        let peer_pub = x25519_dalek::PublicKey::from(*peer_kex_pub);
        self.kex_secret.diffie_hellman(&peer_pub).to_bytes()
    }

    /// Export identity (public info only)
    pub fn export_public(&self) -> IdentityExport {
        let datetime: chrono::DateTime<chrono::Utc> = self.created_at.into();
        IdentityExport {
            operator_id: self.operator_id.clone(),
            sign_pub: hex::encode(self.sign_pub()),
            kex_pub: hex::encode(self.kex_pub()),
            created_at: datetime.to_rfc3339(),
        }
    }

    /// Export identity to file
    pub fn export_to_file(&self, path: &Path) -> Result<(), IdentityError> {
        let export = self.export_public();
        let json = serde_json::to_string_pretty(&export)
            .map_err(|e| IdentityError::Serialization(e.to_string()))?;
        
        fs::write(path, json)?;
        Ok(())
    }

    /// Rotate identity (warning: breaks existing pairings)
    ///
    /// This generates a completely new identity, invalidating all existing
    /// pairings with devices.
    pub async fn rotate(&mut self) -> Result<(), IdentityError> {
        let mut rng = rand_core::OsRng;
        
        // Generate new keys
        self.signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        self.kex_secret = x25519_dalek::StaticSecret::random_from_rng(&mut rng);
        self.operator_id = Self::compute_operator_id(&self.signing_key);
        self.created_at = SystemTime::now();

        // Store the new identity
        let stored = StoredIdentity::new(
            &self.signing_key.to_bytes(),
            self.kex_secret.as_bytes(),
            self.created_at,
        );
        self.key_store.store(&stored)?;

        tracing::info!(
            operator_id = %self.operator_id,
            "Rotated operator identity"
        );

        Ok(())
    }

    /// Display identity info
    pub fn display_info(&self) -> IdentityInfo {
        IdentityInfo {
            operator_id: self.operator_id.clone(),
            fingerprint: hex::encode(self.sign_pub()),
            created_at: self.created_at,
            key_algorithm: "Ed25519/X25519".to_string(),
        }
    }

    /// Check if identity exists in storage
    pub fn identity_exists(config: &IdentityConfig) -> bool {
        let key_store = Self::create_key_store(config);
        key_store.exists()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> IdentityConfig {
        IdentityConfig {
            key_path: Some(temp_dir.path().join("identity.json")),
            key_store: "file".to_string(),
        }
    }

    #[tokio::test]
    async fn test_identity_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let identity = IdentityManager::init(&config).await.unwrap();

        // Operator ID should be 16 hex characters
        assert_eq!(identity.operator_id().len(), 16);
        
        // Keys should be 32 bytes
        assert_eq!(identity.sign_pub().len(), 32);
        assert_eq!(identity.kex_pub().len(), 32);
    }

    #[tokio::test]
    async fn test_identity_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        // Generate identity
        let identity1 = IdentityManager::init(&config).await.unwrap();
        let operator_id1 = identity1.operator_id().to_string();
        let sign_pub1 = identity1.sign_pub();
        let kex_pub1 = identity1.kex_pub();
        drop(identity1);

        // Load identity again
        let identity2 = IdentityManager::init(&config).await.unwrap();
        
        // Should be the same identity
        assert_eq!(identity2.operator_id(), operator_id1);
        assert_eq!(identity2.sign_pub(), sign_pub1);
        assert_eq!(identity2.kex_pub(), kex_pub1);
    }

    #[tokio::test]
    async fn test_identity_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let mut identity = IdentityManager::init(&config).await.unwrap();
        let old_operator_id = identity.operator_id().to_string();
        let old_sign_pub = identity.sign_pub();

        // Rotate identity
        identity.rotate().await.unwrap();

        // Should have new keys
        assert_ne!(identity.operator_id(), old_operator_id);
        assert_ne!(identity.sign_pub(), old_sign_pub);
    }

    #[tokio::test]
    async fn test_signature_verification() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let identity = IdentityManager::init(&config).await.unwrap();
        let message = b"Test message for signing";

        let signature = identity.sign(message);
        
        // Verify signature using ed25519_dalek
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let verifying_key = VerifyingKey::from_bytes(&identity.sign_pub()).unwrap();
        let sig = Signature::from_bytes(&signature);
        assert!(verifying_key.verify(message, &sig).is_ok());
    }

    #[tokio::test]
    async fn test_key_exchange() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        let config1 = test_config(&temp_dir1);
        let config2 = test_config(&temp_dir2);

        let identity1 = IdentityManager::init(&config1).await.unwrap();
        let identity2 = IdentityManager::init(&config2).await.unwrap();

        // Both parties should derive the same shared secret
        let shared1 = identity1.key_exchange(&identity2.kex_pub());
        let shared2 = identity2.key_exchange(&identity1.kex_pub());

        assert_eq!(shared1, shared2);
    }

    #[tokio::test]
    async fn test_export_public() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let identity = IdentityManager::init(&config).await.unwrap();
        let export = identity.export_public();

        assert_eq!(export.operator_id, identity.operator_id());
        assert_eq!(export.sign_pub, hex::encode(identity.sign_pub()));
        assert_eq!(export.kex_pub, hex::encode(identity.kex_pub()));
    }

    #[tokio::test]
    async fn test_export_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let identity = IdentityManager::init(&config).await.unwrap();
        let export_path = temp_dir.path().join("export.json");
        
        identity.export_to_file(&export_path).unwrap();

        // Read and verify export
        let contents = fs::read_to_string(&export_path).unwrap();
        let export: IdentityExport = serde_json::from_str(&contents).unwrap();
        
        assert_eq!(export.operator_id, identity.operator_id());
    }

    #[tokio::test]
    async fn test_display_info() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);

        let identity = IdentityManager::init(&config).await.unwrap();
        let info = identity.display_info();

        assert_eq!(info.operator_id, identity.operator_id());
        assert_eq!(info.fingerprint, hex::encode(identity.sign_pub()));
        assert_eq!(info.key_algorithm, "Ed25519/X25519");
    }

    #[test]
    fn test_file_key_store() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_identity.json");
        let store = FileKeyStore::new(path.clone());

        // Initially should not exist
        assert!(!store.exists());

        // Create test identity data
        let stored = StoredIdentity::new(
            &[1u8; 32],
            &[2u8; 32],
            SystemTime::now(),
        );

        // Store and verify
        store.store(&stored).unwrap();
        assert!(store.exists());

        // Load and verify
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.sign_seed, stored.sign_seed);
        assert_eq!(loaded.kex_secret, stored.kex_secret);

        // Delete and verify
        store.delete().unwrap();
        assert!(!store.exists());
    }
}
