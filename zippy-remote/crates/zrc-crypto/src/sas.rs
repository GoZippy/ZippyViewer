use crate::hash::sha256;

/// 6-digit SAS from a transcript bytes blob.
/// Stable across platforms. Uses first 4 bytes big-endian.
pub fn sas_6digit(transcript_bytes: &[u8]) -> String {
    let h = sha256(transcript_bytes);
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:06}", n)
}

