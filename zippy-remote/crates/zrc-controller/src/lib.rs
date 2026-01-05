//! ZRC Controller - CLI for remote control operations
//!
//! This crate provides a command-line interface for:
//! - Pairing with remote devices
//! - Initiating and managing sessions
//! - Sending input commands
//! - Debugging transport and cryptography

pub mod cli;
pub mod config;
pub mod debug;
pub mod frames;
pub mod identity;
pub mod input;
pub mod output;
pub mod pairing;
pub mod pairings;
pub mod session;

#[cfg(test)]
mod proptests;

pub use cli::Cli;
pub use config::{Config, CliOverrides};
pub use output::{OutputFormat, OutputFormatter, JsonResponse, SuccessMessage};

/// Exit codes for CLI operations
/// Requirements: 9.6
///
/// Exit codes provide machine-readable status for scripting and automation:
/// - 0: Success - operation completed successfully
/// - 1: General error - unspecified error occurred
/// - 2: Authentication failed - credentials or verification failed
/// - 3: Timeout - operation timed out
/// - 4: Connection failed - could not establish connection
/// - 5: Invalid input - bad arguments or data provided
/// - 6: Not paired - device pairing required
/// - 7: Permission denied - insufficient permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Operation completed successfully (exit code 0)
    Success = 0,
    /// General error (exit code 1)
    GeneralError = 1,
    /// Authentication failed (exit code 2)
    AuthenticationFailed = 2,
    /// Operation timed out (exit code 3)
    Timeout = 3,
    /// Connection failed (exit code 4)
    ConnectionFailed = 4,
    /// Invalid input provided (exit code 5)
    InvalidInput = 5,
    /// Device not paired (exit code 6)
    NotPaired = 6,
    /// Permission denied (exit code 7)
    PermissionDenied = 7,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code as i32
    }
}

impl ExitCode {
    /// Convert to process exit code
    pub fn to_exit_code(self) -> std::process::ExitCode {
        std::process::ExitCode::from(self as u8)
    }

    /// Get the exit code name as a string
    pub fn name(&self) -> &'static str {
        match self {
            ExitCode::Success => "SUCCESS",
            ExitCode::GeneralError => "GENERAL_ERROR",
            ExitCode::AuthenticationFailed => "AUTH_FAILED",
            ExitCode::Timeout => "TIMEOUT",
            ExitCode::ConnectionFailed => "CONNECTION_FAILED",
            ExitCode::InvalidInput => "INVALID_INPUT",
            ExitCode::NotPaired => "NOT_PAIRED",
            ExitCode::PermissionDenied => "PERMISSION_DENIED",
        }
    }

    /// Get a human-readable description of the exit code
    pub fn description(&self) -> &'static str {
        match self {
            ExitCode::Success => "Operation completed successfully",
            ExitCode::GeneralError => "An unspecified error occurred",
            ExitCode::AuthenticationFailed => "Authentication or verification failed",
            ExitCode::Timeout => "Operation timed out",
            ExitCode::ConnectionFailed => "Could not establish connection",
            ExitCode::InvalidInput => "Invalid arguments or data provided",
            ExitCode::NotPaired => "Device pairing required",
            ExitCode::PermissionDenied => "Insufficient permissions for operation",
        }
    }
}


#[cfg(test)]
mod exit_code_tests {
    use super::*;

    /// Test that exit codes match the documented values
    /// Requirements: 9.6
    #[test]
    fn test_exit_code_values() {
        // 0=success, 1=error, 2=auth_failed, 3=timeout
        assert_eq!(ExitCode::Success as i32, 0);
        assert_eq!(ExitCode::GeneralError as i32, 1);
        assert_eq!(ExitCode::AuthenticationFailed as i32, 2);
        assert_eq!(ExitCode::Timeout as i32, 3);
        assert_eq!(ExitCode::ConnectionFailed as i32, 4);
        assert_eq!(ExitCode::InvalidInput as i32, 5);
        assert_eq!(ExitCode::NotPaired as i32, 6);
        assert_eq!(ExitCode::PermissionDenied as i32, 7);
    }

    #[test]
    fn test_exit_code_from_i32() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::GeneralError), 1);
        assert_eq!(i32::from(ExitCode::AuthenticationFailed), 2);
        assert_eq!(i32::from(ExitCode::Timeout), 3);
    }

    #[test]
    fn test_exit_code_names() {
        assert_eq!(ExitCode::Success.name(), "SUCCESS");
        assert_eq!(ExitCode::GeneralError.name(), "GENERAL_ERROR");
        assert_eq!(ExitCode::AuthenticationFailed.name(), "AUTH_FAILED");
        assert_eq!(ExitCode::Timeout.name(), "TIMEOUT");
        assert_eq!(ExitCode::ConnectionFailed.name(), "CONNECTION_FAILED");
        assert_eq!(ExitCode::InvalidInput.name(), "INVALID_INPUT");
        assert_eq!(ExitCode::NotPaired.name(), "NOT_PAIRED");
        assert_eq!(ExitCode::PermissionDenied.name(), "PERMISSION_DENIED");
    }

    #[test]
    fn test_exit_code_descriptions() {
        // All exit codes should have non-empty descriptions
        assert!(!ExitCode::Success.description().is_empty());
        assert!(!ExitCode::GeneralError.description().is_empty());
        assert!(!ExitCode::AuthenticationFailed.description().is_empty());
        assert!(!ExitCode::Timeout.description().is_empty());
        assert!(!ExitCode::ConnectionFailed.description().is_empty());
        assert!(!ExitCode::InvalidInput.description().is_empty());
        assert!(!ExitCode::NotPaired.description().is_empty());
        assert!(!ExitCode::PermissionDenied.description().is_empty());
    }

    #[test]
    fn test_exit_code_to_process_exit_code() {
        // Verify conversion to std::process::ExitCode works
        let _ = ExitCode::Success.to_exit_code();
        let _ = ExitCode::GeneralError.to_exit_code();
        let _ = ExitCode::AuthenticationFailed.to_exit_code();
        let _ = ExitCode::Timeout.to_exit_code();
    }
}
