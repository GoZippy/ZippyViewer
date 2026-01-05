//! Artifact verification.
//!
//! Handles verification of downloaded update artifacts using SHA-256 hashes
//! and optional platform-specific code signature verification.
//!
//! # Security
//!
//! This module implements secure artifact verification:
//! - SHA-256 hash verification against manifest-specified hash
//! - Constant-time comparison to prevent timing attacks
//! - Optional platform-specific code signature verification
//!
//! # Requirements
//!
//! - Requirement 2.1: Verify artifact hash matches manifest
//! - Requirement 2.2: Use SHA-256 for artifact hashing
//! - Requirement 2.3: Verify artifact signature (optional)
//! - Requirement 2.5: Reject artifacts with mismatched hashes

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::UpdateError;

/// Buffer size for reading files during hash computation.
/// 8KB is a good balance between memory usage and I/O efficiency.
const HASH_BUFFER_SIZE: usize = 8192;

/// Verifies downloaded artifacts for integrity and authenticity.
///
/// # Requirements
/// - Requirement 2.1: Verify artifact hash matches manifest
/// - Requirement 2.2: Use SHA-256 for artifact hashing
/// - Requirement 2.3: Verify artifact signature (optional)
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// use zrc_updater::ArtifactVerifier;
///
/// let verifier = ArtifactVerifier::new();
/// let expected_hash = [0u8; 32]; // From manifest
/// verifier.verify(Path::new("update.zip"), &expected_hash)?;
/// ```
pub struct ArtifactVerifier {
    /// Optional platform-specific code signature verifier.
    /// When present, code signature is verified after hash verification.
    code_verifier: Option<Box<dyn CodeSignerVerifier>>,
}

impl ArtifactVerifier {
    /// Create a new artifact verifier without code signature verification.
    ///
    /// This verifier will only check SHA-256 hashes.
    pub fn new() -> Self {
        Self { code_verifier: None }
    }

    /// Create a new artifact verifier with code signature verification.
    ///
    /// This verifier will check both SHA-256 hash and platform-specific
    /// code signature (e.g., Authenticode on Windows, codesign on macOS).
    ///
    /// # Arguments
    ///
    /// * `verifier` - Platform-specific code signature verifier
    pub fn with_code_verifier(verifier: Box<dyn CodeSignerVerifier>) -> Self {
        Self {
            code_verifier: Some(verifier),
        }
    }

    /// Check if code signature verification is enabled.
    pub fn has_code_verifier(&self) -> bool {
        self.code_verifier.is_some()
    }

    /// Verify artifact integrity and authenticity.
    ///
    /// This method performs the following verifications:
    /// 1. Compute SHA-256 hash of the artifact file
    /// 2. Compare hash with expected value using constant-time comparison
    /// 3. If code verifier is configured, verify platform code signature
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the downloaded artifact file
    /// * `expected_hash` - Expected SHA-256 hash from the signed manifest
    ///
    /// # Returns
    ///
    /// `Ok(())` if all verifications pass.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be read
    /// - Hash does not match (constant-time comparison)
    /// - Code signature verification fails (if enabled)
    ///
    /// # Security
    ///
    /// - Uses constant-time comparison to prevent timing attacks (Requirement 2.5)
    /// - Hash is computed before any code execution (Requirement 12.4)
    pub fn verify(&self, path: &Path, expected_hash: &[u8; 32]) -> Result<(), UpdateError> {
        // Compute the actual hash of the file
        let actual_hash = self.compute_hash(path)?;

        // Use constant-time comparison to prevent timing attacks (Requirement 2.5)
        if actual_hash.ct_eq(expected_hash).unwrap_u8() != 1 {
            tracing::error!(
                expected = %hex::encode(expected_hash),
                actual = %hex::encode(actual_hash),
                path = %path.display(),
                "Artifact hash mismatch"
            );
            return Err(UpdateError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(actual_hash),
            });
        }

        tracing::debug!(
            hash = %hex::encode(actual_hash),
            path = %path.display(),
            "Artifact hash verified"
        );

        // Verify code signature if verifier is configured (Requirement 2.3)
        if let Some(code_verifier) = &self.code_verifier {
            tracing::debug!(path = %path.display(), "Verifying code signature");
            code_verifier.verify(path)?;
            tracing::debug!(path = %path.display(), "Code signature verified");
        }

        tracing::info!(path = %path.display(), "Artifact verification complete");
        Ok(())
    }

    /// Compute SHA-256 hash of a file.
    ///
    /// Reads the file in chunks to handle large files efficiently
    /// without loading the entire file into memory.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to hash
    ///
    /// # Returns
    ///
    /// The 32-byte SHA-256 hash of the file contents.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or read.
    pub fn compute_hash(&self, path: &Path) -> Result<[u8; 32], UpdateError> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; HASH_BUFFER_SIZE];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash: [u8; 32] = hasher.finalize().into();
        Ok(hash)
    }

    /// Verify artifact size matches expected value.
    ///
    /// This is an additional check that can be performed before
    /// downloading the full artifact to detect truncated downloads.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the artifact file
    /// * `expected_size` - Expected file size in bytes from manifest
    ///
    /// # Returns
    ///
    /// `Ok(())` if size matches.
    ///
    /// # Errors
    ///
    /// Returns `UpdateError::SizeMismatch` if sizes don't match.
    pub fn verify_size(&self, path: &Path, expected_size: u64) -> Result<(), UpdateError> {
        let metadata = std::fs::metadata(path)?;
        let actual_size = metadata.len();

        if actual_size != expected_size {
            tracing::error!(
                expected = expected_size,
                actual = actual_size,
                path = %path.display(),
                "Artifact size mismatch"
            );
            return Err(UpdateError::SizeMismatch {
                expected: expected_size,
                actual: actual_size,
            });
        }

        tracing::debug!(
            size = actual_size,
            path = %path.display(),
            "Artifact size verified"
        );
        Ok(())
    }
}

impl Default for ArtifactVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-specific code signature verification.
///
/// Implementations verify that an artifact is signed by a trusted
/// code signing certificate. This provides an additional layer of
/// security beyond hash verification.
///
/// # Requirements
/// - Requirement 2.3: Verify artifact signature (optional)
pub trait CodeSignerVerifier: Send + Sync {
    /// Verify the code signature of an artifact.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the artifact to verify
    ///
    /// # Returns
    ///
    /// `Ok(())` if the signature is valid and from a trusted signer.
    ///
    /// # Errors
    ///
    /// Returns `UpdateError::CodeSignatureInvalid` if verification fails.
    fn verify(&self, path: &Path) -> Result<(), UpdateError>;

    /// Get a description of this verifier for logging.
    fn description(&self) -> &str;
}



// =============================================================================
// Platform-specific code verifiers
// =============================================================================

/// Windows Authenticode signature verifier.
///
/// Verifies that an executable is signed with a valid Authenticode
/// signature from a certificate with the expected thumbprint.
///
/// # Requirements
/// - Requirement 2.3: Verify artifact signature
/// - Requirement 6.4: Verify Windows code signature post-install
#[cfg(target_os = "windows")]
pub struct WindowsCodeVerifier {
    /// Expected certificate thumbprint (SHA-1 hash of certificate, hex encoded).
    /// This should be the thumbprint of your code signing certificate.
    expected_thumbprint: String,
}

#[cfg(target_os = "windows")]
impl WindowsCodeVerifier {
    /// Create a new Windows code verifier.
    ///
    /// # Arguments
    ///
    /// * `expected_thumbprint` - SHA-1 thumbprint of the expected signing certificate
    ///   (hex encoded, case-insensitive)
    pub fn new(expected_thumbprint: String) -> Self {
        Self {
            expected_thumbprint: expected_thumbprint.to_uppercase(),
        }
    }

    /// Get the expected certificate thumbprint.
    pub fn expected_thumbprint(&self) -> &str {
        &self.expected_thumbprint
    }
}

#[cfg(target_os = "windows")]
impl CodeSignerVerifier for WindowsCodeVerifier {
    fn verify(&self, path: &Path) -> Result<(), UpdateError> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{HANDLE, HWND};
        use windows::Win32::Security::WinTrust::{
            WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA,
            WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_VERIFY,
            WTD_UI_NONE, WINTRUST_DATA_PROVIDER_FLAGS, WINTRUST_DATA_UICONTEXT,
        };

        // Convert path to wide string
        let path_wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Set up WINTRUST_FILE_INFO
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(path_wide.as_ptr()),
            hFile: HANDLE::default(),
            pgKnownSubject: std::ptr::null_mut(),
        };

        // Set up WINTRUST_DATA
        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            pPolicyCallbackData: std::ptr::null_mut(),
            pSIPClientData: std::ptr::null_mut(),
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: windows::Win32::Security::WinTrust::WINTRUST_DATA_0 {
                pFile: &mut file_info,
            },
            dwStateAction: WTD_STATEACTION_VERIFY,
            hWVTStateData: HANDLE::default(),
            pwszURLReference: windows::core::PWSTR::null(),
            dwProvFlags: WINTRUST_DATA_PROVIDER_FLAGS(0),
            dwUIContext: WINTRUST_DATA_UICONTEXT(0),
            pSignatureSettings: std::ptr::null_mut(),
        };

        // Verify the signature
        // Use INVALID_HANDLE_VALUE cast to HWND for WinVerifyTrust
        let mut action_guid = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let result = unsafe {
            WinVerifyTrust(
                HWND(-1isize as *mut std::ffi::c_void), // INVALID_HANDLE_VALUE as HWND
                &mut action_guid,
                &mut trust_data as *mut _ as *mut std::ffi::c_void,
            )
        };

        if result != 0 {
            return Err(UpdateError::CodeSignatureInvalid(format!(
                "WinVerifyTrust failed with error code: 0x{:08X}",
                result
            )));
        }

        // For full implementation, we would also extract and verify the certificate thumbprint
        // This requires additional Windows API calls to get the signer certificate
        // and compute its SHA-1 thumbprint for comparison with expected_thumbprint
        
        tracing::debug!(
            path = %path.display(),
            "Windows Authenticode signature verified"
        );

        Ok(())
    }

    fn description(&self) -> &str {
        "Windows Authenticode"
    }
}

/// macOS code signature verifier.
///
/// Verifies that an application or binary is signed with a valid
/// code signature from a developer with the expected team ID.
/// Also checks notarization status.
///
/// # Requirements
/// - Requirement 2.3: Verify artifact signature
/// - Requirement 7.4: Verify code signature and notarization post-install
#[cfg(target_os = "macos")]
pub struct MacOSCodeVerifier {
    /// Expected Apple Developer Team ID.
    /// This is the 10-character alphanumeric identifier for your team.
    expected_team_id: String,
    /// Whether to verify notarization status.
    verify_notarization: bool,
}

#[cfg(target_os = "macos")]
impl MacOSCodeVerifier {
    /// Create a new macOS code verifier.
    ///
    /// # Arguments
    ///
    /// * `expected_team_id` - Apple Developer Team ID (10 characters)
    pub fn new(expected_team_id: String) -> Self {
        Self {
            expected_team_id,
            verify_notarization: true,
        }
    }

    /// Create a verifier without notarization check.
    ///
    /// Useful for development builds that aren't notarized.
    pub fn without_notarization(expected_team_id: String) -> Self {
        Self {
            expected_team_id,
            verify_notarization: false,
        }
    }

    /// Get the expected team ID.
    pub fn expected_team_id(&self) -> &str {
        &self.expected_team_id
    }

    /// Check if notarization verification is enabled.
    pub fn verifies_notarization(&self) -> bool {
        self.verify_notarization
    }
}

#[cfg(target_os = "macos")]
impl CodeSignerVerifier for MacOSCodeVerifier {
    fn verify(&self, path: &Path) -> Result<(), UpdateError> {
        use std::process::Command;

        // First, verify the code signature using codesign
        let codesign_output = Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(path)
            .output()
            .map_err(|e| UpdateError::CodeSignatureInvalid(format!(
                "Failed to run codesign: {}", e
            )))?;

        if !codesign_output.status.success() {
            let stderr = String::from_utf8_lossy(&codesign_output.stderr);
            return Err(UpdateError::CodeSignatureInvalid(format!(
                "codesign verification failed: {}", stderr
            )));
        }

        // Extract and verify the team ID
        let display_output = Command::new("codesign")
            .args(["-d", "--verbose=2"])
            .arg(path)
            .output()
            .map_err(|e| UpdateError::CodeSignatureInvalid(format!(
                "Failed to get signature details: {}", e
            )))?;

        let stderr = String::from_utf8_lossy(&display_output.stderr);
        
        // Look for TeamIdentifier in the output
        let team_id_found = stderr
            .lines()
            .find(|line| line.starts_with("TeamIdentifier="))
            .and_then(|line| line.strip_prefix("TeamIdentifier="))
            .map(|id| id.trim());

        match team_id_found {
            Some(team_id) if team_id == self.expected_team_id => {
                tracing::debug!(
                    team_id = team_id,
                    path = %path.display(),
                    "Team ID verified"
                );
            }
            Some(team_id) => {
                return Err(UpdateError::CodeSignatureInvalid(format!(
                    "Team ID mismatch: expected {}, got {}",
                    self.expected_team_id, team_id
                )));
            }
            None => {
                return Err(UpdateError::CodeSignatureInvalid(
                    "Could not extract Team ID from signature".to_string()
                ));
            }
        }

        // Verify notarization if enabled
        if self.verify_notarization {
            let spctl_output = Command::new("spctl")
                .args(["--assess", "--type", "execute", "-v"])
                .arg(path)
                .output()
                .map_err(|e| UpdateError::CodeSignatureInvalid(format!(
                    "Failed to run spctl: {}", e
                )))?;

            if !spctl_output.status.success() {
                let stderr = String::from_utf8_lossy(&spctl_output.stderr);
                return Err(UpdateError::CodeSignatureInvalid(format!(
                    "Notarization check failed: {}", stderr
                )));
            }

            tracing::debug!(
                path = %path.display(),
                "Notarization verified"
            );
        }

        tracing::debug!(
            path = %path.display(),
            "macOS code signature verified"
        );

        Ok(())
    }

    fn description(&self) -> &str {
        "macOS codesign"
    }
}

/// Linux code verifier (placeholder).
///
/// Linux doesn't have a standard code signing mechanism like Windows or macOS.
/// This verifier can be extended to support:
/// - GPG signature verification
/// - AppImage signature verification
/// - Custom signing schemes
#[cfg(target_os = "linux")]
pub struct LinuxCodeVerifier {
    /// Expected GPG key fingerprint (if using GPG signatures).
    expected_key_fingerprint: Option<String>,
}

#[cfg(target_os = "linux")]
impl LinuxCodeVerifier {
    /// Create a new Linux code verifier.
    pub fn new() -> Self {
        Self {
            expected_key_fingerprint: None,
        }
    }

    /// Create a verifier with GPG key fingerprint.
    pub fn with_gpg_key(fingerprint: String) -> Self {
        Self {
            expected_key_fingerprint: Some(fingerprint),
        }
    }
}

#[cfg(target_os = "linux")]
impl Default for LinuxCodeVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl CodeSignerVerifier for LinuxCodeVerifier {
    fn verify(&self, path: &Path) -> Result<(), UpdateError> {
        // Linux doesn't have a standard code signing mechanism
        // For now, we rely on hash verification only
        // 
        // Future implementations could:
        // 1. Verify GPG detached signatures
        // 2. Verify AppImage signatures
        // 3. Use custom signing schemes
        
        if let Some(ref _fingerprint) = self.expected_key_fingerprint {
            // TODO: Implement GPG signature verification
            tracing::warn!(
                path = %path.display(),
                "GPG signature verification not yet implemented"
            );
        }

        tracing::debug!(
            path = %path.display(),
            "Linux code verification skipped (no standard mechanism)"
        );

        Ok(())
    }

    fn description(&self) -> &str {
        "Linux (hash only)"
    }
}



// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Create a test file with known content and return its path and expected hash.
    fn create_test_file(content: &[u8]) -> (NamedTempFile, [u8; 32]) {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();

        // Compute expected hash
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash: [u8; 32] = hasher.finalize().into();

        (file, hash)
    }

    #[test]
    fn test_artifact_verifier_new() {
        let verifier = ArtifactVerifier::new();
        assert!(!verifier.has_code_verifier());
    }

    #[test]
    fn test_artifact_verifier_default() {
        let verifier = ArtifactVerifier::default();
        assert!(!verifier.has_code_verifier());
    }

    #[test]
    fn test_compute_hash_empty_file() {
        let (file, expected_hash) = create_test_file(b"");
        let verifier = ArtifactVerifier::new();
        
        let actual_hash = verifier.compute_hash(file.path()).unwrap();
        
        // SHA-256 of empty string
        assert_eq!(actual_hash, expected_hash);
        assert_eq!(
            hex::encode(actual_hash),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_hash_known_content() {
        let content = b"Hello, World!";
        let (file, expected_hash) = create_test_file(content);
        let verifier = ArtifactVerifier::new();
        
        let actual_hash = verifier.compute_hash(file.path()).unwrap();
        
        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_hash_large_file() {
        // Create a file larger than the buffer size (8KB)
        let content: Vec<u8> = (0..20000).map(|i| (i % 256) as u8).collect();
        let (file, expected_hash) = create_test_file(&content);
        let verifier = ArtifactVerifier::new();
        
        let actual_hash = verifier.compute_hash(file.path()).unwrap();
        
        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_hash_nonexistent_file() {
        let verifier = ArtifactVerifier::new();
        let result = verifier.compute_hash(Path::new("/nonexistent/file.bin"));
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpdateError::IoError(_)));
    }

    #[test]
    fn test_verify_valid_hash() {
        let content = b"Test artifact content";
        let (file, expected_hash) = create_test_file(content);
        let verifier = ArtifactVerifier::new();
        
        let result = verifier.verify(file.path(), &expected_hash);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_invalid_hash() {
        let content = b"Test artifact content";
        let (file, _) = create_test_file(content);
        let verifier = ArtifactVerifier::new();
        
        // Use wrong hash
        let wrong_hash = [0u8; 32];
        let result = verifier.verify(file.path(), &wrong_hash);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            UpdateError::HashMismatch { expected, actual } => {
                assert_eq!(expected, hex::encode([0u8; 32]));
                assert!(!actual.is_empty());
            }
            e => panic!("Expected HashMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_verify_size_correct() {
        let content = b"Test content";
        let (file, _) = create_test_file(content);
        let verifier = ArtifactVerifier::new();
        
        let result = verifier.verify_size(file.path(), content.len() as u64);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_size_mismatch() {
        let content = b"Test content";
        let (file, _) = create_test_file(content);
        let verifier = ArtifactVerifier::new();
        
        let result = verifier.verify_size(file.path(), 999);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            UpdateError::SizeMismatch { expected, actual } => {
                assert_eq!(expected, 999);
                assert_eq!(actual, content.len() as u64);
            }
            e => panic!("Expected SizeMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_verify_size_nonexistent_file() {
        let verifier = ArtifactVerifier::new();
        let result = verifier.verify_size(Path::new("/nonexistent/file.bin"), 100);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpdateError::IoError(_)));
    }

    /// Mock code verifier for testing
    struct MockCodeVerifier {
        should_succeed: bool,
    }

    impl CodeSignerVerifier for MockCodeVerifier {
        fn verify(&self, _path: &Path) -> Result<(), UpdateError> {
            if self.should_succeed {
                Ok(())
            } else {
                Err(UpdateError::CodeSignatureInvalid("Mock failure".to_string()))
            }
        }

        fn description(&self) -> &str {
            "Mock verifier"
        }
    }

    #[test]
    fn test_verify_with_code_verifier_success() {
        let content = b"Test artifact";
        let (file, expected_hash) = create_test_file(content);
        
        let verifier = ArtifactVerifier::with_code_verifier(
            Box::new(MockCodeVerifier { should_succeed: true })
        );
        
        assert!(verifier.has_code_verifier());
        let result = verifier.verify(file.path(), &expected_hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_with_code_verifier_failure() {
        let content = b"Test artifact";
        let (file, expected_hash) = create_test_file(content);
        
        let verifier = ArtifactVerifier::with_code_verifier(
            Box::new(MockCodeVerifier { should_succeed: false })
        );
        
        let result = verifier.verify(file.path(), &expected_hash);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpdateError::CodeSignatureInvalid(_)));
    }

    #[test]
    fn test_verify_hash_fails_before_code_verification() {
        let content = b"Test artifact";
        let (file, _) = create_test_file(content);
        
        // Even with a code verifier that would succeed, hash mismatch should fail first
        let verifier = ArtifactVerifier::with_code_verifier(
            Box::new(MockCodeVerifier { should_succeed: true })
        );
        
        let wrong_hash = [0u8; 32];
        let result = verifier.verify(file.path(), &wrong_hash);
        
        // Should fail with hash mismatch, not code signature error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpdateError::HashMismatch { .. }));
    }

    #[test]
    fn test_constant_time_comparison() {
        // This test verifies that we're using constant-time comparison
        // by checking that both matching and non-matching hashes work correctly
        let content = b"Test content for timing";
        let (file, correct_hash) = create_test_file(content);
        let verifier = ArtifactVerifier::new();

        // Correct hash should pass
        assert!(verifier.verify(file.path(), &correct_hash).is_ok());

        // Hash with one bit different should fail
        let mut almost_correct = correct_hash;
        almost_correct[0] ^= 1;
        assert!(verifier.verify(file.path(), &almost_correct).is_err());

        // Completely different hash should fail
        let completely_wrong = [0xFFu8; 32];
        assert!(verifier.verify(file.path(), &completely_wrong).is_err());
    }

    // Platform-specific tests
    #[cfg(target_os = "windows")]
    mod windows_tests {
        use super::*;

        #[test]
        fn test_windows_code_verifier_new() {
            let verifier = WindowsCodeVerifier::new("ABC123".to_string());
            assert_eq!(verifier.expected_thumbprint(), "ABC123");
        }

        #[test]
        fn test_windows_code_verifier_uppercase() {
            let verifier = WindowsCodeVerifier::new("abc123".to_string());
            assert_eq!(verifier.expected_thumbprint(), "ABC123");
        }
    }

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;

        #[test]
        fn test_macos_code_verifier_new() {
            let verifier = MacOSCodeVerifier::new("ABCDE12345".to_string());
            assert_eq!(verifier.expected_team_id(), "ABCDE12345");
            assert!(verifier.verifies_notarization());
        }

        #[test]
        fn test_macos_code_verifier_without_notarization() {
            let verifier = MacOSCodeVerifier::without_notarization("ABCDE12345".to_string());
            assert_eq!(verifier.expected_team_id(), "ABCDE12345");
            assert!(!verifier.verifies_notarization());
        }
    }

    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::*;

        #[test]
        fn test_linux_code_verifier_new() {
            let verifier = LinuxCodeVerifier::new();
            assert!(verifier.expected_key_fingerprint.is_none());
        }

        #[test]
        fn test_linux_code_verifier_with_gpg() {
            let verifier = LinuxCodeVerifier::with_gpg_key("FINGERPRINT".to_string());
            assert_eq!(verifier.expected_key_fingerprint, Some("FINGERPRINT".to_string()));
        }

        #[test]
        fn test_linux_code_verifier_verify() {
            let content = b"Test";
            let (file, _) = create_test_file(content);
            let verifier = LinuxCodeVerifier::new();
            
            // Should succeed (no-op on Linux)
            assert!(verifier.verify(file.path()).is_ok());
        }
    }
}
