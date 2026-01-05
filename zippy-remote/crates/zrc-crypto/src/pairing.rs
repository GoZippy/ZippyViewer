use crate::{sas::sas_6digit, transcript::Transcript};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroize;

use zrc_proto::v1::{PublicKeyV1, TimestampV1, UserIdV1, DeviceIdV1};

type HmacSha256 = Hmac<Sha256>;

/// Build canonical input for pair_proof:
/// operator_id || operator_sign_pub || operator_kex_pub || device_id || created_at
pub fn pair_proof_input_v1(
    operator_id: &UserIdV1,
    operator_sign_pub: &PublicKeyV1,
    operator_kex_pub: &PublicKeyV1,
    device_id: &DeviceIdV1,
    created_at: &TimestampV1,
) -> Vec<u8> {
    let mut t = Transcript::new("zrc_pair_proof_v1");

    // Tags are fixed and MUST NOT change once released.
    t.append_bytes(1, &operator_id.id);
    t.append_bytes(2, &operator_sign_pub.key_bytes);
    t.append_bytes(3, &operator_kex_pub.key_bytes);
    t.append_bytes(4, &device_id.id);
    t.append_u64(5, created_at.unix_seconds);

    t.as_bytes().to_vec()
}

/// Compute pair_proof = HMAC-SHA256(invite_secret, pair_proof_input_v1(...))
pub fn compute_pair_proof_v1(invite_secret: &[u8], proof_input: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(invite_secret)
        .expect("HMAC can take keys of any size");
    mac.update(proof_input);
    let out = mac.finalize().into_bytes();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

/// Pairing SAS transcript (no secrets). Both ends should compute this and show the SAS.
/// Use in invite-only pairing as optional verification; in discoverable mode, require it.
pub fn pairing_sas_transcript_v1(
    pair_request_fields_without_proof: &[u8], // caller provides canonical bytes, see below
    operator_sign_pub_bytes: &[u8],
    device_sign_pub_bytes: &[u8],
    created_at_unix: u64,
    invite_expires_at_unix: u64,
) -> Vec<u8> {
    let mut t = Transcript::new("zrc_pair_sas_v1");
    t.append_bytes(1, pair_request_fields_without_proof);
    t.append_bytes(2, operator_sign_pub_bytes);
    t.append_bytes(3, device_sign_pub_bytes);
    t.append_u64(4, created_at_unix);
    t.append_u64(5, invite_expires_at_unix);
    t.as_bytes().to_vec()
}

/// Convenience: get SAS string directly from the transcript builder above.
pub fn compute_pairing_sas_6digit_v1(transcript: &[u8]) -> String {
    sas_6digit(transcript)
}

/// Helper to build canonical "PairRequest without pair_proof" bytes, independent of protobuf encoding.
/// This is the safest path to avoid proto-encoding differences.
///
/// You can call this before you even serialize PairRequestV1.
pub fn canonical_pair_request_fields_without_proof_v1(
    operator_id: &UserIdV1,
    operator_sign_pub: &PublicKeyV1,
    operator_kex_pub: &PublicKeyV1,
    device_id: &DeviceIdV1,
    created_at: &TimestampV1,
    request_sas: bool,
) -> Vec<u8> {
    let mut t = Transcript::new("zrc_pair_request_fields_v1");
    t.append_bytes(1, &operator_id.id);
    t.append_bytes(2, &operator_sign_pub.key_bytes);
    t.append_bytes(3, &operator_kex_pub.key_bytes);
    t.append_bytes(4, &device_id.id);
    t.append_u64(5, created_at.unix_seconds);
    t.append_bool(6, request_sas);
    t.as_bytes().to_vec()
}

/// Zeroize helper for secrets you hold in memory.
pub fn zeroize_vec(mut v: Vec<u8>) {
    v.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;

    fn pk(bytes: &[u8]) -> PublicKeyV1 {
        PublicKeyV1 { key_type: 1, key_bytes: bytes.to_vec() } // key_type irrelevant for hashing
    }

    #[test]
    fn test_pair_proof_and_sas_stability() {
        // Fixed vectors (not real keys)
        let operator_id = UserIdV1 { id: <Vec<u8>>::from_hex("010203").unwrap() };
        let device_id = DeviceIdV1 { id: <Vec<u8>>::from_hex("aabbcc").unwrap() };

        let op_sign = pk(&<Vec<u8>>::from_hex("11".repeat(32)).unwrap());
        let op_kex  = pk(&<Vec<u8>>::from_hex("22".repeat(32)).unwrap());
        let dev_sign_bytes = <Vec<u8>>::from_hex("33".repeat(32)).unwrap();

        let created_at = TimestampV1 { unix_seconds: 1_760_000_000 };
        let invite_expires_at = 1_760_000_600u64;

        let proof_input = pair_proof_input_v1(&operator_id, &op_sign, &op_kex, &device_id, &created_at);
        let invite_secret = <Vec<u8>>::from_hex("44".repeat(32)).unwrap();
        let proof = compute_pair_proof_v1(&invite_secret, &proof_input);

        // Just ensure deterministic length and a deterministic SAS
        assert_eq!(proof.len(), 32);

        let fields_wo_proof = canonical_pair_request_fields_without_proof_v1(
            &operator_id, &op_sign, &op_kex, &device_id, &created_at, true
        );

        let sas_tx = pairing_sas_transcript_v1(
            &fields_wo_proof,
            &op_sign.key_bytes,
            &dev_sign_bytes,
            created_at.unix_seconds,
            invite_expires_at,
        );

        let sas = compute_pairing_sas_6digit_v1(&sas_tx);
        assert_eq!(sas.len(), 6);
        assert!(sas.chars().all(|c| c.is_ascii_digit()));
    }
}

