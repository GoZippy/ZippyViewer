//! SAS (Short Authentication String) verification for MITM detection.
//!
//! Requirements: 2.3, 2.6

use zrc_crypto::sas::sas_6digit;

/// Session transcript for SAS computation.
///
/// Contains the handshake messages and peer IDs used to compute
/// a deterministic SAS that both parties can verify.
#[derive(Debug, Clone)]
pub struct SessionTranscript {
    /// Raw transcript bytes
    bytes: Vec<u8>,
}

impl SessionTranscript {
    /// Create a new session transcript.
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
        }
    }

    /// Get the transcript as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert transcript to bytes (consumes).
    pub fn to_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl Default for SessionTranscript {
    fn default() -> Self {
        Self::new()
    }
}

/// SAS verification for MITM detection.
///
/// Requirements: 2.3, 2.6
pub struct SasVerification;

impl SasVerification {
    /// Compute a 6-digit SAS from a session transcript.
    ///
    /// Uses SHA-256 hash of the transcript and converts to 6-digit decimal.
    ///
    /// Requirements: 2.3
    pub fn compute_sas(transcript: &SessionTranscript) -> String {
        sas_6digit(transcript.as_bytes())
    }

    /// Compute transcript from handshake messages and peer IDs.
    ///
    /// Includes all handshake messages and IDs in a canonical format
    /// to ensure both parties compute the same SAS.
    ///
    /// Requirements: 2.3
    pub fn compute_transcript(
        initiator_hello: &[u8],
        responder_hello: &[u8],
        initiator_id: &[u8],
        responder_id: &[u8],
    ) -> SessionTranscript {
        let mut bytes = Vec::new();
        
        // Domain separator
        bytes.extend_from_slice(b"ZRC-SAS-v1");
        
        // Handshake messages
        bytes.extend_from_slice(&(initiator_hello.len() as u32).to_be_bytes());
        bytes.extend_from_slice(initiator_hello);
        bytes.extend_from_slice(&(responder_hello.len() as u32).to_be_bytes());
        bytes.extend_from_slice(responder_hello);
        
        // Peer IDs
        bytes.extend_from_slice(&(initiator_id.len() as u32).to_be_bytes());
        bytes.extend_from_slice(initiator_id);
        bytes.extend_from_slice(&(responder_id.len() as u32).to_be_bytes());
        bytes.extend_from_slice(responder_id);

        SessionTranscript { bytes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sas_is_deterministic() {
        let hello1 = b"initiator_hello";
        let hello2 = b"responder_hello";
        let id1 = b"initiator_id_32_bytes_long_!!!!";
        let id2 = b"responder_id_32_bytes_long_!!!!";

        let transcript1 = SasVerification::compute_transcript(hello1, hello2, id1, id2);
        let transcript2 = SasVerification::compute_transcript(hello1, hello2, id1, id2);

        let sas1 = SasVerification::compute_sas(&transcript1);
        let sas2 = SasVerification::compute_sas(&transcript2);

        assert_eq!(sas1, sas2);
        assert_eq!(sas1.len(), 6);
    }

    #[test]
    fn test_sas_is_6_digits() {
        let hello1 = b"test1";
        let hello2 = b"test2";
        let id1 = b"id1_32_bytes_long_!!!!!!!!!!";
        let id2 = b"id2_32_bytes_long_!!!!!!!!!!";

        let transcript = SasVerification::compute_transcript(hello1, hello2, id1, id2);
        let sas = SasVerification::compute_sas(&transcript);

        assert_eq!(sas.len(), 6);
        assert!(sas.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_different_inputs_different_sas() {
        let hello1a = b"hello_a";
        let hello1b = b"hello_b";
        let hello2 = b"responder";
        let id1 = b"id1_32_bytes_long_!!!!!!!!!!";
        let id2 = b"id2_32_bytes_long_!!!!!!!!!!";

        let transcript_a = SasVerification::compute_transcript(hello1a, hello2, id1, id2);
        let transcript_b = SasVerification::compute_transcript(hello1b, hello2, id1, id2);

        let sas_a = SasVerification::compute_sas(&transcript_a);
        let sas_b = SasVerification::compute_sas(&transcript_b);

        assert_ne!(sas_a, sas_b);
    }
}
