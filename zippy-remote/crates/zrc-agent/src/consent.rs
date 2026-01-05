use async_trait::async_trait;
use std::sync::Arc;
use zrc_proto::v1::{PermissionV1, UserIdV1};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum ConsentError {
    #[error("consent denied by user")]
    Denied,
    #[error("consent timeout")]
    Timeout,
    #[error("consent UI error: {0}")]
    UiError(String),
}

#[derive(Debug, Clone)]
pub struct PairingConsentRequest {
    pub operator_id: [u8; 32],
    pub operator_name: Option<String>,
    pub requested_permissions: Vec<PermissionV1>,
}

#[derive(Debug, Clone)]
pub struct SessionConsentRequest {
    pub operator_id: [u8; 32],
    pub operator_name: Option<String>,
    pub requested_permissions: Vec<PermissionV1>,
    pub session_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ConsentDecision {
    pub approved: bool,
    pub granted_permissions: Vec<PermissionV1>,
}

#[async_trait]
pub trait ConsentHandler: Send + Sync {
    async fn request_pairing_consent(
        &self,
        request: PairingConsentRequest,
    ) -> Result<ConsentDecision, ConsentError>;

    async fn request_session_consent(
        &self,
        request: SessionConsentRequest,
    ) -> Result<ConsentDecision, ConsentError>;

    async fn terminate_all_sessions(&self) -> Result<(), ConsentError>;
}

/// GUI-based consent handler using system tray and dialogs
pub struct GuiConsentHandler {
    // TODO: System tray integration
    // TODO: Dialog UI integration
}

impl GuiConsentHandler {
    pub fn new() -> Result<Self, ConsentError> {
        // TODO: Initialize system tray and UI components
        Ok(Self {})
    }
}

#[async_trait]
impl ConsentHandler for GuiConsentHandler {
    async fn request_pairing_consent(
        &self,
        request: PairingConsentRequest,
    ) -> Result<ConsentDecision, ConsentError> {
        // TODO: Show pairing consent dialog
        warn!("GUI consent handler not yet implemented, defaulting to deny");
        Err(ConsentError::Denied)
    }

    async fn request_session_consent(
        &self,
        request: SessionConsentRequest,
    ) -> Result<ConsentDecision, ConsentError> {
        // TODO: Show session consent dialog
        warn!("GUI consent handler not yet implemented, defaulting to deny");
        Err(ConsentError::Denied)
    }

    async fn terminate_all_sessions(&self) -> Result<(), ConsentError> {
        // TODO: Implement panic button
        info!("Panic button pressed - terminating all sessions");
        Ok(())
    }
}

/// Headless consent handler for unattended mode
pub struct HeadlessConsentHandler {
    allow_unattended: bool,
}

impl HeadlessConsentHandler {
    pub fn new(allow_unattended: bool) -> Self {
        Self { allow_unattended }
    }
}

#[async_trait]
impl ConsentHandler for HeadlessConsentHandler {
    async fn request_pairing_consent(
        &self,
        _request: PairingConsentRequest,
    ) -> Result<ConsentDecision, ConsentError> {
        if self.allow_unattended {
            info!("Headless mode: auto-approving pairing");
            Ok(ConsentDecision {
                approved: true,
                granted_permissions: vec![PermissionV1::View, PermissionV1::Control],
            })
        } else {
            Err(ConsentError::Denied)
        }
    }

    async fn request_session_consent(
        &self,
        _request: SessionConsentRequest,
    ) -> Result<ConsentDecision, ConsentError> {
        if self.allow_unattended {
            info!("Headless mode: auto-approving session");
            Ok(ConsentDecision {
                approved: true,
                granted_permissions: vec![PermissionV1::View, PermissionV1::Control],
            })
        } else {
            Err(ConsentError::Denied)
        }
    }

    async fn terminate_all_sessions(&self) -> Result<(), ConsentError> {
        info!("Headless mode: terminate all sessions requested");
        Ok(())
    }
}
