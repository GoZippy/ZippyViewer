use std::sync::Arc;
use zrc_crypto::identity::Identity;
use zrc_crypto::cert_binding::{sign_cert_fingerprint, verify_cert_binding, CertBinding, CertBindingError};
use zrc_crypto::hash::sha256;
use zrc_proto::v1::PublicKeyBundleV1;
use async_trait::async_trait;
use thiserror::Error;
use tracing::{error, info, warn};

#[cfg(windows)]
use zrc_platform_win::keystore::DpapiKeyStore;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("key generation failed: {0}")]
    KeyGenerationFailed(String),
    #[error("key storage failed: {0}")]
    KeyStorageFailed(String),
    #[error("key loading failed: {0}")]
    KeyLoadingFailed(String),
    #[error("device ID derivation failed")]
    DeviceIdDerivationFailed,
    #[error("cert binding error: {0}")]
    CertBinding(#[from] CertBindingError),
}

#[async_trait]
pub trait KeyStore: Send + Sync {
    async fn store_key(&self, key_id: &str, key_data: &[u8]) -> Result<(), IdentityError>;
    async fn load_key(&self, key_id: &str) -> Result<Vec<u8>, IdentityError>;
    async fn key_exists(&self, key_id: &str) -> bool;
}

#[cfg(windows)]
#[async_trait]
impl KeyStore for DpapiKeyStore {
    async fn store_key(&self, key_id: &str, key_data: &[u8]) -> Result<(), IdentityError> {
        DpapiKeyStore::store_key(self, key_id, key_data)
            .map_err(|e| IdentityError::KeyStorageFailed(e.to_string()))
    }

    async fn load_key(&self, key_id: &str) -> Result<Vec<u8>, IdentityError> {
        let zeroized_key = DpapiKeyStore::load_key(self, key_id)
            .map_err(|e| IdentityError::KeyLoadingFailed(e.to_string()))?;
        Ok(zeroized_key.as_bytes().to_vec())
    }

    async fn key_exists(&self, key_id: &str) -> bool {
        DpapiKeyStore::key_exists(self, key_id)
    }
}

pub struct IdentityManager {
    identity: Arc<Identity>,
    device_id: [u8; 32],
    keystore: Arc<dyn KeyStore>,
}

impl IdentityManager {
    pub async fn new(keystore: Arc<dyn KeyStore>) -> Result<Self, IdentityError> {
        const SIGN_KEY_ID: &str = "zrc_identity_sign_key";
        const KEX_KEY_ID: &str = "zrc_identity_kex_key";

        let identity = if keystore.key_exists(SIGN_KEY_ID).await && keystore.key_exists(KEX_KEY_ID).await {
            // Load existing keys
            info!("Loading existing identity keys");
            let sign_key_bytes = keystore.load_key(SIGN_KEY_ID).await?;
            let kex_key_bytes = keystore.load_key(KEX_KEY_ID).await?;
            
            if sign_key_bytes.len() != 32 || kex_key_bytes.len() != 32 {
                return Err(IdentityError::KeyLoadingFailed("Invalid key length".to_string()));
            }
            
            let sign_key_array: [u8; 32] = sign_key_bytes.try_into()
                .map_err(|_| IdentityError::KeyLoadingFailed("Failed to convert sign key".to_string()))?;
            let kex_key_array: [u8; 32] = kex_key_bytes.try_into()
                .map_err(|_| IdentityError::KeyLoadingFailed("Failed to convert kex key".to_string()))?;
            
            Identity::from_bytes(&sign_key_array, &kex_key_array)
                .map_err(|e| IdentityError::KeyLoadingFailed(e.to_string()))?
        } else {
            // Generate new identity
            info!("Generating new identity keys");
            let identity = Identity::generate();
            
            // Store keys - extract bytes from signing key and kex secret
            let sign_key_bytes = identity.sign_key().to_bytes();
            let kex_key_bytes = *identity.kex_secret().as_bytes();
            
            keystore.store_key(SIGN_KEY_ID, &sign_key_bytes).await?;
            keystore.store_key(KEX_KEY_ID, &kex_key_bytes).await?;
            
            identity
        };

        let sign_pub = identity.sign_pub();
        let device_id = sha256(&sign_pub);

        Ok(Self {
            identity: Arc::new(identity),
            device_id,
            keystore,
        })
    }

    pub fn identity(&self) -> Arc<Identity> {
        self.identity.clone()
    }

    pub fn device_id(&self) -> &[u8; 32] {
        &self.device_id
    }

    pub fn public_bundle(&self) -> PublicKeyBundleV1 {
        self.identity.public_bundle()
    }

    /// Generate a DTLS certificate binding.
    /// This signs the DTLS certificate fingerprint with the device's Ed25519 identity key.
    pub fn bind_dtls_cert(&self, dtls_fingerprint: &[u8; 32]) -> CertBinding {
        sign_cert_fingerprint(&self.identity, dtls_fingerprint)
    }

    /// Verify a peer's DTLS certificate binding against a pinned identity.
    pub fn verify_peer_cert_binding(
        &self,
        binding: &CertBinding,
        expected_pub: &[u8; 32],
    ) -> Result<(), IdentityError> {
        verify_cert_binding(binding, expected_pub)?;
        Ok(())
    }
}
