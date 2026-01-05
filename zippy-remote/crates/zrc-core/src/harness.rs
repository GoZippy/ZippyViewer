//! Test harness for ZRC core functionality.
//!
//! This module provides test utilities and integration test helpers
//! for the pairing and session workflows.
//!
//! Note: The full end-to-end flow test will be implemented once
//! the session state machines are complete (Tasks 6-7).

use std::sync::Arc;

use async_trait::async_trait;
use getrandom::getrandom;

use crate::{
    pairing::{ConsentHandler, PairDecision, PairingController, PairingError, PairingHost},
    store::{InMemoryStore, InviteRecord},
    types::IdentityKeys,
};
use zrc_crypto::hash::sha256;
use zrc_proto::v1::{InviteV1, PermissionV1};

/// Auto-approve consent handler for testing.
pub struct AutoApprove;

#[async_trait]
impl ConsentHandler for AutoApprove {
    async fn request_consent(
        &self,
        _operator_id: &[u8],
        _sas: Option<&str>,
    ) -> Result<PairDecision, PairingError> {
        Ok(PairDecision {
            approved: true,
            granted_perms: vec![PermissionV1::View, PermissionV1::Control],
            unattended_enabled: true,
            require_consent_each_time: false,
        })
    }
}

/// Generate a random 16-byte array.
pub fn rand16() -> [u8; 16] {
    let mut b = [0u8; 16];
    getrandom(&mut b).expect("rng");
    b
}

/// Generate a random 32-byte array.
pub fn rand32() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom(&mut b).expect("rng");
    b
}

/// Build an InviteV1 + store-side InviteRecord, consistent.
pub fn make_invite(now_unix: u64, device: &IdentityKeys) -> (InviteV1, InviteRecord, [u8; 32]) {
    let invite_secret = rand32();
    let expires = now_unix + 600;
    let secret_hash = sha256(&invite_secret);

    let invite = InviteV1 {
        device_id: device.id32.to_vec(),
        device_sign_pub: device.sign_pub.key_bytes.clone(),
        invite_secret_hash: secret_hash.to_vec(),
        expires_at: expires,
        transport_hints: None,
    };

    let rec = InviteRecord {
        device_id: device.id32.to_vec(),
        invite_secret,
        expires_at_unix: expires,
    };

    (invite, rec, invite_secret)
}

/// Run a complete pairing flow between host and controller.
///
/// This tests the pairing state machines end-to-end:
/// 1. Host generates invite
/// 2. Controller imports invite
/// 3. Controller sends pair request
/// 4. Host verifies and approves
/// 5. Controller confirms SAS
pub async fn run_pairing_flow(
    device: IdentityKeys,
    operator: IdentityKeys,
) -> Result<(), PairingError> {
    let store_host = Arc::new(InMemoryStore::new());
    let store_ctrl = Arc::new(InMemoryStore::new());
    let consent = Arc::new(AutoApprove);

    // Create host and controller state machines
    let mut host = PairingHost::new(device.clone(), store_host.clone(), consent);
    let mut controller = PairingController::new(operator.clone(), store_ctrl.clone());

    // Host generates invite
    let invite = host.generate_invite(300, None).await?;

    // Get the invite secret from the host state
    let secret = match host.state() {
        crate::pairing::PairingHostState::InviteGenerated { secret, .. } => *secret,
        _ => return Err(PairingError::InvalidState("expected InviteGenerated state".into())),
    };

    // Controller imports invite
    controller.import_invite_decoded(invite.clone())?;

    // Controller sends pair request
    let request = controller.send_request(&secret, 0x03).await?;

    // Host handles request
    let _action = host.handle_request(request, "test-source").await?;

    // Host approves
    let receipt = host.approve(0x03).await?;

    // Controller handles receipt
    let _action = controller.handle_receipt(receipt).await?;

    // Controller confirms SAS
    let _receipt = controller.confirm_sas().await?;

    // Verify both sides are paired
    assert!(matches!(host.state(), crate::pairing::PairingHostState::Paired { .. }));
    assert!(controller.is_paired());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::generate_identity_keys;

    #[tokio::test]
    async fn test_pairing_flow() {
        let device = generate_identity_keys();
        let operator = generate_identity_keys();

        run_pairing_flow(device, operator)
            .await
            .expect("pairing flow should succeed");
    }
}
