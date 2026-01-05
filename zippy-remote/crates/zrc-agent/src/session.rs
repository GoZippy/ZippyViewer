use std::sync::Arc;
use std::time::{Duration, SystemTime};
use zrc_core::session::{SessionHost, SessionError as CoreSessionError, SessionConsentHandler, SessionConsentDecision};
use zrc_core::store::Store;
use zrc_core::policy::PolicyEngine;
use zrc_core::types::IdentityKeys;
use zrc_proto::v1::{SessionInitRequestV1, SessionInitResponseV1, SessionTicketV1};
use async_trait::async_trait;
use thiserror::Error;
use tracing::info;
use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub ticket: SessionTicketV1,
    pub operator_id: [u8; 32],
    pub started_at: SystemTime,
    pub last_activity: SystemTime,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("core session error: {0}")]
    Core(#[from] CoreSessionError),
    #[error("max concurrent sessions exceeded")]
    MaxSessionsExceeded,
    #[error("session not found")]
    NotFound,
}

/// Simple consent handler that auto-approves sessions (for testing/unattended mode)
pub struct AutoApproveSessionConsentHandler;

#[async_trait]
impl SessionConsentHandler for AutoApproveSessionConsentHandler {
    async fn request_consent(
        &self,
        _operator_id: &[u8],
        requested_permissions: u32,
        paired_permissions: u32,
    ) -> Result<SessionConsentDecision, CoreSessionError> {
        // Grant the intersection of requested and paired permissions
        Ok(SessionConsentDecision {
            approved: true,
            granted_permissions: requested_permissions & paired_permissions,
        })
    }
}

pub struct SessionManager<S: Store + Send + Sync + 'static, C: SessionConsentHandler + 'static> {
    device_keys: IdentityKeys,
    store: Arc<S>,
    policy: Arc<PolicyEngine>,
    consent_handler: Arc<C>,
    active_sessions: Arc<DashMap<Vec<u8>, ActiveSession>>,
    max_concurrent_sessions: usize,
    session_timeout: Duration,
}

impl<S: Store + Send + Sync + 'static, C: SessionConsentHandler + 'static> SessionManager<S, C> {
    pub fn new(
        device_keys: IdentityKeys,
        store: Arc<S>,
        policy: Arc<PolicyEngine>,
        consent_handler: Arc<C>,
        max_concurrent_sessions: usize,
        session_timeout: Duration,
    ) -> Result<Self, SessionError> {
        Ok(Self {
            device_keys,
            store,
            policy,
            consent_handler,
            active_sessions: Arc::new(DashMap::new()),
            max_concurrent_sessions,
            session_timeout,
        })
    }

    pub async fn handle_session_request(
        &self,
        request: SessionInitRequestV1,
        _require_consent: bool,
    ) -> Result<SessionInitResponseV1, SessionError> {
        // Check max concurrent sessions
        if self.active_sessions.len() >= self.max_concurrent_sessions {
            return Err(SessionError::MaxSessionsExceeded);
        }

        // Create a new session host to handle this request
        let mut host = SessionHost::new(
            self.device_keys.clone(),
            self.store.clone(),
            self.policy.clone(),
            self.consent_handler.clone(),
        );

        // Handle the request
        let action = host
            .handle_request(request.clone())
            .await
            .map_err(SessionError::Core)?;

        // Get the response based on the action
        let response = match action {
            zrc_core::session::SessionAction::AutoApproved { response } => response,
            zrc_core::session::SessionAction::AwaitingConsent { .. } => {
                // For now, auto-approve
                host.approve().await.map_err(SessionError::Core)?
            }
            zrc_core::session::SessionAction::Rejected { reason } => {
                return Err(SessionError::Core(CoreSessionError::PermissionDenied(reason)));
            }
        };

        // Track active session
        if let Some(ticket) = &response.issued_ticket {
            let ticket_id = ticket.ticket_id.clone();
            let operator_id = request.operator_id.clone();
            let operator_id_bytes: [u8; 32] = operator_id
                .try_into()
                .map_err(|_| SessionError::Core(CoreSessionError::MissingField("operator_id".to_string())))?;

            let session = ActiveSession {
                ticket: ticket.clone(),
                operator_id: operator_id_bytes,
                started_at: SystemTime::now(),
                last_activity: SystemTime::now(),
            };
            self.active_sessions.insert(ticket_id, session);
        }

        info!("Session started successfully");
        Ok(response)
    }

    pub async fn terminate_session(&self, ticket_id: &[u8]) -> Result<(), SessionError> {
        if self.active_sessions.remove(ticket_id).is_none() {
            return Err(SessionError::NotFound);
        }
        info!("Session terminated: {}", hex::encode(ticket_id));
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) {
        let now = SystemTime::now();
        let mut to_remove = Vec::new();

        for entry in self.active_sessions.iter() {
            let session = entry.value();
            if now.duration_since(session.started_at).unwrap_or(Duration::ZERO) > self.session_timeout {
                to_remove.push(entry.key().clone());
            }
        }

        for ticket_id in to_remove {
            let _ = self.terminate_session(&ticket_id).await;
        }
    }

    pub fn active_session_count(&self) -> usize {
        self.active_sessions.len()
    }
}
