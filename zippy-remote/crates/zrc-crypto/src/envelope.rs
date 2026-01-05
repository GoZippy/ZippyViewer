//! Envelope module for sealed-box encryption.
//! Implements HPKE-style sealing using X25519 + HKDF + ChaCha20Poly1305.

use bytes::Bytes;
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Key, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};

use crate::hash::{derive_id, sha256};
use crate::transcript::Transcript;
use zrc_proto::v1::{EnvelopeHeaderV1, EnvelopeV1, MsgTypeV1};



#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("missing field: {0}")]
    Missing(&'static str),
    #[error("invalid key bytes")]
    InvalidKeyBytes,
    #[error("unsupported suite")]
    UnsupportedSuite,
    #[error("signature verification failed")]
    BadSignature,
    #[error("sender_id does not match sender_sign_pub")]
    SenderIdMismatch,
    #[error("decryption failed")]
    DecryptFailed,
    #[error("encryption failed")]
    EncryptFailed,
}

/// Deterministic AAD for EnvelopeV1.
/// This is included as AEAD AAD and also carried in envelope.aad.
pub fn envelope_aad_v1(header: &EnvelopeHeaderV1) -> Vec<u8> {
    let mut t = Transcript::new("zrc_env_aad_v1");
    t.append_bytes(1, &header.sender_id);
    t.append_bytes(2, &header.recipient_id);
    t.append_u64(3, header.timestamp);
    t.append_bytes(4, &header.nonce);
    t.as_bytes().to_vec()
}

fn kdf_key_nonce(shared_secret: &[u8; 32], salt: &[u8]) -> ([u8; 32], [u8; 12]) {
    // HKDF-SHA256(salt, shared_secret)
    let hk = Hkdf::<Sha256>::new(Some(salt), shared_secret);

    let mut key = [0u8; 32];
    hk.expand(b"zrc_env_v1_key", &mut key).unwrap(); // Output size matches digest size, infallible

    let mut nonce = [0u8; 12];
    hk.expand(b"zrc_env_v1_nonce", &mut nonce)
        .unwrap(); // Output size < digest size, infallible

    (key, nonce)
}

fn sign_envelope(
    sender_sign: &SigningKey,
    header: &EnvelopeHeaderV1,
    sender_kex_pub: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
) -> [u8; 64] {
    // Build canonical data to sign: header fields || kex_pub || aad || ciphertext
    let mut t = Transcript::new("zrc_env_sig_v1");
    t.append_u64(1, header.version as u64);
    t.append_u64(2, header.msg_type as u64);
    t.append_bytes(3, &header.sender_id);
    t.append_bytes(4, &header.recipient_id);
    t.append_u64(5, header.timestamp);
    t.append_bytes(6, &header.nonce);
    t.append_bytes(7, sender_kex_pub);
    t.append_bytes(8, aad);
    t.append_bytes(9, ciphertext);

    let sig_input = sha256(t.as_bytes());
    let sig: Signature = sender_sign.sign(&sig_input);
    sig.to_bytes()
}

fn verify_envelope_sig(
    header: &EnvelopeHeaderV1,
    sender_kex_pub: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
    signature: &[u8],
    sender_sign_pub: &[u8; 32],
) -> Result<(), EnvelopeError> {
    // Verify sender_id matches the public key
    let derived_id = derive_id(sender_sign_pub);
    if header.sender_id != derived_id {
        return Err(EnvelopeError::SenderIdMismatch);
    }

    // Build canonical data that was signed
    let mut t = Transcript::new("zrc_env_sig_v1");
    t.append_u64(1, header.version as u64);
    t.append_u64(2, header.msg_type as u64);
    t.append_bytes(3, &header.sender_id);
    t.append_bytes(4, &header.recipient_id);
    t.append_u64(5, header.timestamp);
    t.append_bytes(6, &header.nonce);
    t.append_bytes(7, sender_kex_pub);
    t.append_bytes(8, aad);
    t.append_bytes(9, ciphertext);

    let sig_input = sha256(t.as_bytes());

    let vk = VerifyingKey::from_bytes(sender_sign_pub)
        .map_err(|_| EnvelopeError::InvalidKeyBytes)?;

    let sig_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| EnvelopeError::InvalidKeyBytes)?;
    let s = Signature::from_bytes(&sig_bytes);

    vk.verify_strict(&sig_input, &s)
        .map_err(|_| EnvelopeError::BadSignature)
}

fn x25519_pub_from_bytes(b: &[u8]) -> Result<X25519PublicKey, EnvelopeError> {
    let arr: [u8; 32] = b.try_into().map_err(|_| EnvelopeError::InvalidKeyBytes)?;
    Ok(X25519PublicKey::from(arr))
}

/// Build an EnvelopeV1 by sealing `plaintext` to `recipient_kex_pub` (X25519) and signing with `sender_sign`.
///
/// Notes:
/// - Routing is untrusted; security comes from signature + E2EE.
/// - `header.sender_id` MUST equal sha256(sender_sign_pub).
/// - `sender_kex_pub` contains sender ephemeral X25519 public key bytes (32).
pub fn envelope_seal_v1(
    sender_sign: &SigningKey,
    sender_id: &[u8],        // 32 bytes
    recipient_id: &[u8],     // 32 bytes
    recipient_kex_pub: &[u8; 32], // X25519
    msg_type: MsgTypeV1,
    plaintext: &[u8],
    now_unix: u64,
) -> Result<EnvelopeV1, EnvelopeError> {
    // Generate random nonce for header
    let mut nonce24 = [0u8; 24];
    getrandom::getrandom(&mut nonce24).map_err(|_| EnvelopeError::EncryptFailed)?;

    // Header
    let header = EnvelopeHeaderV1 {
        version: 1,
        msg_type: msg_type.into(),
        sender_id: sender_id.to_vec(),
        recipient_id: recipient_id.to_vec(),
        timestamp: now_unix,
        nonce: nonce24.to_vec(),
    };

    // AAD
    let aad = envelope_aad_v1(&header);

    // "HPKE-like" sealed box using X25519 + HKDF + ChaCha20Poly1305
    let eph = EphemeralSecret::random_from_rng(OsRng);
    let eph_pub = X25519PublicKey::from(&eph);

    let recip_pub = x25519_pub_from_bytes(recipient_kex_pub)?;
    let shared = eph.diffie_hellman(&recip_pub);
    let shared_bytes: [u8; 32] = shared.to_bytes();

    // Use header nonce as HKDF salt to bind derived keys to this envelope
    let (key32, nonce12) = kdf_key_nonce(&shared_bytes, &nonce24);

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key32));
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce12),
            Payload { msg: plaintext, aad: &aad },
        )
        .map_err(|_| EnvelopeError::EncryptFailed)?;

    let sender_kex_pub = eph_pub.as_bytes().to_vec();
    let signature = sign_envelope(sender_sign, &header, &sender_kex_pub, &aad, &ct);

    Ok(EnvelopeV1 {
        header: Some(header),
        sender_kex_pub,
        encrypted_payload: ct,
        signature: signature.to_vec(),
        aad,
    })
}

/// Verify signature, then decrypt `EnvelopeV1` for the recipient with `recipient_kex_priv` (X25519).
///
/// Returns plaintext bytes and the verified sender_id.
pub fn envelope_open_v1(
    env: &EnvelopeV1,
    recipient_kex_priv: &StaticSecret,
    sender_sign_pub: &[u8; 32],
) -> Result<(Bytes, Vec<u8>), EnvelopeError> {
    let header = env
        .header
        .as_ref()
        .ok_or(EnvelopeError::Missing("env.header"))?;

    // Verify signature BEFORE decrypting
    verify_envelope_sig(
        header,
        &env.sender_kex_pub,
        &env.aad,
        &env.encrypted_payload,
        &env.signature,
        sender_sign_pub,
    )?;

    // Recompute AAD from header and require it matches env.aad (prevents tampering)
    let recomputed_aad = envelope_aad_v1(header);
    if recomputed_aad != env.aad {
        return Err(EnvelopeError::BadSignature);
    }

    // Derive shared secret using sender ephemeral pub from sender_kex_pub
    if env.sender_kex_pub.len() != 32 {
        return Err(EnvelopeError::InvalidKeyBytes);
    }
    let eph_pub = x25519_pub_from_bytes(&env.sender_kex_pub)?;
    let shared = recipient_kex_priv.diffie_hellman(&eph_pub);
    let shared_bytes: [u8; 32] = shared.to_bytes();

    let (key32, nonce12) = kdf_key_nonce(&shared_bytes, &header.nonce);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key32));

    let pt = cipher
        .decrypt(
            Nonce::from_slice(&nonce12),
            Payload {
                msg: &env.encrypted_payload,
                aad: &env.aad,
            },
        )
        .map_err(|_| EnvelopeError::DecryptFailed)?;

    Ok((Bytes::from(pt), header.sender_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_round_trip() {
        // Generate sender identity keys
        let sender_sign = SigningKey::generate(&mut OsRng);
        let sender_sign_pub = sender_sign.verifying_key().to_bytes();
        let sender_id = derive_id(&sender_sign_pub);

        // Generate recipient keys
        let recipient_kex_priv = StaticSecret::random_from_rng(OsRng);
        let recipient_kex_pub = X25519PublicKey::from(&recipient_kex_priv);
        let recipient_id = sha256(recipient_kex_pub.as_bytes());

        let plaintext = b"Hello, secure world!";
        let now = 1700000000u64;

        // Seal
        let env = envelope_seal_v1(
            &sender_sign,
            &sender_id,
            &recipient_id,
            recipient_kex_pub.as_bytes(),
            MsgTypeV1::ControlMsg,
            plaintext,
            now,
        )
        .unwrap();

        // Open
        let (decrypted, verified_sender_id) =
            envelope_open_v1(&env, &recipient_kex_priv, &sender_sign_pub).unwrap();

        assert_eq!(decrypted.as_ref(), plaintext);
        assert_eq!(verified_sender_id, sender_id);
    }
}
