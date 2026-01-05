//! Transcript module for deterministic hashing.
//!
//! Provides a transcript builder that appends tagged data in a canonical
//! format, ensuring the same logical data produces the same hash everywhere.

use bytes::{BufMut, BytesMut};
use sha2::{Digest, Sha256};

/// Standard tag constants for common transcript fields.
pub mod tags {
    pub const DOMAIN: u32 = 0;
    pub const MESSAGE: u32 = 1;
    pub const KEY: u32 = 2;
    pub const NONCE: u32 = 3;
    pub const COUNTER: u32 = 4;
    pub const TIMESTAMP: u32 = 5;
    pub const ID: u32 = 6;
    pub const SIGNATURE: u32 = 7;
    pub const PAYLOAD: u32 = 8;
}

/// A minimal deterministic transcript builder.
/// We append (tag, len, bytes) tuples so the same logical data hashes the same everywhere.
#[derive(Clone, Debug, Default)]
pub struct Transcript {
    buf: BytesMut,
}

impl Transcript {
    /// Create a new transcript with the given domain separator.
    pub fn new(domain: &'static str) -> Self {
        let mut t = Self { buf: BytesMut::with_capacity(256) };
        t.append_str(tags::DOMAIN, domain);
        t
    }

    /// Create an empty transcript without a domain (for advanced use cases).
    pub fn empty() -> Self {
        Self { buf: BytesMut::with_capacity(256) }
    }

    /// Append raw bytes with a tag.
    pub fn append_bytes(&mut self, tag: u32, data: &[u8]) -> &mut Self {
        // tag (u32 be) + len (u32 be) + data
        self.buf.put_u32(tag);
        self.buf.put_u32(data.len() as u32);
        self.buf.extend_from_slice(data);
        self
    }

    /// Append a u64 value with a tag.
    pub fn append_u64(&mut self, tag: u32, v: u64) -> &mut Self {
        self.buf.put_u32(tag);
        self.buf.put_u32(8);
        self.buf.put_u64(v);
        self
    }

    /// Append a boolean value with a tag.
    pub fn append_bool(&mut self, tag: u32, v: bool) -> &mut Self {
        self.buf.put_u32(tag);
        self.buf.put_u32(1);
        self.buf.put_u8(if v { 1 } else { 0 });
        self
    }

    /// Append a string with a tag (encoded as UTF-8 bytes).
    pub fn append_str(&mut self, tag: u32, s: &str) -> &mut Self {
        self.append_bytes(tag, s.as_bytes())
    }

    /// Get the raw transcript bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Finalize the transcript by computing SHA-256 hash of its contents.
    ///
    /// This consumes the transcript and returns the 32-byte hash.
    pub fn finalize(self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.buf);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Finalize the transcript by computing SHA-256 hash without consuming it.
    pub fn finalize_ref(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.buf);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Fork the transcript to create a new branch.
    ///
    /// This clones the current transcript state and appends a fork label,
    /// allowing multiple independent derivations from the same base state.
    pub fn fork(&self, label: &str) -> Self {
        let mut forked = self.clone();
        // Use a special tag for fork labels to distinguish from regular data
        const FORK_TAG: u32 = u32::MAX;
        forked.append_str(FORK_TAG, label);
        forked
    }

    /// Get the current length of the transcript in bytes.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Check if the transcript is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcript_determinism() {
        let t1 = Transcript::new("test_domain")
            .append_bytes(1, b"hello")
            .append_u64(2, 12345)
            .clone();

        let t2 = Transcript::new("test_domain")
            .append_bytes(1, b"hello")
            .append_u64(2, 12345)
            .clone();

        assert_eq!(t1.as_bytes(), t2.as_bytes());
        assert_eq!(t1.finalize(), t2.finalize());
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let t1 = Transcript::new("test").append_bytes(1, b"hello").clone();
        let t2 = Transcript::new("test").append_bytes(1, b"world").clone();

        assert_ne!(t1.as_bytes(), t2.as_bytes());
        assert_ne!(t1.finalize(), t2.finalize());
    }

    #[test]
    fn test_different_domains_different_outputs() {
        let t1 = Transcript::new("domain_a").append_bytes(1, b"data").clone();
        let t2 = Transcript::new("domain_b").append_bytes(1, b"data").clone();

        assert_ne!(t1.as_bytes(), t2.as_bytes());
        assert_ne!(t1.finalize(), t2.finalize());
    }

    #[test]
    fn test_fork_creates_independent_branch() {
        let base = Transcript::new("base")
            .append_bytes(1, b"shared_data")
            .clone();

        let fork_a = base.fork("branch_a");
        let fork_b = base.fork("branch_b");

        // Forks should have different content
        assert_ne!(fork_a.as_bytes(), fork_b.as_bytes());
        assert_ne!(fork_a.finalize_ref(), fork_b.finalize_ref());

        // Same fork label should be deterministic
        let fork_a2 = base.fork("branch_a");
        assert_eq!(fork_a.finalize(), fork_a2.finalize());
    }

    #[test]
    fn test_fork_preserves_base() {
        let base = Transcript::new("base")
            .append_bytes(1, b"data")
            .clone();
        let base_len = base.len();

        let _fork = base.fork("label");

        // Base should be unchanged
        assert_eq!(base.len(), base_len);
    }

    #[test]
    fn test_finalize_ref_vs_finalize() {
        let t = Transcript::new("test")
            .append_bytes(1, b"data")
            .clone();

        let hash1 = t.finalize_ref();
        let hash2 = t.finalize();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_append_bool() {
        let t_true = Transcript::new("test").append_bool(1, true).clone();
        let t_false = Transcript::new("test").append_bool(1, false).clone();

        assert_ne!(t_true.finalize(), t_false.finalize());
    }

    #[test]
    fn test_tag_ordering_matters() {
        // Same data with different tags should produce different hashes
        let t1 = Transcript::new("test").append_bytes(1, b"data").clone();
        let t2 = Transcript::new("test").append_bytes(2, b"data").clone();

        assert_ne!(t1.finalize(), t2.finalize());
    }

    #[test]
    fn test_order_of_appends_matters() {
        let t1 = Transcript::new("test")
            .append_bytes(1, b"first")
            .append_bytes(2, b"second")
            .clone();

        let t2 = Transcript::new("test")
            .append_bytes(2, b"second")
            .append_bytes(1, b"first")
            .clone();

        assert_ne!(t1.finalize(), t2.finalize());
    }

    #[test]
    fn test_standard_tags_exist() {
        // Just verify the tags module is accessible
        assert_eq!(tags::DOMAIN, 0);
        assert!(tags::MESSAGE < tags::PAYLOAD);
    }
}
