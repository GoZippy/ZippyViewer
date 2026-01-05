#![cfg(windows)]
#![allow(unsafe_code)] // Windows API calls require unsafe.

use std::path::PathBuf;
use thiserror::Error;
use zeroize::ZeroizeOnDrop;
use windows::Win32::{
    Foundation::*,
    Security::Cryptography::*,
};

#[derive(Debug, Error)]
pub enum KeyStoreError {
    #[error("DPAPI encryption failed")]
    EncryptionFailed,
    #[error("DPAPI decryption failed")]
    DecryptionFailed,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("key not found")]
    NotFound,
}

pub enum DpapiScope {
    CurrentUser,
    LocalMachine,
}

/// DPAPI-based secure key storage
pub struct DpapiKeyStore {
    scope: DpapiScope,
    entropy: Option<Vec<u8>>,
    key_dir: PathBuf,
}

impl DpapiKeyStore {
    /// Create key store with specified scope
    pub fn new(scope: DpapiScope) -> Self {
        let key_dir = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zrc-keys");

        Self {
            scope,
            entropy: None,
            key_dir,
        }
    }

    /// Add optional entropy for additional protection
    pub fn with_entropy(mut self, entropy: &[u8]) -> Self {
        self.entropy = Some(entropy.to_vec());
        self
    }

    fn flags(&self) -> u32 {
        match self.scope {
            DpapiScope::CurrentUser => 0,
            DpapiScope::LocalMachine => CRYPTPROTECT_LOCAL_MACHINE,
        }
    }

    fn key_path(&self, name: &str) -> PathBuf {
        self.key_dir.join(format!("{}.key", name))
    }

    /// Store key with DPAPI encryption
    pub fn store_key(&self, name: &str, key: &[u8]) -> Result<(), KeyStoreError> {
        unsafe {
            // Create key directory if it doesn't exist
            std::fs::create_dir_all(&self.key_dir)?;

            let data_in = CRYPT_INTEGER_BLOB {
                cbData: key.len() as u32,
                pbData: key.as_ptr() as *mut u8,
            };

            let mut data_out = CRYPT_INTEGER_BLOB::default();

            let entropy_blob = self.entropy.as_ref().map(|ent| CRYPT_INTEGER_BLOB {
                cbData: ent.len() as u32,
                pbData: ent.as_ptr() as *mut u8,
            });

            let description = windows::core::PCWSTR::null();

            let result = CryptProtectData(
                &data_in,
                description,
                entropy_blob.as_ref().map(|e| e as *const _),
                None,
                None,
                self.flags(),
                &mut data_out,
            );

            if result.is_err() {
                return Err(KeyStoreError::EncryptionFailed);
            }

            // Write encrypted blob to file
            let path = self.key_path(name);
            let encrypted = std::slice::from_raw_parts(
                data_out.pbData,
                data_out.cbData as usize,
            );
            std::fs::write(path, encrypted)?;

            // Free the encrypted data
            let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut _)));

            Ok(())
        }
    }

    /// Load and decrypt key
    pub fn load_key(&self, name: &str) -> Result<ZeroizedKey, KeyStoreError> {
        unsafe {
            let path = self.key_path(name);
            if !path.exists() {
                return Err(KeyStoreError::NotFound);
            }

            let encrypted = std::fs::read(path)?;

            let data_in = CRYPT_INTEGER_BLOB {
                cbData: encrypted.len() as u32,
                pbData: encrypted.as_ptr() as *mut u8,
            };

            let mut data_out = CRYPT_INTEGER_BLOB::default();

            let entropy_blob = self.entropy.as_ref().map(|ent| CRYPT_INTEGER_BLOB {
                cbData: ent.len() as u32,
                pbData: ent.as_ptr() as *mut u8,
            });

            let result = CryptUnprotectData(
                &data_in,
                None,
                entropy_blob.as_ref().map(|e| e as *const _),
                None,
                None,
                0,
                &mut data_out,
            );

            if result.is_err() {
                return Err(KeyStoreError::DecryptionFailed);
            }

            let decrypted = std::slice::from_raw_parts(
                data_out.pbData,
                data_out.cbData as usize,
            )
            .to_vec();

            // Free the decrypted data
            let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut _)));

            Ok(ZeroizedKey(decrypted))
        }
    }

    /// Delete key
    pub fn delete_key(&self, name: &str) -> Result<(), KeyStoreError> {
        let path = self.key_path(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Check if key exists
    pub fn key_exists(&self, name: &str) -> bool {
        self.key_path(name).exists()
    }
}

/// Zeroized key wrapper for secure memory handling
#[derive(ZeroizeOnDrop)]
pub struct ZeroizedKey(Vec<u8>);

impl ZeroizedKey {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
