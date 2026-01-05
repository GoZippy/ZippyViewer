use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use zrc_crypto::hash::derive_id;
use zrc_proto::v1::{KeyTypeV1, PublicKeyV1};

use crate::types::IdentityKeys;

/// Generate fresh identity keys. (OS keystore integration can wrap this later.)
pub fn generate_identity_keys() -> IdentityKeys {
    // Ed25519
    let sign = SigningKey::generate(&mut OsRng);
    let sign_pub_bytes = sign.verifying_key().to_bytes().to_vec();
    let id32 = derive_id(&sign_pub_bytes);

    // X25519
    let kex_priv = StaticSecret::random_from_rng(OsRng);
    let kex_pub_bytes = X25519PublicKey::from(&kex_priv).to_bytes().to_vec();

    IdentityKeys {
        sign,
        sign_pub: PublicKeyV1 {
            key_type: KeyTypeV1::Ed25519 as i32,
            key_bytes: sign_pub_bytes,
        },
        kex_priv,
        kex_pub: PublicKeyV1 {
            key_type: KeyTypeV1::X25519 as i32,
            key_bytes: kex_pub_bytes,
        },
        id32,
    }
}

