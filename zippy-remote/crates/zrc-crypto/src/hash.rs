use sha2::{Digest, Sha256};

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

/// Recommended identity derivation: id = sha256(pubkey_bytes).
pub fn derive_id(pubkey_bytes: &[u8]) -> [u8; 32] {
    sha256(pubkey_bytes)
}

