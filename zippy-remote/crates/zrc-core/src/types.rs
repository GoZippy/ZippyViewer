use bytes::Bytes;
use ed25519_dalek::SigningKey;
use x25519_dalek::StaticSecret;
use zrc_proto::v1::PublicKeyV1;

#[derive(Clone)]
pub struct IdentityKeys {
    pub sign: SigningKey,
    pub sign_pub: PublicKeyV1, // Ed25519 bytes
    pub kex_priv: StaticSecret,
    pub kex_pub: PublicKeyV1,  // X25519 bytes
    pub id32: [u8; 32],        // sha256(sign_pub.key_bytes)
}

#[derive(Clone, Debug)]
pub struct Outgoing {
    pub recipient_id: Vec<u8>,   // raw 32-byte id
    pub envelope_bytes: Bytes,   // protobuf-encoded EnvelopeV1
}

