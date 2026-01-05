//! Ticket module for session ticket signing and verification.
//! Session tickets are capability tokens signed by the device.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::hash::sha256;
use crate::transcript::Transcript;
use zrc_proto::v1::{KeyTypeV1, PublicKeyV1, SessionTicketV1};

#[derive(Debug, thiserror::Error)]
pub enum TicketError {
    #[error("missing field: {0}")]
    Missing(&'static str),
    #[error("invalid key bytes")]
    InvalidKeyBytes,
    #[error("signature verification failed")]
    BadSignature,
    #[error("ticket expired")]
    Expired,
    #[error("binding mismatch")]
    BindingMismatch,
}

/// session_binding = H(session_id || operator_id || device_id || ticket_binding_nonce)
pub fn compute_session_binding_v1(
    session_id: &[u8],
    operator_id: &[u8],
    device_id: &[u8],
    ticket_binding_nonce: &[u8],
) -> [u8; 32] {
    let mut t = Transcript::new("zrc_ticket_bind_v1");
    t.append_bytes(1, session_id);
    t.append_bytes(2, operator_id);
    t.append_bytes(3, device_id);
    t.append_bytes(4, ticket_binding_nonce);
    sha256(t.as_bytes())
}

/// Produce canonical bytes of SessionTicket fields for signing.
/// We compute a hash over the ticket fields (excluding signature).
fn ticket_signing_bytes_v1(ticket: &SessionTicketV1) -> [u8; 32] {
    let mut t = Transcript::new("zrc_ticket_sig_v1");
    t.append_bytes(1, &ticket.ticket_id);
    t.append_bytes(2, &ticket.session_id);
    t.append_bytes(3, &ticket.operator_id);
    t.append_bytes(4, &ticket.device_id);
    t.append_u64(5, ticket.permissions as u64);
    t.append_u64(6, ticket.expires_at);
    t.append_bytes(7, &ticket.session_binding);
    sha256(t.as_bytes())
}

/// Sign the ticket (device_signature) using Ed25519.
pub fn sign_ticket_v1(
    device_sign: &SigningKey,
    ticket: &mut SessionTicketV1,
) -> Result<(), TicketError> {
    let digest = ticket_signing_bytes_v1(ticket);
    let sig: Signature = device_sign.sign(&digest);

    // Set the device signing public key
    ticket.device_sign_pub = Some(PublicKeyV1 {
        key_type: KeyTypeV1::Ed25519.into(),
        key_bytes: device_sign.verifying_key().to_bytes().to_vec(),
    });

    // Set the signature
    ticket.device_signature = sig.to_bytes().to_vec();

    Ok(())
}

/// Verify the ticket signature and expiry and binding.
/// - checks device_signature
/// - checks expires_at > now_unix
/// - checks session_binding matches expected
pub fn verify_ticket_v1(
    ticket: &SessionTicketV1,
    now_unix: u64,
    expected_session_binding: &[u8; 32],
) -> Result<(), TicketError> {
    // Check signature is present
    if ticket.device_signature.is_empty() {
        return Err(TicketError::Missing("device_signature"));
    }

    // Get device signing public key
    let device_sign_pub = ticket
        .device_sign_pub
        .as_ref()
        .ok_or(TicketError::Missing("device_sign_pub"))?;
    if device_sign_pub.key_bytes.len() != 32 {
        return Err(TicketError::InvalidKeyBytes);
    }

    // Check expiry
    if ticket.expires_at <= now_unix {
        return Err(TicketError::Expired);
    }

    // Check session binding
    if ticket.session_binding != expected_session_binding.as_slice() {
        return Err(TicketError::BindingMismatch);
    }

    // Verify signature
    let digest = ticket_signing_bytes_v1(ticket);

    let vk = VerifyingKey::from_bytes(
        device_sign_pub
            .key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| TicketError::InvalidKeyBytes)?,
    )
    .map_err(|_| TicketError::InvalidKeyBytes)?;

    let sig_bytes: [u8; 64] = ticket
        .device_signature
        .as_slice()
        .try_into()
        .map_err(|_| TicketError::InvalidKeyBytes)?;
    let s = Signature::from_bytes(&sig_bytes);

    vk.verify_strict(&digest, &s)
        .map_err(|_| TicketError::BadSignature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    #[test]
    fn test_ticket_sign_verify_round_trip() {
        let device_sign = SigningKey::generate(&mut OsRng);

        let session_binding = compute_session_binding_v1(
            b"session123",
            b"operator456",
            b"device789",
            b"nonce000",
        );

        let mut ticket = SessionTicketV1 {
            ticket_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            session_id: b"session123".to_vec(),
            operator_id: b"operator456".to_vec(),
            device_id: b"device789".to_vec(),
            permissions: 7,
            expires_at: 1700000000 + 3600, // 1 hour from now
            session_binding: session_binding.to_vec(),
            ..Default::default()
        };

        // Sign
        sign_ticket_v1(&device_sign, &mut ticket).unwrap();

        // Verify
        let now = 1700000000u64;
        verify_ticket_v1(&ticket, now, &session_binding).unwrap();
    }

    #[test]
    fn test_ticket_expired() {
        let device_sign = SigningKey::generate(&mut OsRng);

        let session_binding = compute_session_binding_v1(
            b"session123",
            b"operator456",
            b"device789",
            b"nonce000",
        );

        let mut ticket = SessionTicketV1 {
            ticket_id: vec![1; 16],
            session_id: b"session123".to_vec(),
            operator_id: b"operator456".to_vec(),
            device_id: b"device789".to_vec(),
            permissions: 7,
            expires_at: 1700000000, // Already expired
            session_binding: session_binding.to_vec(),
            ..Default::default()
        };

        sign_ticket_v1(&device_sign, &mut ticket).unwrap();

        // Verify should fail - ticket expired
        let now = 1700000001u64; // 1 second after expiry
        let result = verify_ticket_v1(&ticket, now, &session_binding);
        assert!(matches!(result, Err(TicketError::Expired)));
    }

    #[test]
    fn test_ticket_binding_mismatch() {
        let device_sign = SigningKey::generate(&mut OsRng);

        let session_binding = compute_session_binding_v1(
            b"session123",
            b"operator456",
            b"device789",
            b"nonce000",
        );

        let mut ticket = SessionTicketV1 {
            ticket_id: vec![1; 16],
            session_id: b"session123".to_vec(),
            operator_id: b"operator456".to_vec(),
            device_id: b"device789".to_vec(),
            permissions: 7,
            expires_at: 1700000000 + 3600,
            session_binding: session_binding.to_vec(),
            ..Default::default()
        };

        sign_ticket_v1(&device_sign, &mut ticket).unwrap();

        // Verify with wrong binding should fail
        let wrong_binding = [0u8; 32];
        let result = verify_ticket_v1(&ticket, 1700000000, &wrong_binding);
        assert!(matches!(result, Err(TicketError::BindingMismatch)));
    }
}
