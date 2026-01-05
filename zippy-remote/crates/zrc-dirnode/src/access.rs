//! Access control for directory lookups

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use thiserror::Error;
use sha2::{Sha256, Digest};

#[derive(Debug, Error)]
pub enum AccessError {
    #[error("Unauthorized: invite token required")]
    Unauthorized,
    #[error("Forbidden: invalid invite token")]
    Forbidden,
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token format")]
    InvalidToken,
}

/// Access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// Require invite token for all lookups
    InviteOnly,
    /// Allow lookups with active discovery token
    DiscoveryEnabled,
    /// Open access (not recommended)
    Open,
}

/// Invite token
struct InviteToken {
    subject_ids: HashSet<[u8; 32]>,
    expires_at: Option<u64>,
    created_by: [u8; 32],
    token_id: String,
}

/// Access controller
pub struct AccessController {
    mode: AccessMode,
    invite_tokens: DashMap<String, Arc<InviteToken>>,
    admin_tokens: HashSet<String>,
}

impl AccessController {
    pub fn new(mode: AccessMode) -> Self {
        Self {
            mode,
            invite_tokens: DashMap::new(),
            admin_tokens: HashSet::new(),
        }
    }

    /// Add admin token
    pub fn add_admin_token(&mut self, token: String) {
        self.admin_tokens.insert(token);
    }

    /// Check if lookup is authorized
    pub fn authorize_lookup(
        &self,
        subject_id: &[u8; 32],
        token: Option<&str>,
        is_discoverable: bool,
    ) -> Result<(), AccessError> {
        match self.mode {
            AccessMode::Open => Ok(()),
            AccessMode::DiscoveryEnabled => {
                if is_discoverable {
                    Ok(())
                } else if let Some(token_str) = token {
                    self.verify_invite_token(token_str, subject_id)
                } else {
                    Err(AccessError::Unauthorized)
                }
            }
            AccessMode::InviteOnly => {
                if let Some(token_str) = token {
                    self.verify_invite_token(token_str, subject_id)
                } else {
                    Err(AccessError::Unauthorized)
                }
            }
        }
    }

    /// Verify invite token
    fn verify_invite_token(&self, token: &str, subject_id: &[u8; 32]) -> Result<(), AccessError> {
        // Parse token (format: base64(JSON))
        let token_data = base64::decode(token)
            .map_err(|_| AccessError::InvalidToken)?;
        let token_json: serde_json::Value = serde_json::from_slice(&token_data)
            .map_err(|_| AccessError::InvalidToken)?;

        let token_id = token_json["token_id"]
            .as_str()
            .ok_or(AccessError::InvalidToken)?;

        // Check if token exists
        let invite = self.invite_tokens.get(token_id)
            .ok_or(AccessError::Forbidden)?;

        // Check expiration
        if let Some(expires_at) = invite.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if expires_at <= now {
                return Err(AccessError::TokenExpired);
            }
        }

        // Check subject_id scope
        if !invite.subject_ids.is_empty() && !invite.subject_ids.contains(subject_id) {
            return Err(AccessError::Forbidden);
        }

        Ok(())
    }

    /// Create invite token
    pub fn create_invite(
        &self,
        subject_ids: Vec<[u8; 32]>,
        ttl: Option<Duration>,
        created_by: [u8; 32],
    ) -> String {
        let token_id = hex::encode(&rand::random::<[u8; 16]>());
        let expires_at = ttl.map(|d| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + d.as_secs()
        });

        let token = Arc::new(InviteToken {
            subject_ids: subject_ids.into_iter().collect(),
            expires_at,
            created_by,
            token_id: token_id.clone(),
        });

        self.invite_tokens.insert(token_id.clone(), token);

        // Encode token as base64(JSON)
        let token_json = serde_json::json!({
            "token_id": token_id,
            "expires_at": expires_at,
        });
        base64::encode(serde_json::to_vec(&token_json).unwrap())
    }

    /// Revoke invite token
    pub fn revoke_invite(&self, token: &str) -> Result<(), AccessError> {
        let token_data = base64::decode(token)
            .map_err(|_| AccessError::InvalidToken)?;
        let token_json: serde_json::Value = serde_json::from_slice(&token_data)
            .map_err(|_| AccessError::InvalidToken)?;

        let token_id = token_json["token_id"]
            .as_str()
            .ok_or(AccessError::InvalidToken)?;

        self.invite_tokens.remove(token_id);
        Ok(())
    }

    /// Check admin authorization
    pub fn authorize_admin(&self, token: &str) -> Result<(), AccessError> {
        if self.admin_tokens.contains(token) {
            Ok(())
        } else {
            Err(AccessError::Forbidden)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invite_only_access() {
        let ctrl = AccessController::new(AccessMode::InviteOnly);
        let subject_id = [1u8; 32];
        
        // Without token, should fail
        assert!(ctrl.authorize_lookup(&subject_id, None, false).is_err());
        
        // Create invite token
        let token = ctrl.create_invite(vec![subject_id], None, [2u8; 32]);
        
        // With token, should succeed
        assert!(ctrl.authorize_lookup(&subject_id, Some(&token), false).is_ok());
    }

    #[test]
    fn test_discovery_enabled_access() {
        let ctrl = AccessController::new(AccessMode::DiscoveryEnabled);
        let subject_id = [1u8; 32];
        
        // With discovery enabled, should succeed
        assert!(ctrl.authorize_lookup(&subject_id, None, true).is_ok());
        
        // Without discovery, should fail
        assert!(ctrl.authorize_lookup(&subject_id, None, false).is_err());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Property 4: Invite-Only Access
    /// Validates: Requirements 3.1, 3.2
    /// 
    /// Property: For any lookup without valid invite token (when in InviteOnly mode), 
    /// the request SHALL be rejected with 401.
    #[test]
    fn prop_invite_only_access() {
        proptest!(|(
            subject_id_bytes in prop::collection::vec(0u8..=255u8, 32..=32),
            has_token in proptest::bool::ANY,
        )| {
            let ctrl = AccessController::new(AccessMode::InviteOnly);
            let mut subject_id = [0u8; 32];
            subject_id.copy_from_slice(&subject_id_bytes);

            if has_token {
                // Create token
                let token = ctrl.create_invite(vec![subject_id], None, [0u8; 32]);
                // Should succeed with token
                prop_assert!(ctrl.authorize_lookup(&subject_id, Some(&token), false).is_ok());
            } else {
                // Should fail without token
                prop_assert!(ctrl.authorize_lookup(&subject_id, None, false).is_err());
            }
        });
    }
}
