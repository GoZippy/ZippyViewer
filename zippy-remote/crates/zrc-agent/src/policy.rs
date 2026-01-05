use std::time::{SystemTime, Duration};
use zrc_core::policy::{PolicyEngine as CorePolicyEngine, ConsentMode, PolicyError as CorePolicyError};
use zrc_proto::v1::PermissionV1;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("core policy error: {0}")]
    Core(#[from] CorePolicyError),
}

pub struct PolicyEngine {
    core: CorePolicyEngine,
    consent_mode: ConsentMode,
    allowed_hours: Option<(u8, u8)>, // (start_hour, end_hour) in 24-hour format
    allowed_days: Option<Vec<u8>>, // Days of week (0=Sunday, 6=Saturday)
}

impl PolicyEngine {
    pub fn new(consent_mode: ConsentMode) -> Self {
        Self {
            core: CorePolicyEngine::new(consent_mode),
            consent_mode,
            allowed_hours: None,
            allowed_days: None,
        }
    }

    pub fn with_time_restrictions(
        mut self,
        allowed_hours: Option<(u8, u8)>,
        allowed_days: Option<Vec<u8>>,
    ) -> Self {
        self.allowed_hours = allowed_hours;
        self.allowed_days = allowed_days;
        self
    }

    pub async fn evaluate_session(
        &self,
        operator_id: &[u8; 32],
        requested_permissions: &[PermissionV1],
        has_unattended_permission: bool,
    ) -> Result<Vec<PermissionV1>, PolicyError> {
        // Check time restrictions
        if let Some((start_hour, end_hour)) = self.allowed_hours {
            if !self.is_within_allowed_hours() {
                return Err(PolicyError::Core(CorePolicyError::TimeRestriction(
                    format!("Access not allowed outside {}:00-{}:00", start_hour, end_hour)
                )));
            }
        }

        if let Some(ref _allowed_days) = self.allowed_days {
            if !self.is_allowed_day() {
                return Err(PolicyError::Core(CorePolicyError::TimeRestriction(
                    "Access not allowed on this day".to_string()
                )));
            }
        }

        // Check if consent is required using core policy engine
        let requires_consent = self.core.requires_consent(operator_id, has_unattended_permission);
        
        if requires_consent {
            // For now, if consent is required, we return the requested permissions
            // The actual consent flow would be handled by the consent module
        }

        // Return the requested permissions (actual validation would use validate_permissions)
        Ok(requested_permissions.to_vec())
    }

    pub fn consent_mode(&self) -> ConsentMode {
        self.consent_mode
    }

    fn is_within_allowed_hours(&self) -> bool {
        if let Some((start_hour, end_hour)) = self.allowed_hours {
            let now = SystemTime::now();
            let duration_since_epoch = now.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO);
            let total_seconds = duration_since_epoch.as_secs();
            let hours_since_midnight = (total_seconds % 86400) / 3600;
            
            if start_hour <= end_hour {
                // Same day range
                hours_since_midnight >= start_hour as u64 && hours_since_midnight < end_hour as u64
            } else {
                // Overnight range
                hours_since_midnight >= start_hour as u64 || hours_since_midnight < end_hour as u64
            }
        } else {
            true
        }
    }

    fn is_allowed_day(&self) -> bool {
        if let Some(ref allowed_days) = self.allowed_days {
            let now = SystemTime::now();
            let duration_since_epoch = now.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(Duration::ZERO);
            let total_seconds = duration_since_epoch.as_secs();
            let days_since_epoch = total_seconds / 86400;
            let day_of_week = (days_since_epoch + 4) % 7; // Jan 1, 1970 was a Thursday (4)
            
            allowed_days.contains(&(day_of_week as u8))
        } else {
            true
        }
    }
}
