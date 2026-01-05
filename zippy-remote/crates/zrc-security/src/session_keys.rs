//! Session key derivation using HKDF.
//!
//! Requirements: 7.1, 7.2, 7.3

use hkdf::Hkdf;
use sha2::Sha256;

/// Shared secret for key derivation (32 bytes).
pub type SharedSecret = [u8; 32];

/// Session ID (32 bytes).
pub type SessionId = [u8; 32];

/// Peer ID (32 bytes).
pub type PeerId = [u8; 32];

/// Session keys for all directions and channels.
///
/// Separate keys are derived for each direction and channel type
/// to ensure key separation.
///
/// Requirements: 7.2, 7.3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKeys {
    /// Initiator to responder control channel key
    pub i2r_control: [u8; 32],
    /// Responder to initiator control channel key
    pub r2i_control: [u8; 32],
    /// Initiator to responder frames channel key
    pub i2r_frames: [u8; 32],
    /// Responder to initiator frames channel key
    pub r2i_frames: [u8; 32],
    /// Initiator to responder files channel key
    pub i2r_files: [u8; 32],
    /// Responder to initiator files channel key
    pub r2i_files: [u8; 32],
}

impl Default for SessionKeys {
    fn default() -> Self {
        Self {
            i2r_control: [0u8; 32],
            r2i_control: [0u8; 32],
            i2r_frames: [0u8; 32],
            r2i_frames: [0u8; 32],
            i2r_files: [0u8; 32],
            r2i_files: [0u8; 32],
        }
    }
}

/// Session key deriver using HKDF-SHA256.
///
/// Requirements: 7.1, 7.2, 7.3
pub struct SessionKeyDeriver;

impl SessionKeyDeriver {
    /// Derive session keys from a shared secret.
    ///
    /// Derives separate keys for each direction and channel type
    /// to ensure key separation.
    ///
    /// Requirements: 7.2, 7.3
    pub fn derive_keys(
        shared_secret: &SharedSecret,
        session_id: &SessionId,
        initiator_id: &PeerId,
        responder_id: &PeerId,
    ) -> SessionKeys {
        // Build HKDF info parameter
        let mut info = Vec::new();
        info.extend_from_slice(b"ZRC-SESSION-KEYS-v1");
        info.extend_from_slice(session_id);
        info.extend_from_slice(initiator_id);
        info.extend_from_slice(responder_id);

        let hk = Hkdf::<Sha256>::new(None, shared_secret);

        let mut keys = SessionKeys::default();

        // Derive separate keys for each direction and channel
        hk.expand(b"initiator-to-responder-control", &mut keys.i2r_control)
            .expect("HKDF expand should not fail for 32-byte output");
        hk.expand(b"responder-to-initiator-control", &mut keys.r2i_control)
            .expect("HKDF expand should not fail for 32-byte output");
        hk.expand(b"initiator-to-responder-frames", &mut keys.i2r_frames)
            .expect("HKDF expand should not fail for 32-byte output");
        hk.expand(b"responder-to-initiator-frames", &mut keys.r2i_frames)
            .expect("HKDF expand should not fail for 32-byte output");
        hk.expand(b"initiator-to-responder-files", &mut keys.i2r_files)
            .expect("HKDF expand should not fail for 32-byte output");
        hk.expand(b"responder-to-initiator-files", &mut keys.r2i_files)
            .expect("HKDF expand should not fail for 32-byte output");

        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation_is_deterministic() {
        let secret = [0xAA; 32];
        let session_id = [0xBB; 32];
        let initiator_id = [0xCC; 32];
        let responder_id = [0xDD; 32];

        let keys1 = SessionKeyDeriver::derive_keys(
            &secret,
            &session_id,
            &initiator_id,
            &responder_id,
        );
        let keys2 = SessionKeyDeriver::derive_keys(
            &secret,
            &session_id,
            &initiator_id,
            &responder_id,
        );

        assert_eq!(keys1, keys2);
    }

    #[test]
    fn test_key_separation() {
        let secret = [0xAA; 32];
        let session_id = [0xBB; 32];
        let initiator_id = [0xCC; 32];
        let responder_id = [0xDD; 32];

        let keys = SessionKeyDeriver::derive_keys(
            &secret,
            &session_id,
            &initiator_id,
            &responder_id,
        );

        // All keys should be different
        assert_ne!(keys.i2r_control, keys.r2i_control);
        assert_ne!(keys.i2r_control, keys.i2r_frames);
        assert_ne!(keys.i2r_control, keys.r2i_frames);
        assert_ne!(keys.i2r_control, keys.i2r_files);
        assert_ne!(keys.i2r_control, keys.r2i_files);
        assert_ne!(keys.r2i_control, keys.i2r_frames);
        assert_ne!(keys.r2i_control, keys.r2i_frames);
        assert_ne!(keys.r2i_control, keys.i2r_files);
        assert_ne!(keys.r2i_control, keys.r2i_files);
        assert_ne!(keys.i2r_frames, keys.r2i_frames);
        assert_ne!(keys.i2r_frames, keys.i2r_files);
        assert_ne!(keys.i2r_frames, keys.r2i_files);
        assert_ne!(keys.r2i_frames, keys.i2r_files);
        assert_ne!(keys.r2i_frames, keys.r2i_files);
        assert_ne!(keys.i2r_files, keys.r2i_files);
    }

    #[test]
    fn test_different_secrets_different_keys() {
        let secret1 = [0xAA; 32];
        let secret2 = [0xBB; 32];
        let session_id = [0xCC; 32];
        let initiator_id = [0xDD; 32];
        let responder_id = [0xEE; 32];

        let keys1 = SessionKeyDeriver::derive_keys(
            &secret1,
            &session_id,
            &initiator_id,
            &responder_id,
        );
        let keys2 = SessionKeyDeriver::derive_keys(
            &secret2,
            &session_id,
            &initiator_id,
            &responder_id,
        );

        assert_ne!(keys1.i2r_control, keys2.i2r_control);
    }
}
