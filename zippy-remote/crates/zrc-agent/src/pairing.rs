use std::sync::Arc;
use std::time::SystemTime;
use zrc_core::pairing::{PairingHost, PairingError as CorePairingError, ConsentHandler, PairDecision};
use zrc_core::rate_limit::RateLimiter;
use zrc_core::store::Store;
use zrc_core::types::IdentityKeys;
use zrc_proto::v1::{InviteV1, PairRequestV1, PairReceiptV1, EndpointHintsV1, PermissionV1};
use async_trait::async_trait;
use thiserror::Error;
use tracing::{info, warn};
use dashmap::DashMap;

#[derive(Debug, Error)]
pub enum PairingError {
    #[error("core pairing error: {0}")]
    Core(#[from] CorePairingError),
    #[error("rate limit exceeded: retry after {0} seconds")]
    RateLimited(u64),
    #[error("store error: {0}")]
    Store(String),
}

/// Simple consent handler that auto-approves pairings (for testing/unattended mode)
pub struct AutoApproveConsentHandler {
    default_permissions: Vec<PermissionV1>,
}

impl AutoApproveConsentHandler {
    pub fn new(default_permissions: Vec<PermissionV1>) -> Self {
        Self { default_permissions }
    }
}

#[async_trait]
impl ConsentHandler for AutoApproveConsentHandler {
    async fn request_consent(
        &self,
        _operator_id: &[u8],
        _sas: Option<&str>,
    ) -> Result<PairDecision, CorePairingError> {
        Ok(PairDecision {
            approved: true,
            granted_perms: self.default_permissions.clone(),
            unattended_enabled: false,
            require_consent_each_time: true,
        })
    }
}

pub struct PairingManager<S: Store + Send + Sync + 'static, C: ConsentHandler + 'static> {
    device_keys: IdentityKeys,
    store: Arc<S>,
    consent_handler: Arc<C>,
    rate_limiter: Arc<RateLimiter>,
    active_invites: Arc<DashMap<String, SystemTime>>,
    #[allow(dead_code)]
    max_concurrent_pairings: usize,
}

impl<S: Store + Send + Sync + 'static, C: ConsentHandler + 'static> PairingManager<S, C> {
    pub fn new(
        device_keys: IdentityKeys,
        store: Arc<S>,
        consent_handler: Arc<C>,
        rate_limiter: Arc<RateLimiter>,
        max_concurrent_pairings: usize,
    ) -> Result<Self, PairingError> {
        Ok(Self {
            device_keys,
            store,
            consent_handler,
            rate_limiter,
            active_invites: Arc::new(DashMap::new()),
            max_concurrent_pairings,
        })
    }

    pub async fn generate_invite(
        &self,
        ttl_seconds: u32,
        transport_hints: Option<EndpointHintsV1>,
    ) -> Result<InviteV1, PairingError> {
        // Create a new pairing host for this invite
        let mut host = PairingHost::new(
            self.device_keys.clone(),
            self.store.clone(),
            self.consent_handler.clone(),
        );

        let invite = host
            .generate_invite(ttl_seconds, transport_hints)
            .await
            .map_err(PairingError::Core)?;

        // Track active invite
        let invite_id = hex::encode(&invite.device_id);
        self.active_invites.insert(invite_id, SystemTime::now());

        info!("Generated invite for device: {}", hex::encode(&invite.device_id));
        Ok(invite)
    }

    pub async fn handle_pair_request(
        &self,
        request: PairRequestV1,
        source_ip: std::net::IpAddr,
    ) -> Result<PairReceiptV1, PairingError> {
        // Rate limiting: 3 attempts per minute per source
        match self.rate_limiter.check_rate_limit(&source_ip.to_string(), zrc_core::rate_limit::RequestType::Pairing).await {
            Ok(()) => {},
            Err(e) => {
                warn!("Pairing rate limit exceeded from {}: {}", source_ip, e);
                return Err(PairingError::RateLimited(60));
            }
        }

        // Create a new pairing host to handle this request
        let mut host = PairingHost::new(
            self.device_keys.clone(),
            self.store.clone(),
            self.consent_handler.clone(),
        );

        // Handle the request
        let _action = host
            .handle_request(request, &source_ip.to_string())
            .await
            .map_err(PairingError::Core)?;

        // For now, auto-approve with default permissions
        let receipt = host
            .approve(0x03) // VIEW | CONTROL
            .await
            .map_err(PairingError::Core)?;

        info!("Pairing completed successfully");
        Ok(receipt)
    }

    pub async fn revoke_pairing(
        &self,
        _operator_id: &[u8; 32],
    ) -> Result<(), PairingError> {
        // TODO: Implement pairing revocation in store
        warn!("Pairing revocation not yet implemented");
        Ok(())
    }
}
