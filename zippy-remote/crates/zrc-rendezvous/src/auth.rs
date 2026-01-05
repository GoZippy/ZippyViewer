use dashmap::DashMap;
use std::{collections::HashSet, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Disabled,
    ServerWide,
    PerMailbox,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub server_tokens: HashSet<String>,
    pub mailbox_tokens: Arc<DashMap<Vec<u8>, HashSet<String>>>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: AuthMode::Disabled,
            server_tokens: HashSet::new(),
            mailbox_tokens: Arc::new(DashMap::new()),
        }
    }
}

impl AuthConfig {
    pub fn new(mode: AuthMode) -> Self {
        Self {
            mode,
            server_tokens: HashSet::new(),
            mailbox_tokens: Arc::new(DashMap::new()),
        }
    }

    pub fn validate(&self, token: Option<&str>, recipient_id: Option<&[u8]>) -> Result<(), AuthError> {
        match self.mode {
            AuthMode::Disabled => Ok(()),
            AuthMode::ServerWide => {
                let token = token.ok_or(AuthError::MissingToken)?;
                if self.server_tokens.contains(token) {
                    Ok(())
                } else {
                    Err(AuthError::InvalidToken)
                }
            }
            AuthMode::PerMailbox => {
                let token = token.ok_or(AuthError::MissingToken)?;
                let recipient_id = recipient_id.ok_or(AuthError::MissingRecipient)?;
                
                // Check server-wide tokens first
                if self.server_tokens.contains(token) {
                    return Ok(());
                }

                // Check per-mailbox tokens
                if let Some(tokens) = self.mailbox_tokens.get(recipient_id) {
                    if tokens.contains(token) {
                        return Ok(());
                    }
                }

                Err(AuthError::InvalidToken)
            }
        }
    }

    pub fn add_server_token(&mut self, token: String) {
        self.server_tokens.insert(token);
    }

    pub fn add_mailbox_token(&self, recipient_id: Vec<u8>, token: String) {
        self.mailbox_tokens
            .entry(recipient_id)
            .or_default()
            .insert(token);
    }

    pub fn remove_server_token(&mut self, token: &str) {
        self.server_tokens.remove(token);
    }

    pub fn remove_mailbox_token(&self, recipient_id: &[u8], token: &str) {
        if let Some(mut tokens) = self.mailbox_tokens.get_mut(recipient_id) {
            tokens.remove(token);
            if tokens.is_empty() {
                drop(tokens);
                self.mailbox_tokens.remove(recipient_id);
            }
        }
    }

    pub fn rotate_server_tokens(&mut self, new_tokens: HashSet<String>) {
        self.server_tokens = new_tokens;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing token")]
    MissingToken,
    #[error("invalid token")]
    InvalidToken,
    #[error("missing recipient id")]
    MissingRecipient,
}

pub fn extract_bearer_token(header: Option<&axum::http::HeaderValue>) -> Option<&str> {
    let header = header?;
    let header_str = header.to_str().ok()?;
    header_str.strip_prefix("Bearer ")
}
