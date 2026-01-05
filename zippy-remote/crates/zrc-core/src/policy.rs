//! Policy engine for consent and permission management.
//!
//! Implements configurable consent modes and permission validation
//! as specified in Requirements 5.1-5.8.

use std::collections::HashSet;
use thiserror::Error;
use tracing::{warn, info};

/// Permission flags for session capabilities.
/// These are bitmask values that can be combined.
pub mod permissions {
    /// View-only access to the remote screen.
    pub const VIEW: u32 = 1 << 0;
    /// Control access (keyboard and mouse input).
    pub const CONTROL: u32 = 1 << 1;
    /// Clipboard sharing between devices.
    pub const CLIPBOARD: u32 = 1 << 2;
    /// File transfer capability.
    pub const FILE_TRANSFER: u32 = 1 << 3;
    /// Audio streaming capability.
    pub const AUDIO: u32 = 1 << 4;
    /// Unattended access (allows sessions without user consent when policy permits).
    pub const UNATTENDED: u32 = 1 << 5;
    
    /// All permissions combined.
    pub const ALL: u32 = VIEW | CONTROL | CLIPBOARD | FILE_TRANSFER | AUDIO | UNATTENDED;
    
    /// Default permissions for new pairings (view and control only).
    pub const DEFAULT: u32 = VIEW | CONTROL;
}

/// Errors from policy evaluation.
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("time restriction: {0}")]
    TimeRestriction(String),
    #[error("operator not trusted: {0}")]
    OperatorNotTrusted(String),
}

/// Consent mode determining when user approval is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsentMode {
    /// Always require user approval for every session (default, most secure).
    #[default]
    AlwaysRequire,
    /// Allow sessions from paired operators with UNATTENDED permission.
    UnattendedAllowed,
    /// Only allow sessions from explicitly trusted operator IDs.
    TrustedOperatorsOnly,
}

/// Time-based access restrictions.
#[derive(Debug, Clone)]
pub struct TimeRestrictions {
    /// Allowed hours as (start_hour, end_hour) in 24h format.
    /// e.g., (9, 17) means 9:00 AM to 5:00 PM.
    pub allowed_hours: Option<(u8, u8)>,
    /// Allowed days of the week (0 = Sunday, 6 = Saturday).
    pub allowed_days: Option<Vec<u8>>,
}

impl Default for TimeRestrictions {
    fn default() -> Self {
        Self {
            allowed_hours: None,
            allowed_days: None,
        }
    }
}


/// Policy engine for evaluating consent requirements and permissions.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    /// Current consent mode.
    consent_mode: ConsentMode,
    /// Set of explicitly trusted operator IDs (32-byte identifiers).
    trusted_operators: HashSet<[u8; 32]>,
    /// Time-based restrictions.
    time_restrictions: TimeRestrictions,
    /// Maximum permissions that can be granted (bitmask).
    permission_limits: u32,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self {
            consent_mode: ConsentMode::AlwaysRequire,
            trusted_operators: HashSet::new(),
            time_restrictions: TimeRestrictions::default(),
            permission_limits: u32::MAX, // No limits by default
        }
    }
}

impl PolicyEngine {
    /// Create a new policy engine with the specified consent mode.
    pub fn new(consent_mode: ConsentMode) -> Self {
        Self {
            consent_mode,
            ..Default::default()
        }
    }

    /// Set the consent mode.
    pub fn set_consent_mode(&mut self, mode: ConsentMode) {
        self.consent_mode = mode;
    }

    /// Get the current consent mode.
    pub fn consent_mode(&self) -> ConsentMode {
        self.consent_mode
    }

    /// Add an operator to the trusted set.
    pub fn add_trusted_operator(&mut self, operator_id: [u8; 32]) {
        self.trusted_operators.insert(operator_id);
    }

    /// Remove an operator from the trusted set.
    pub fn remove_trusted_operator(&mut self, operator_id: &[u8; 32]) {
        self.trusted_operators.remove(operator_id);
    }

    /// Check if an operator is trusted.
    pub fn is_trusted(&self, operator_id: &[u8; 32]) -> bool {
        self.trusted_operators.contains(operator_id)
    }

    /// Set time restrictions.
    pub fn set_time_restrictions(&mut self, restrictions: TimeRestrictions) {
        self.time_restrictions = restrictions;
    }

    /// Set permission limits.
    pub fn set_permission_limits(&mut self, limits: u32) {
        self.permission_limits = limits;
    }


    /// Check if a session requires user consent.
    ///
    /// Returns `true` if consent is required, `false` if session can proceed automatically.
    pub fn requires_consent(&self, operator_id: &[u8; 32], has_unattended_permission: bool) -> bool {
        match self.consent_mode {
            ConsentMode::AlwaysRequire => true,
            ConsentMode::UnattendedAllowed => !has_unattended_permission,
            ConsentMode::TrustedOperatorsOnly => !self.trusted_operators.contains(operator_id),
        }
    }

    /// Check if a session requires user consent based on operator permissions.
    ///
    /// This is a convenience method that extracts the UNATTENDED permission from the
    /// paired permissions bitmask.
    pub fn requires_consent_for_permissions(&self, operator_id: &[u8; 32], paired_permissions: u32) -> bool {
        let has_unattended = (paired_permissions & permissions::UNATTENDED) != 0;
        self.requires_consent(operator_id, has_unattended)
    }

    /// Validate requested permissions against paired permissions and policy limits.
    ///
    /// Returns the effective permissions (intersection of requested, paired, and limits).
    /// Logs policy violations when requested permissions exceed allowed permissions.
    pub fn validate_permissions(
        &self,
        operator_id: &[u8; 32],
        requested: u32,
        paired: u32,
    ) -> Result<u32, PolicyError> {
        // Effective permissions = requested ∩ paired ∩ limits
        let effective = requested & paired & self.permission_limits;

        // If requested permissions exceed what's allowed, that's a policy violation
        if requested & !paired != 0 {
            let violation = "requested permissions exceed paired permissions";
            warn!(
                operator_id = hex::encode(operator_id),
                requested = requested,
                paired = paired,
                "Policy violation: {}", violation
            );
            return Err(PolicyError::PermissionDenied(violation.into()));
        }

        if requested & !self.permission_limits != 0 {
            let violation = "requested permissions exceed policy limits";
            warn!(
                operator_id = hex::encode(operator_id),
                requested = requested,
                limits = self.permission_limits,
                "Policy violation: {}", violation
            );
            return Err(PolicyError::PermissionDenied(violation.into()));
        }

        info!(
            operator_id = hex::encode(operator_id),
            effective = effective,
            "Permissions validated successfully"
        );
        Ok(effective)
    }

    /// Check time-based restrictions.
    ///
    /// Returns `Ok(())` if access is allowed at the current time.
    /// Logs policy violations when access is denied due to time restrictions.
    pub fn check_time_restrictions(&self) -> Result<(), PolicyError> {
        // If no restrictions are set, always allow
        if self.time_restrictions.allowed_hours.is_none()
            && self.time_restrictions.allowed_days.is_none()
        {
            return Ok(());
        }

        let now = chrono::Local::now();

        // Check allowed hours
        if let Some((start, end)) = self.time_restrictions.allowed_hours {
            let hour = now.format("%H").to_string().parse::<u8>().unwrap_or(0);
            if hour < start || hour >= end {
                let violation = format!(
                    "access only allowed between {}:00 and {}:00",
                    start, end
                );
                warn!(
                    current_hour = hour,
                    allowed_start = start,
                    allowed_end = end,
                    "Policy violation: time restriction - {}", violation
                );
                return Err(PolicyError::TimeRestriction(violation));
            }
        }

        // Check allowed days
        if let Some(ref days) = self.time_restrictions.allowed_days {
            let weekday = now.format("%w").to_string().parse::<u8>().unwrap_or(0);
            if !days.contains(&weekday) {
                let violation = "access not allowed on this day";
                warn!(
                    current_day = weekday,
                    allowed_days = ?days,
                    "Policy violation: time restriction - {}", violation
                );
                return Err(PolicyError::TimeRestriction(violation.into()));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consent_always_require() {
        let policy = PolicyEngine::new(ConsentMode::AlwaysRequire);
        let operator_id = [0u8; 32];
        // AlwaysRequire should always return true regardless of permissions
        assert!(policy.requires_consent(&operator_id, false));
        assert!(policy.requires_consent(&operator_id, true));
    }

    #[test]
    fn test_consent_unattended_allowed() {
        let policy = PolicyEngine::new(ConsentMode::UnattendedAllowed);
        let operator_id = [0u8; 32];
        // Without UNATTENDED permission, consent is required
        assert!(policy.requires_consent(&operator_id, false));
        // With UNATTENDED permission, consent is not required
        assert!(!policy.requires_consent(&operator_id, true));
    }

    #[test]
    fn test_consent_unattended_with_permission_flags() {
        let policy = PolicyEngine::new(ConsentMode::UnattendedAllowed);
        let operator_id = [0u8; 32];
        
        // Without UNATTENDED permission flag
        let perms_no_unattended = permissions::VIEW | permissions::CONTROL;
        assert!(policy.requires_consent_for_permissions(&operator_id, perms_no_unattended));
        
        // With UNATTENDED permission flag
        let perms_with_unattended = permissions::VIEW | permissions::CONTROL | permissions::UNATTENDED;
        assert!(!policy.requires_consent_for_permissions(&operator_id, perms_with_unattended));
    }

    #[test]
    fn test_consent_trusted_only() {
        let mut policy = PolicyEngine::new(ConsentMode::TrustedOperatorsOnly);
        let trusted_id = [1u8; 32];
        let untrusted_id = [2u8; 32];

        policy.add_trusted_operator(trusted_id);

        // Trusted operator doesn't need consent
        assert!(!policy.requires_consent(&trusted_id, false));
        // Untrusted operator needs consent
        assert!(policy.requires_consent(&untrusted_id, false));
    }

    #[test]
    fn test_trusted_operator_management() {
        let mut policy = PolicyEngine::new(ConsentMode::TrustedOperatorsOnly);
        let operator_id = [1u8; 32];

        // Initially not trusted
        assert!(!policy.is_trusted(&operator_id));
        assert!(policy.requires_consent(&operator_id, false));

        // Add to trusted
        policy.add_trusted_operator(operator_id);
        assert!(policy.is_trusted(&operator_id));
        assert!(!policy.requires_consent(&operator_id, false));

        // Remove from trusted
        policy.remove_trusted_operator(&operator_id);
        assert!(!policy.is_trusted(&operator_id));
        assert!(policy.requires_consent(&operator_id, false));
    }

    #[test]
    fn test_permission_validation() {
        let policy = PolicyEngine::default();
        let operator_id = [0u8; 32];

        // Requested within paired permissions
        let result = policy.validate_permissions(&operator_id, 0b0011, 0b0111);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0b0011);

        // Requested exceeds paired permissions
        let result = policy.validate_permissions(&operator_id, 0b1111, 0b0011);
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_validation_with_flags() {
        let policy = PolicyEngine::default();
        let operator_id = [0u8; 32];

        // Request VIEW and CONTROL when paired with VIEW, CONTROL, CLIPBOARD
        let paired = permissions::VIEW | permissions::CONTROL | permissions::CLIPBOARD;
        let requested = permissions::VIEW | permissions::CONTROL;
        let result = policy.validate_permissions(&operator_id, requested, paired);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), requested);

        // Request FILE_TRANSFER when not paired with it
        let requested_extra = permissions::VIEW | permissions::FILE_TRANSFER;
        let result = policy.validate_permissions(&operator_id, requested_extra, paired);
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_limits() {
        let mut policy = PolicyEngine::default();
        let operator_id = [0u8; 32];

        // Set policy limit to VIEW only
        policy.set_permission_limits(permissions::VIEW);

        // Even if paired with CONTROL, policy limits prevent it
        let paired = permissions::VIEW | permissions::CONTROL;
        let requested = permissions::VIEW | permissions::CONTROL;
        let result = policy.validate_permissions(&operator_id, requested, paired);
        assert!(result.is_err());

        // Requesting only VIEW should work
        let result = policy.validate_permissions(&operator_id, permissions::VIEW, paired);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), permissions::VIEW);
    }

    #[test]
    fn test_time_restrictions_none() {
        let policy = PolicyEngine::default();
        // No restrictions set, should always allow
        assert!(policy.check_time_restrictions().is_ok());
    }

    #[test]
    fn test_time_restrictions_hours_setup() {
        let mut policy = PolicyEngine::default();
        
        // Set allowed hours to 9-17 (9 AM to 5 PM)
        policy.set_time_restrictions(TimeRestrictions {
            allowed_hours: Some((9, 17)),
            allowed_days: None,
        });

        // The result depends on current time, but we verify the method runs
        let _ = policy.check_time_restrictions();
    }

    #[test]
    fn test_time_restrictions_days_setup() {
        let mut policy = PolicyEngine::default();
        
        // Set allowed days to weekdays (1-5, Monday-Friday)
        policy.set_time_restrictions(TimeRestrictions {
            allowed_hours: None,
            allowed_days: Some(vec![1, 2, 3, 4, 5]),
        });

        // The result depends on current day, but we verify the method runs
        let _ = policy.check_time_restrictions();
    }

    #[test]
    fn test_time_restrictions_combined() {
        let mut policy = PolicyEngine::default();
        
        // Set both hours and days restrictions
        policy.set_time_restrictions(TimeRestrictions {
            allowed_hours: Some((9, 17)),
            allowed_days: Some(vec![1, 2, 3, 4, 5]),
        });

        // The result depends on current time/day, but we verify the method runs
        let _ = policy.check_time_restrictions();
    }

    #[test]
    fn test_consent_mode_setter() {
        let mut policy = PolicyEngine::default();
        assert_eq!(policy.consent_mode(), ConsentMode::AlwaysRequire);

        policy.set_consent_mode(ConsentMode::UnattendedAllowed);
        assert_eq!(policy.consent_mode(), ConsentMode::UnattendedAllowed);

        policy.set_consent_mode(ConsentMode::TrustedOperatorsOnly);
        assert_eq!(policy.consent_mode(), ConsentMode::TrustedOperatorsOnly);
    }
}
