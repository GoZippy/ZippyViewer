//! Integration tests for ZRC core pairing and session flows.

use zrc_core::{harness::run_pairing_flow, keys::generate_identity_keys};

#[tokio::test]
async fn test_pairing_flow() {
    let device = generate_identity_keys();
    let operator = generate_identity_keys();

    run_pairing_flow(device, operator)
        .await
        .expect("pairing flow should succeed");
}
