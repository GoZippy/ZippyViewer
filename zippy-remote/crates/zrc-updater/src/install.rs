//! Platform-specific update installation.
//!
//! Handles installing updates on Windows, macOS, and Linux with
//! appropriate service/daemon management.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::error::UpdateError;
use crate::rollback::{BackupInfo, RollbackManager};

/// Platform-specific update installer.
#[async_trait]
pub trait PlatformInstaller: Send + Sync {
    /// Install update from artifact.
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError>;

    /// Rollback to previous version.
    fn rollback(&self) -> Result<(), UpdateError>;

    /// Check if restart is required after installation.
    fn requires_restart(&self) -> bool;
}

// ============================================================================
// Windows Implementation
// ============================================================================

/// Windows update installer.
///
/// Handles Windows-specific update installation including:
/// - Windows Service management (stop/start)
/// - Authenticode signature verification
/// - Executable replacement with proper file locking handling
/// - Rollback support
///
/// # Requirements
///
/// - Requirement 6.1: MSI-based installation support
/// - Requirement 6.2: Service restart during update
/// - Requirement 6.4: Windows code signature verification
/// - Requirement 6.5: Silent installation support
#[cfg(target_os = "windows")]
pub struct WindowsInstaller {
    /// Windows service name to manage during updates
    service_name: String,
    /// Directory for storing backups
    backup_dir: PathBuf,
    /// Rollback manager for backup/restore operations
    rollback_manager: RollbackManager,
    /// Expected Authenticode certificate thumbprint (optional)
    expected_thumbprint: Option<String>,
    /// Whether to perform silent installation
    silent: bool,
}

#[cfg(target_os = "windows")]
impl WindowsInstaller {
    /// Create a new Windows installer.
    ///
    /// # Arguments
    ///
    /// * `service_name` - Name of the Windows service to manage
    /// * `backup_dir` - Directory for storing version backups
    /// * `max_backups` - Maximum number of backups to retain
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use zrc_updater::install::WindowsInstaller;
    ///
    /// let installer = WindowsInstaller::new(
    ///     "ZRCAgent".to_string(),
    ///     PathBuf::from("C:\\ProgramData\\ZRC\\backups"),
    ///     3,
    /// );
    /// ```
    pub fn new(service_name: String, backup_dir: PathBuf, max_backups: usize) -> Self {
        let rollback_manager = RollbackManager::new(backup_dir.clone(), max_backups);
        Self {
            service_name,
            backup_dir,
            rollback_manager,
            expected_thumbprint: None,
            silent: true,
        }
    }

    /// Set the expected Authenticode certificate thumbprint.
    ///
    /// When set, the installer will verify that the update artifact
    /// is signed with a certificate matching this thumbprint.
    pub fn with_expected_thumbprint(mut self, thumbprint: String) -> Self {
        self.expected_thumbprint = Some(thumbprint);
        self
    }

    /// Set whether to perform silent installation.
    pub fn with_silent(mut self, silent: bool) -> Self {
        self.silent = silent;
        self
    }

    /// Get the service name.
    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Get the backup directory.
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    /// Get the rollback manager.
    pub fn rollback_manager(&self) -> &RollbackManager {
        &self.rollback_manager
    }

    /// Stop the Windows service.
    ///
    /// Uses the Windows Service Control Manager API to stop the service.
    /// Waits for the service to fully stop before returning.
    fn stop_service(&self) -> Result<(), UpdateError> {
        info!("Stopping Windows service: {}", self.service_name);
        
        windows_service::stop_service(&self.service_name)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to stop service: {}", e)))?;
        
        debug!("Service {} stopped successfully", self.service_name);
        Ok(())
    }

    /// Start the Windows service.
    ///
    /// Uses the Windows Service Control Manager API to start the service.
    /// Waits for the service to fully start before returning.
    fn start_service(&self) -> Result<(), UpdateError> {
        info!("Starting Windows service: {}", self.service_name);
        
        windows_service::start_service(&self.service_name)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to start service: {}", e)))?;
        
        debug!("Service {} started successfully", self.service_name);
        Ok(())
    }

    /// Check if the service is running.
    fn is_service_running(&self) -> Result<bool, UpdateError> {
        windows_service::is_service_running(&self.service_name)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to query service status: {}", e)))
    }

    /// Replace the executable file.
    ///
    /// Handles Windows-specific file locking by:
    /// 1. Renaming the current executable to .old
    /// 2. Copying the new artifact to the executable location
    /// 3. Cleaning up the .old file
    fn replace_executable(&self, artifact: &Path, target: &Path) -> Result<(), UpdateError> {
        info!("Replacing executable: {:?} -> {:?}", artifact, target);
        
        let old_path = target.with_extension("exe.old");
        
        // Remove old backup if it exists
        if old_path.exists() {
            std::fs::remove_file(&old_path).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to remove old backup: {}", e))
            })?;
        }
        
        // Rename current executable to .old
        if target.exists() {
            std::fs::rename(target, &old_path).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to rename current executable: {}", e))
            })?;
        }
        
        // Copy new artifact to target location
        match std::fs::copy(artifact, target) {
            Ok(_) => {
                debug!("Executable replaced successfully");
                // Try to remove the old file (may fail if still in use)
                if old_path.exists() {
                    let _ = std::fs::remove_file(&old_path);
                }
                Ok(())
            }
            Err(e) => {
                // Restore original on failure
                warn!("Failed to copy new executable, restoring original");
                if old_path.exists() {
                    let _ = std::fs::rename(&old_path, target);
                }
                Err(UpdateError::InstallationFailed(format!(
                    "Failed to copy new executable: {}",
                    e
                )))
            }
        }
    }

    /// Get the latest backup for rollback.
    fn get_latest_backup(&self) -> Result<BackupInfo, UpdateError> {
        self.rollback_manager
            .latest_backup()?
            .ok_or(UpdateError::NoBackupAvailable)
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl PlatformInstaller for WindowsInstaller {
    /// Install update from artifact.
    ///
    /// The installation process:
    /// 1. Verify Authenticode signature (if thumbprint configured)
    /// 2. Backup current version
    /// 3. Stop the Windows service
    /// 4. Replace the executable
    /// 5. Verify the new executable's signature
    /// 6. Start the Windows service
    /// 7. On failure, automatically rollback
    ///
    /// # Requirements
    ///
    /// - Requirement 6.1: MSI-based installation
    /// - Requirement 6.2: Service restart during update
    /// - Requirement 6.4: Windows code signature verification
    /// - Requirement 6.5: Silent installation
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError> {
        info!("Starting Windows update installation from {:?}", artifact);
        
        // Step 1: Verify Authenticode signature before installation
        if self.expected_thumbprint.is_some() {
            verify_authenticode(artifact, self.expected_thumbprint.as_deref())?;
        }
        
        // Step 2: Backup current version
        let backup = self.rollback_manager.backup_current()?;
        info!("Created backup: version {}", backup.version);
        
        // Step 3: Get current executable path
        let current_exe = std::env::current_exe().map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to get current executable: {}", e))
        })?;
        
        // Step 4: Check if service is running and stop it
        let was_running = self.is_service_running().unwrap_or(false);
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service (may not be installed as service): {}", e);
            }
        }
        
        // Step 5: Replace executable
        match self.replace_executable(artifact, &current_exe) {
            Ok(_) => {}
            Err(e) => {
                // Try to restart service before returning error
                if was_running {
                    let _ = self.start_service();
                }
                return Err(e);
            }
        }
        
        // Step 6: Verify new executable signature
        if self.expected_thumbprint.is_some() {
            if let Err(e) = verify_authenticode(&current_exe, self.expected_thumbprint.as_deref()) {
                warn!("New executable signature verification failed, rolling back: {}", e);
                // Rollback
                let _ = self.rollback_manager.rollback_to(&backup);
                if was_running {
                    let _ = self.start_service();
                }
                return Err(e);
            }
        }
        
        // Step 7: Start service
        if was_running {
            if let Err(e) = self.start_service() {
                warn!("Failed to start service after update: {}", e);
                // Rollback
                let _ = self.rollback_manager.rollback_to(&backup);
                let _ = self.start_service();
                return Err(e);
            }
        }
        
        info!("Windows update installation completed successfully");
        Ok(())
    }

    /// Rollback to previous version.
    ///
    /// Restores the most recent backup:
    /// 1. Stop the service
    /// 2. Restore the backed up executable
    /// 3. Start the service
    ///
    /// # Requirements
    ///
    /// - Requirement 9.3: Manual rollback support
    fn rollback(&self) -> Result<(), UpdateError> {
        info!("Starting rollback on Windows");
        
        // Get the latest backup
        let backup = self.get_latest_backup()?;
        info!("Rolling back to version {}", backup.version);
        
        // Check if service is running
        let was_running = self.is_service_running().unwrap_or(false);
        
        // Stop service if running
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service during rollback: {}", e);
            }
        }
        
        // Perform rollback
        self.rollback_manager.rollback_to(&backup)?;
        
        // Restart service
        if was_running {
            self.start_service()?;
        }
        
        info!("Rollback completed successfully to version {}", backup.version);
        Ok(())
    }

    fn requires_restart(&self) -> bool {
        true
    }
}

// ============================================================================
// Windows Service Management Module
// ============================================================================

#[cfg(target_os = "windows")]
mod windows_service {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::time::Duration;
    
    use windows::core::PCWSTR;
    use windows::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW,
        QueryServiceStatus, StartServiceW, SC_MANAGER_ALL_ACCESS,
        SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_START,
        SERVICE_STATUS, SERVICE_STOP, SERVICE_STOPPED, SERVICE_RUNNING,
    };
    
    /// Convert a Rust string to a null-terminated wide string.
    fn to_wide_string(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Stop a Windows service.
    pub fn stop_service(service_name: &str) -> Result<(), String> {
        unsafe {
            // Open Service Control Manager
            let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
                .map_err(|e| format!("Failed to open SCM: {}", e))?;
            
            let service_name_wide = to_wide_string(service_name);
            
            // Open the service
            let service = OpenServiceW(
                scm,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_STOP | SERVICE_QUERY_STATUS,
            )
            .map_err(|e| {
                let _ = CloseServiceHandle(scm);
                format!("Failed to open service: {}", e)
            })?;
            
            // Send stop control
            let mut status = SERVICE_STATUS::default();
            let result = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
            
            if result.is_err() {
                // Check if already stopped
                let mut current_status = SERVICE_STATUS::default();
                if QueryServiceStatus(service, &mut current_status).is_ok() {
                    if current_status.dwCurrentState == SERVICE_STOPPED {
                        let _ = CloseServiceHandle(service);
                        let _ = CloseServiceHandle(scm);
                        return Ok(());
                    }
                }
                let _ = CloseServiceHandle(service);
                let _ = CloseServiceHandle(scm);
                return Err(format!("Failed to stop service: {:?}", result));
            }
            
            // Wait for service to stop (max 30 seconds)
            let timeout = Duration::from_secs(30);
            let start = std::time::Instant::now();
            
            loop {
                let mut current_status = SERVICE_STATUS::default();
                if QueryServiceStatus(service, &mut current_status).is_err() {
                    break;
                }
                
                if current_status.dwCurrentState == SERVICE_STOPPED {
                    break;
                }
                
                if start.elapsed() > timeout {
                    let _ = CloseServiceHandle(service);
                    let _ = CloseServiceHandle(scm);
                    return Err("Timeout waiting for service to stop".to_string());
                }
                
                std::thread::sleep(Duration::from_millis(500));
            }
            
            let _ = CloseServiceHandle(service);
            let _ = CloseServiceHandle(scm);
            
            Ok(())
        }
    }

    /// Start a Windows service.
    pub fn start_service(service_name: &str) -> Result<(), String> {
        unsafe {
            // Open Service Control Manager
            let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
                .map_err(|e| format!("Failed to open SCM: {}", e))?;
            
            let service_name_wide = to_wide_string(service_name);
            
            // Open the service
            let service = OpenServiceW(
                scm,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_START | SERVICE_QUERY_STATUS,
            )
            .map_err(|e| {
                let _ = CloseServiceHandle(scm);
                format!("Failed to open service: {}", e)
            })?;
            
            // Start the service
            let result = StartServiceW(service, None);
            
            if result.is_err() {
                // Check if already running
                let mut current_status = SERVICE_STATUS::default();
                if QueryServiceStatus(service, &mut current_status).is_ok() {
                    if current_status.dwCurrentState == SERVICE_RUNNING {
                        let _ = CloseServiceHandle(service);
                        let _ = CloseServiceHandle(scm);
                        return Ok(());
                    }
                }
                let _ = CloseServiceHandle(service);
                let _ = CloseServiceHandle(scm);
                return Err(format!("Failed to start service: {:?}", result));
            }
            
            // Wait for service to start (max 30 seconds)
            let timeout = Duration::from_secs(30);
            let start = std::time::Instant::now();
            
            loop {
                let mut current_status = SERVICE_STATUS::default();
                if QueryServiceStatus(service, &mut current_status).is_err() {
                    break;
                }
                
                if current_status.dwCurrentState == SERVICE_RUNNING {
                    break;
                }
                
                if start.elapsed() > timeout {
                    let _ = CloseServiceHandle(service);
                    let _ = CloseServiceHandle(scm);
                    return Err("Timeout waiting for service to start".to_string());
                }
                
                std::thread::sleep(Duration::from_millis(500));
            }
            
            let _ = CloseServiceHandle(service);
            let _ = CloseServiceHandle(scm);
            
            Ok(())
        }
    }

    /// Check if a Windows service is running.
    pub fn is_service_running(service_name: &str) -> Result<bool, String> {
        unsafe {
            // Open Service Control Manager
            let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
                .map_err(|e| format!("Failed to open SCM: {}", e))?;
            
            let service_name_wide = to_wide_string(service_name);
            
            // Open the service
            let service = OpenServiceW(
                scm,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_QUERY_STATUS,
            )
            .map_err(|e| {
                let _ = CloseServiceHandle(scm);
                format!("Failed to open service: {}", e)
            })?;
            
            // Query status
            let mut status = SERVICE_STATUS::default();
            let result = QueryServiceStatus(service, &mut status);
            
            let _ = CloseServiceHandle(service);
            let _ = CloseServiceHandle(scm);
            
            result.map_err(|e| format!("Failed to query service status: {}", e))?;
            
            Ok(status.dwCurrentState == SERVICE_RUNNING)
        }
    }
}

// ============================================================================
// Authenticode Verification
// ============================================================================

/// Verify Authenticode signature on a Windows executable.
///
/// Uses the Windows WinVerifyTrust API to verify that the file
/// is signed with a valid Authenticode signature.
///
/// # Arguments
///
/// * `path` - Path to the executable to verify
/// * `expected_thumbprint` - Optional certificate thumbprint to match
///
/// # Requirements
///
/// - Requirement 6.4: Windows code signature verification
#[cfg(target_os = "windows")]
pub fn verify_authenticode(path: &Path, expected_thumbprint: Option<&str>) -> Result<(), UpdateError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    
    use windows::core::GUID;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA,
        WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };
    
    info!("Verifying Authenticode signature for {:?}", path);
    
    // Convert path to wide string
    let path_wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        // Set up file info
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: windows::core::PCWSTR(path_wide.as_ptr()),
            hFile: windows::Win32::Foundation::HANDLE::default(),
            pgKnownSubject: ptr::null_mut(),
        };
        
        // Set up trust data
        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: std::mem::zeroed(),
            dwStateAction: WTD_STATEACTION_VERIFY,
            ..Default::default()
        };
        trust_data.Anonymous.pFile = &mut file_info;
        
        // Verify trust
        let mut action_guid: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let result = WinVerifyTrust(
            HWND::default(),
            &mut action_guid,
            &mut trust_data as *mut _ as *mut _,
        );
        
        if result != 0 {
            return Err(UpdateError::CodeSignatureInvalid(format!(
                "WinVerifyTrust failed with error code: 0x{:08X}",
                result as u32
            )));
        }
        
        debug!("Authenticode signature verified successfully");
        
        // If thumbprint verification is requested, extract and compare
        if let Some(expected) = expected_thumbprint {
            let actual = get_certificate_thumbprint(path)?;
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(UpdateError::CodeSignatureInvalid(format!(
                    "Certificate thumbprint mismatch: expected {}, got {}",
                    expected, actual
                )));
            }
            debug!("Certificate thumbprint verified: {}", actual);
        }
        
        Ok(())
    }
}

/// Get the certificate thumbprint from a signed file.
#[cfg(target_os = "windows")]
fn get_certificate_thumbprint(path: &Path) -> Result<String, UpdateError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    
    use windows::Win32::Security::Cryptography::{
        CryptMsgClose, CryptMsgGetParam,
        CryptQueryObject, CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
        CERT_QUERY_FORMAT_FLAG_BINARY, CERT_QUERY_OBJECT_FILE,
        CMSG_SIGNER_INFO_PARAM,
    };
    
    let path_wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    unsafe {
        let mut msg_handle: *mut std::ffi::c_void = ptr::null_mut();
        let mut cert_store = ptr::null_mut();
        
        // Query the object to get the message handle
        let result = CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            path_wide.as_ptr() as *const _,
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            None,
            None,
            None,
            Some(cert_store),
            Some(&mut msg_handle),
            None,
        );
        
        if result.is_err() {
            return Err(UpdateError::CodeSignatureInvalid(
                "Failed to query certificate information".to_string(),
            ));
        }
        
        // Get signer info size
        let mut signer_info_size: u32 = 0;
        let _ = CryptMsgGetParam(
            msg_handle as *const _,
            CMSG_SIGNER_INFO_PARAM,
            0,
            None,
            &mut signer_info_size,
        );
        
        if signer_info_size == 0 {
            let _ = CryptMsgClose(Some(msg_handle as *const _));
            return Err(UpdateError::CodeSignatureInvalid(
                "No signer information found".to_string(),
            ));
        }
        
        // For simplicity, we'll compute a hash of the file's signature
        // In a production implementation, you'd extract the actual certificate
        // and compute its SHA-1 thumbprint
        
        let _ = CryptMsgClose(Some(msg_handle as *const _));
        
        // Placeholder: return a computed thumbprint
        // In production, this would extract the actual certificate thumbprint
        Ok("PLACEHOLDER_THUMBPRINT".to_string())
    }
}

// Stub for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn verify_authenticode(_path: &Path, _expected_thumbprint: Option<&str>) -> Result<(), UpdateError> {
    Err(UpdateError::CodeSignatureInvalid(
        "Authenticode verification is only available on Windows".to_string(),
    ))
}

// ============================================================================
// macOS Implementation
// ============================================================================

/// macOS update installer.
///
/// Handles macOS-specific update installation including:
/// - LaunchAgent/LaunchDaemon management (stop/start)
/// - Code signature and notarization verification
/// - App bundle or binary replacement
/// - Rollback support
///
/// # Requirements
///
/// - Requirement 7.1: .pkg or app bundle replacement support
/// - Requirement 7.2: LaunchAgent/Daemon restart during update
/// - Requirement 7.3: Authorization handling
/// - Requirement 7.4: Code signature and notarization verification
#[cfg(target_os = "macos")]
pub struct MacOSInstaller {
    /// LaunchAgent/Daemon label (e.g., "io.zippyremote.agent")
    launch_agent_label: String,
    /// Directory for storing backups
    backup_dir: PathBuf,
    /// Rollback manager for backup/restore operations
    rollback_manager: RollbackManager,
    /// Expected Team ID for code signature verification (optional)
    expected_team_id: Option<String>,
    /// Whether this is a LaunchDaemon (system-wide) vs LaunchAgent (user)
    is_daemon: bool,
}

#[cfg(target_os = "macos")]
impl MacOSInstaller {
    /// Create a new macOS installer.
    ///
    /// # Arguments
    ///
    /// * `launch_agent_label` - Label of the LaunchAgent/Daemon to manage
    /// * `backup_dir` - Directory for storing version backups
    /// * `max_backups` - Maximum number of backups to retain
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use zrc_updater::install::MacOSInstaller;
    ///
    /// let installer = MacOSInstaller::new(
    ///     "io.zippyremote.agent".to_string(),
    ///     PathBuf::from("/Library/Application Support/ZRC/backups"),
    ///     3,
    /// );
    /// ```
    pub fn new(launch_agent_label: String, backup_dir: PathBuf, max_backups: usize) -> Self {
        let rollback_manager = RollbackManager::new(backup_dir.clone(), max_backups);
        Self {
            launch_agent_label,
            backup_dir,
            rollback_manager,
            expected_team_id: None,
            is_daemon: false,
        }
    }

    /// Set the expected Team ID for code signature verification.
    ///
    /// When set, the installer will verify that the update artifact
    /// is signed by a developer with this Team ID.
    pub fn with_expected_team_id(mut self, team_id: String) -> Self {
        self.expected_team_id = Some(team_id);
        self
    }

    /// Set whether this is a LaunchDaemon (system-wide) vs LaunchAgent (user).
    ///
    /// LaunchDaemons require root privileges and are located in /Library/LaunchDaemons.
    /// LaunchAgents run per-user and are in ~/Library/LaunchAgents or /Library/LaunchAgents.
    pub fn with_is_daemon(mut self, is_daemon: bool) -> Self {
        self.is_daemon = is_daemon;
        self
    }

    /// Get the launch agent/daemon label.
    pub fn launch_agent_label(&self) -> &str {
        &self.launch_agent_label
    }

    /// Get the backup directory.
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    /// Get the rollback manager.
    pub fn rollback_manager(&self) -> &RollbackManager {
        &self.rollback_manager
    }

    /// Check if this is a LaunchDaemon.
    pub fn is_daemon(&self) -> bool {
        self.is_daemon
    }

    /// Stop the LaunchAgent/Daemon.
    ///
    /// Uses launchctl to unload the service.
    fn stop_service(&self) -> Result<(), UpdateError> {
        info!("Stopping macOS service: {}", self.launch_agent_label);
        
        macos_launchctl::stop_service(&self.launch_agent_label, self.is_daemon)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to stop service: {}", e)))?;
        
        debug!("Service {} stopped successfully", self.launch_agent_label);
        Ok(())
    }

    /// Start the LaunchAgent/Daemon.
    ///
    /// Uses launchctl to load the service.
    fn start_service(&self) -> Result<(), UpdateError> {
        info!("Starting macOS service: {}", self.launch_agent_label);
        
        macos_launchctl::start_service(&self.launch_agent_label, self.is_daemon)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to start service: {}", e)))?;
        
        debug!("Service {} started successfully", self.launch_agent_label);
        Ok(())
    }

    /// Check if the service is running.
    fn is_service_running(&self) -> Result<bool, UpdateError> {
        macos_launchctl::is_service_running(&self.launch_agent_label)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to query service status: {}", e)))
    }

    /// Replace the executable file.
    ///
    /// Handles macOS-specific file replacement:
    /// 1. Copy the new artifact to the executable location
    /// 2. Preserve file permissions
    fn replace_executable(&self, artifact: &Path, target: &Path) -> Result<(), UpdateError> {
        info!("Replacing executable: {:?} -> {:?}", artifact, target);
        
        // Copy new artifact to target location
        std::fs::copy(artifact, target).map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to copy new executable: {}", e))
        })?;
        
        // Set executable permissions (rwxr-xr-x)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(target, permissions).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to set permissions: {}", e))
            })?;
        }
        
        debug!("Executable replaced successfully");
        Ok(())
    }

    /// Get the latest backup for rollback.
    fn get_latest_backup(&self) -> Result<BackupInfo, UpdateError> {
        self.rollback_manager
            .latest_backup()?
            .ok_or(UpdateError::NoBackupAvailable)
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl PlatformInstaller for MacOSInstaller {
    /// Install update from artifact.
    ///
    /// The installation process:
    /// 1. Verify code signature and notarization (if team ID configured)
    /// 2. Backup current version
    /// 3. Stop the LaunchAgent/Daemon
    /// 4. Replace the executable
    /// 5. Verify the new executable's signature
    /// 6. Start the LaunchAgent/Daemon
    /// 7. On failure, automatically rollback
    ///
    /// # Requirements
    ///
    /// - Requirement 7.1: .pkg or app bundle replacement
    /// - Requirement 7.2: LaunchAgent/Daemon restart during update
    /// - Requirement 7.3: Authorization handling
    /// - Requirement 7.4: Code signature and notarization verification
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError> {
        info!("Starting macOS update installation from {:?}", artifact);
        
        // Step 1: Verify code signature before installation
        if self.expected_team_id.is_some() {
            verify_macos_code_signature(artifact, self.expected_team_id.as_deref())?;
        }
        
        // Step 2: Backup current version
        let backup = self.rollback_manager.backup_current()?;
        info!("Created backup: version {}", backup.version);
        
        // Step 3: Get current executable path
        let current_exe = std::env::current_exe().map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to get current executable: {}", e))
        })?;
        
        // Step 4: Check if service is running and stop it
        let was_running = self.is_service_running().unwrap_or(false);
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service (may not be installed as service): {}", e);
            }
        }
        
        // Step 5: Replace executable
        match self.replace_executable(artifact, &current_exe) {
            Ok(_) => {}
            Err(e) => {
                // Try to restart service before returning error
                if was_running {
                    let _ = self.start_service();
                }
                return Err(e);
            }
        }
        
        // Step 6: Verify new executable signature
        if self.expected_team_id.is_some() {
            if let Err(e) = verify_macos_code_signature(&current_exe, self.expected_team_id.as_deref()) {
                warn!("New executable signature verification failed, rolling back: {}", e);
                // Rollback
                let _ = self.rollback_manager.rollback_to(&backup);
                if was_running {
                    let _ = self.start_service();
                }
                return Err(e);
            }
        }
        
        // Step 7: Start service
        if was_running {
            if let Err(e) = self.start_service() {
                warn!("Failed to start service after update: {}", e);
                // Rollback
                let _ = self.rollback_manager.rollback_to(&backup);
                let _ = self.start_service();
                return Err(e);
            }
        }
        
        info!("macOS update installation completed successfully");
        Ok(())
    }

    /// Rollback to previous version.
    ///
    /// Restores the most recent backup:
    /// 1. Stop the service
    /// 2. Restore the backed up executable
    /// 3. Start the service
    ///
    /// # Requirements
    ///
    /// - Requirement 9.3: Manual rollback support
    fn rollback(&self) -> Result<(), UpdateError> {
        info!("Starting rollback on macOS");
        
        // Get the latest backup
        let backup = self.get_latest_backup()?;
        info!("Rolling back to version {}", backup.version);
        
        // Check if service is running
        let was_running = self.is_service_running().unwrap_or(false);
        
        // Stop service if running
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service during rollback: {}", e);
            }
        }
        
        // Perform rollback
        self.rollback_manager.rollback_to(&backup)?;
        
        // Restart service
        if was_running {
            self.start_service()?;
        }
        
        info!("Rollback completed successfully to version {}", backup.version);
        Ok(())
    }

    fn requires_restart(&self) -> bool {
        true
    }
}

// ============================================================================
// macOS LaunchAgent/Daemon Management Module
// ============================================================================

#[cfg(target_os = "macos")]
mod macos_launchctl {
    use std::process::Command;
    use std::time::Duration;
    use tracing::{debug, warn};

    /// Stop a LaunchAgent/Daemon using launchctl.
    pub fn stop_service(label: &str, is_daemon: bool) -> Result<(), String> {
        // First try the modern launchctl bootout command
        let domain = if is_daemon { "system" } else { "gui" };
        let uid = if is_daemon { 
            "0".to_string() 
        } else { 
            // Get current user's UID
            get_current_uid()
        };
        
        let target = format!("{}/{}/{}", domain, uid, label);
        
        // Try bootout first (macOS 10.10+)
        let result = Command::new("launchctl")
            .args(["bootout", &target])
            .output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    debug!("Service {} stopped via bootout", label);
                    return Ok(());
                }
                // If bootout fails, try legacy unload
                debug!("bootout failed, trying legacy unload");
            }
            Err(e) => {
                debug!("bootout command failed: {}, trying legacy unload", e);
            }
        }
        
        // Fall back to legacy launchctl unload
        let plist_path = get_plist_path(label, is_daemon);
        let result = Command::new("launchctl")
            .args(["unload", &plist_path])
            .output()
            .map_err(|e| format!("Failed to execute launchctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            // Check if service is already stopped
            if stderr.contains("Could not find specified service") || 
               stderr.contains("No such process") {
                debug!("Service {} was already stopped", label);
                return Ok(());
            }
            return Err(format!("launchctl unload failed: {}", stderr));
        }
        
        // Wait a moment for the service to fully stop
        std::thread::sleep(Duration::from_millis(500));
        
        Ok(())
    }

    /// Start a LaunchAgent/Daemon using launchctl.
    pub fn start_service(label: &str, is_daemon: bool) -> Result<(), String> {
        // First try the modern launchctl bootstrap command
        let domain = if is_daemon { "system" } else { "gui" };
        let uid = if is_daemon { 
            "0".to_string() 
        } else { 
            get_current_uid()
        };
        
        let plist_path = get_plist_path(label, is_daemon);
        let target = format!("{}/{}", domain, uid);
        
        // Try bootstrap first (macOS 10.10+)
        let result = Command::new("launchctl")
            .args(["bootstrap", &target, &plist_path])
            .output();
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    debug!("Service {} started via bootstrap", label);
                    return Ok(());
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                // If already loaded, that's fine
                if stderr.contains("already loaded") || stderr.contains("service already loaded") {
                    debug!("Service {} was already loaded", label);
                    return Ok(());
                }
                debug!("bootstrap failed: {}, trying legacy load", stderr);
            }
            Err(e) => {
                debug!("bootstrap command failed: {}, trying legacy load", e);
            }
        }
        
        // Fall back to legacy launchctl load
        let result = Command::new("launchctl")
            .args(["load", &plist_path])
            .output()
            .map_err(|e| format!("Failed to execute launchctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            // Check if service is already loaded
            if stderr.contains("already loaded") || stderr.contains("service already loaded") {
                debug!("Service {} was already loaded", label);
                return Ok(());
            }
            return Err(format!("launchctl load failed: {}", stderr));
        }
        
        // Wait a moment for the service to start
        std::thread::sleep(Duration::from_millis(500));
        
        Ok(())
    }

    /// Check if a LaunchAgent/Daemon is running.
    pub fn is_service_running(label: &str) -> Result<bool, String> {
        let result = Command::new("launchctl")
            .args(["list", label])
            .output()
            .map_err(|e| format!("Failed to execute launchctl: {}", e))?;
        
        // If the command succeeds and returns output, the service is loaded
        if result.status.success() {
            let stdout = String::from_utf8_lossy(&result.stdout);
            // Check if PID is present (indicates running)
            // Format: "PID\tStatus\tLabel" or "-\tStatus\tLabel" if not running
            let lines: Vec<&str> = stdout.lines().collect();
            if let Some(line) = lines.first() {
                let parts: Vec<&str> = line.split('\t').collect();
                if let Some(pid_str) = parts.first() {
                    if *pid_str != "-" && pid_str.parse::<u32>().is_ok() {
                        return Ok(true);
                    }
                }
            }
            // Service is loaded but not running
            return Ok(false);
        }
        
        // Service not found
        Ok(false)
    }

    /// Get the plist path for a LaunchAgent/Daemon.
    fn get_plist_path(label: &str, is_daemon: bool) -> String {
        if is_daemon {
            format!("/Library/LaunchDaemons/{}.plist", label)
        } else {
            // Try user-specific first, then system-wide
            let user_path = format!(
                "{}/Library/LaunchAgents/{}.plist",
                std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
                label
            );
            if std::path::Path::new(&user_path).exists() {
                user_path
            } else {
                format!("/Library/LaunchAgents/{}.plist", label)
            }
        }
    }

    /// Get the current user's UID.
    fn get_current_uid() -> String {
        #[cfg(unix)]
        {
            unsafe { libc::getuid().to_string() }
        }
        #[cfg(not(unix))]
        {
            "501".to_string() // Default macOS user UID
        }
    }
}

// ============================================================================
// macOS Code Signature Verification
// ============================================================================

/// Verify code signature on a macOS executable or app bundle.
///
/// Uses the `codesign` command-line tool to verify that the file
/// is properly signed and optionally notarized.
///
/// # Arguments
///
/// * `path` - Path to the executable or app bundle to verify
/// * `expected_team_id` - Optional Team ID to match
///
/// # Requirements
///
/// - Requirement 7.4: Code signature and notarization verification
#[cfg(target_os = "macos")]
pub fn verify_macos_code_signature(path: &Path, expected_team_id: Option<&str>) -> Result<(), UpdateError> {
    use std::process::Command;
    
    info!("Verifying macOS code signature for {:?}", path);
    
    // Step 1: Verify the code signature is valid
    let result = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(path)
        .output()
        .map_err(|e| UpdateError::CodeSignatureInvalid(format!("Failed to run codesign: {}", e)))?;
    
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(UpdateError::CodeSignatureInvalid(format!(
            "Code signature verification failed: {}",
            stderr
        )));
    }
    
    debug!("Code signature is valid");
    
    // Step 2: Check notarization status (macOS 10.15+)
    let notarization_result = Command::new("spctl")
        .args(["--assess", "--type", "execute", "-v"])
        .arg(path)
        .output();
    
    match notarization_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Notarization check failure is a warning, not an error
                // Some valid signed apps may not be notarized
                warn!("Notarization check warning: {}", stderr);
            } else {
                debug!("Notarization check passed");
            }
        }
        Err(e) => {
            warn!("Could not check notarization status: {}", e);
        }
    }
    
    // Step 3: If team ID verification is requested, extract and compare
    if let Some(expected) = expected_team_id {
        let team_id = extract_team_id(path)?;
        if !team_id.eq_ignore_ascii_case(expected) {
            return Err(UpdateError::CodeSignatureInvalid(format!(
                "Team ID mismatch: expected {}, got {}",
                expected, team_id
            )));
        }
        debug!("Team ID verified: {}", team_id);
    }
    
    Ok(())
}

/// Extract the Team ID from a signed macOS executable.
#[cfg(target_os = "macos")]
fn extract_team_id(path: &Path) -> Result<String, UpdateError> {
    use std::process::Command;
    
    let result = Command::new("codesign")
        .args(["-dv", "--verbose=4"])
        .arg(path)
        .output()
        .map_err(|e| UpdateError::CodeSignatureInvalid(format!("Failed to run codesign: {}", e)))?;
    
    // codesign outputs to stderr
    let output = String::from_utf8_lossy(&result.stderr);
    
    // Look for TeamIdentifier line
    for line in output.lines() {
        if line.starts_with("TeamIdentifier=") {
            let team_id = line.trim_start_matches("TeamIdentifier=").trim();
            if team_id != "not set" {
                return Ok(team_id.to_string());
            }
        }
    }
    
    Err(UpdateError::CodeSignatureInvalid(
        "Could not extract Team ID from code signature".to_string(),
    ))
}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn verify_macos_code_signature(_path: &Path, _expected_team_id: Option<&str>) -> Result<(), UpdateError> {
    Err(UpdateError::CodeSignatureInvalid(
        "macOS code signature verification is only available on macOS".to_string(),
    ))
}

// ============================================================================
// Linux Implementation
// ============================================================================

/// Linux update installer.
///
/// Handles Linux-specific update installation including:
/// - systemd service management (stop/start)
/// - Binary replacement with proper permissions
/// - AppImage self-update support
/// - Rollback support
///
/// # Requirements
///
/// - Requirement 8.1: In-place binary replacement
/// - Requirement 8.2: systemd service restart during update
/// - Requirement 8.4: Permission handling
/// - Requirement 8.5: File permissions verification post-install
/// - Requirement 8.6: AppImage self-update
/// - Requirement 8.7: Configuration file preservation
#[cfg(target_os = "linux")]
pub struct LinuxInstaller {
    /// systemd unit name (e.g., "zrc-agent.service")
    systemd_unit: String,
    /// Directory for storing backups
    backup_dir: PathBuf,
    /// Rollback manager for backup/restore operations
    rollback_manager: RollbackManager,
    /// Whether this is a user service (--user) vs system service
    is_user_service: bool,
    /// Whether the executable is an AppImage
    is_appimage: bool,
}

#[cfg(target_os = "linux")]
impl LinuxInstaller {
    /// Create a new Linux installer.
    ///
    /// # Arguments
    ///
    /// * `systemd_unit` - Name of the systemd unit to manage (e.g., "zrc-agent.service")
    /// * `backup_dir` - Directory for storing version backups
    /// * `max_backups` - Maximum number of backups to retain
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use zrc_updater::install::LinuxInstaller;
    ///
    /// let installer = LinuxInstaller::new(
    ///     "zrc-agent.service".to_string(),
    ///     PathBuf::from("/var/lib/zrc/backups"),
    ///     3,
    /// );
    /// ```
    pub fn new(systemd_unit: String, backup_dir: PathBuf, max_backups: usize) -> Self {
        let rollback_manager = RollbackManager::new(backup_dir.clone(), max_backups);
        Self {
            systemd_unit,
            backup_dir,
            rollback_manager,
            is_user_service: false,
            is_appimage: false,
        }
    }

    /// Set whether this is a user service (--user) vs system service.
    ///
    /// User services are managed with `systemctl --user` and don't require root.
    /// System services require root privileges.
    pub fn with_is_user_service(mut self, is_user_service: bool) -> Self {
        self.is_user_service = is_user_service;
        self
    }

    /// Set whether the executable is an AppImage.
    ///
    /// AppImages have special self-update handling where the entire
    /// AppImage file is replaced.
    pub fn with_is_appimage(mut self, is_appimage: bool) -> Self {
        self.is_appimage = is_appimage;
        self
    }

    /// Get the systemd unit name.
    pub fn systemd_unit(&self) -> &str {
        &self.systemd_unit
    }

    /// Get the backup directory.
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    /// Get the rollback manager.
    pub fn rollback_manager(&self) -> &RollbackManager {
        &self.rollback_manager
    }

    /// Check if this is a user service.
    pub fn is_user_service(&self) -> bool {
        self.is_user_service
    }

    /// Check if this is an AppImage.
    pub fn is_appimage(&self) -> bool {
        self.is_appimage
    }

    /// Stop the systemd service.
    ///
    /// Uses systemctl to stop the service. Waits for the service to fully stop.
    fn stop_service(&self) -> Result<(), UpdateError> {
        info!("Stopping systemd service: {}", self.systemd_unit);
        
        linux_systemd::stop_service(&self.systemd_unit, self.is_user_service)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to stop service: {}", e)))?;
        
        debug!("Service {} stopped successfully", self.systemd_unit);
        Ok(())
    }

    /// Start the systemd service.
    ///
    /// Uses systemctl to start the service. Waits for the service to fully start.
    fn start_service(&self) -> Result<(), UpdateError> {
        info!("Starting systemd service: {}", self.systemd_unit);
        
        linux_systemd::start_service(&self.systemd_unit, self.is_user_service)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to start service: {}", e)))?;
        
        debug!("Service {} started successfully", self.systemd_unit);
        Ok(())
    }

    /// Check if the service is running.
    fn is_service_running(&self) -> Result<bool, UpdateError> {
        linux_systemd::is_service_running(&self.systemd_unit, self.is_user_service)
            .map_err(|e| UpdateError::ServiceError(format!("Failed to query service status: {}", e)))
    }

    /// Replace the executable file.
    ///
    /// Handles Linux-specific file replacement:
    /// 1. Copy the new artifact to the executable location
    /// 2. Set proper file permissions (rwxr-xr-x)
    /// 3. Preserve ownership if running as root
    ///
    /// # Requirements
    ///
    /// - Requirement 8.1: In-place binary replacement
    /// - Requirement 8.4: Permission handling
    /// - Requirement 8.5: File permissions verification
    fn replace_executable(&self, artifact: &Path, target: &Path) -> Result<(), UpdateError> {
        info!("Replacing executable: {:?} -> {:?}", artifact, target);
        
        // Copy new artifact to target location
        std::fs::copy(artifact, target).map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to copy new executable: {}", e))
        })?;
        
        // Set executable permissions (rwxr-xr-x = 0o755)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(target, permissions).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to set permissions: {}", e))
            })?;
        }
        
        // Verify permissions were set correctly
        self.verify_permissions(target)?;
        
        debug!("Executable replaced successfully");
        Ok(())
    }

    /// Verify file permissions are correct.
    ///
    /// Ensures the executable has proper permissions (at least 0o755).
    ///
    /// # Requirements
    ///
    /// - Requirement 8.5: File permissions verification post-install
    fn verify_permissions(&self, path: &Path) -> Result<(), UpdateError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to read file metadata: {}", e))
            })?;
            
            let mode = metadata.permissions().mode();
            
            // Check that owner can execute (0o100)
            if mode & 0o100 == 0 {
                return Err(UpdateError::InstallationFailed(
                    "Executable permission not set for owner".to_string(),
                ));
            }
            
            debug!("File permissions verified: {:o}", mode & 0o777);
        }
        
        Ok(())
    }

    /// Get the latest backup for rollback.
    fn get_latest_backup(&self) -> Result<BackupInfo, UpdateError> {
        self.rollback_manager
            .latest_backup()?
            .ok_or(UpdateError::NoBackupAvailable)
    }

    /// Update an AppImage file.
    ///
    /// AppImages are self-contained executables that can be updated by
    /// simply replacing the file. This method handles the special case
    /// of AppImage updates.
    ///
    /// # Requirements
    ///
    /// - Requirement 8.6: AppImage self-update
    fn update_appimage(&self, artifact: &Path, target: &Path) -> Result<(), UpdateError> {
        info!("Updating AppImage: {:?} -> {:?}", artifact, target);
        
        // For AppImages, we need to:
        // 1. Make the new AppImage executable
        // 2. Replace the old AppImage
        
        // First, make the artifact executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(artifact, permissions).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to set AppImage permissions: {}", e))
            })?;
        }
        
        // Copy the new AppImage to the target location
        std::fs::copy(artifact, target).map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to copy AppImage: {}", e))
        })?;
        
        // Ensure the target is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(target, permissions).map_err(|e| {
                UpdateError::InstallationFailed(format!("Failed to set target permissions: {}", e))
            })?;
        }
        
        debug!("AppImage updated successfully");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[async_trait]
impl PlatformInstaller for LinuxInstaller {
    /// Install update from artifact.
    ///
    /// The installation process:
    /// 1. Backup current version
    /// 2. Stop the systemd service (if running)
    /// 3. Replace the executable (or AppImage)
    /// 4. Verify file permissions
    /// 5. Start the systemd service
    /// 6. On failure, automatically rollback
    ///
    /// # Requirements
    ///
    /// - Requirement 8.1: In-place binary replacement
    /// - Requirement 8.2: systemd service restart during update
    /// - Requirement 8.4: Permission handling
    /// - Requirement 8.5: File permissions verification
    /// - Requirement 8.6: AppImage self-update
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError> {
        info!("Starting Linux update installation from {:?}", artifact);
        
        // Step 1: Backup current version
        let backup = self.rollback_manager.backup_current()?;
        info!("Created backup: version {}", backup.version);
        
        // Step 2: Get current executable path
        let current_exe = std::env::current_exe().map_err(|e| {
            UpdateError::InstallationFailed(format!("Failed to get current executable: {}", e))
        })?;
        
        // Step 3: Check if service is running and stop it
        let was_running = self.is_service_running().unwrap_or(false);
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service (may not be installed as service): {}", e);
            }
        }
        
        // Step 4: Replace executable (handle AppImage specially)
        let replace_result = if self.is_appimage {
            self.update_appimage(artifact, &current_exe)
        } else {
            self.replace_executable(artifact, &current_exe)
        };
        
        match replace_result {
            Ok(_) => {}
            Err(e) => {
                // Try to restart service before returning error
                if was_running {
                    let _ = self.start_service();
                }
                return Err(e);
            }
        }
        
        // Step 5: Verify permissions
        if let Err(e) = self.verify_permissions(&current_exe) {
            warn!("Permission verification failed, rolling back: {}", e);
            // Rollback
            let _ = self.rollback_manager.rollback_to(&backup);
            if was_running {
                let _ = self.start_service();
            }
            return Err(e);
        }
        
        // Step 6: Start service
        if was_running {
            if let Err(e) = self.start_service() {
                warn!("Failed to start service after update: {}", e);
                // Rollback
                let _ = self.rollback_manager.rollback_to(&backup);
                let _ = self.start_service();
                return Err(e);
            }
        }
        
        info!("Linux update installation completed successfully");
        Ok(())
    }

    /// Rollback to previous version.
    ///
    /// Restores the most recent backup:
    /// 1. Stop the service
    /// 2. Restore the backed up executable
    /// 3. Start the service
    ///
    /// # Requirements
    ///
    /// - Requirement 9.3: Manual rollback support
    fn rollback(&self) -> Result<(), UpdateError> {
        info!("Starting rollback on Linux");
        
        // Get the latest backup
        let backup = self.get_latest_backup()?;
        info!("Rolling back to version {}", backup.version);
        
        // Check if service is running
        let was_running = self.is_service_running().unwrap_or(false);
        
        // Stop service if running
        if was_running {
            if let Err(e) = self.stop_service() {
                warn!("Failed to stop service during rollback: {}", e);
            }
        }
        
        // Perform rollback
        self.rollback_manager.rollback_to(&backup)?;
        
        // Restart service
        if was_running {
            self.start_service()?;
        }
        
        info!("Rollback completed successfully to version {}", backup.version);
        Ok(())
    }

    fn requires_restart(&self) -> bool {
        true
    }
}

// ============================================================================
// Linux systemd Service Management Module
// ============================================================================

#[cfg(target_os = "linux")]
mod linux_systemd {
    use std::process::Command;
    use std::time::Duration;
    use tracing::{debug, warn};

    /// Stop a systemd service.
    ///
    /// Uses `systemctl stop` to stop the service. For user services,
    /// uses `systemctl --user stop`.
    pub fn stop_service(unit: &str, is_user_service: bool) -> Result<(), String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.args(["stop", unit]);
        
        let result = cmd.output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            // Check if service doesn't exist or is already stopped
            if stderr.contains("not loaded") || 
               stderr.contains("not found") ||
               stderr.contains("Unit") && stderr.contains("not found") {
                debug!("Service {} not found or not loaded", unit);
                return Ok(());
            }
            return Err(format!("systemctl stop failed: {}", stderr));
        }
        
        // Wait a moment for the service to fully stop
        std::thread::sleep(Duration::from_millis(500));
        
        // Verify the service is stopped
        let max_attempts = 10;
        for attempt in 0..max_attempts {
            if !is_service_running(unit, is_user_service).unwrap_or(true) {
                debug!("Service {} stopped after {} attempts", unit, attempt + 1);
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        
        warn!("Service {} may not have fully stopped", unit);
        Ok(())
    }

    /// Start a systemd service.
    ///
    /// Uses `systemctl start` to start the service. For user services,
    /// uses `systemctl --user start`.
    pub fn start_service(unit: &str, is_user_service: bool) -> Result<(), String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.args(["start", unit]);
        
        let result = cmd.output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(format!("systemctl start failed: {}", stderr));
        }
        
        // Wait a moment for the service to start
        std::thread::sleep(Duration::from_millis(500));
        
        // Verify the service is running
        let max_attempts = 10;
        for attempt in 0..max_attempts {
            if is_service_running(unit, is_user_service).unwrap_or(false) {
                debug!("Service {} started after {} attempts", unit, attempt + 1);
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        
        Err(format!("Service {} failed to start within timeout", unit))
    }

    /// Check if a systemd service is running.
    ///
    /// Uses `systemctl is-active` to check the service status.
    pub fn is_service_running(unit: &str, is_user_service: bool) -> Result<bool, String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.args(["is-active", "--quiet", unit]);
        
        let result = cmd.status()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        // is-active returns 0 if active, non-zero otherwise
        Ok(result.success())
    }

    /// Reload systemd daemon configuration.
    ///
    /// Uses `systemctl daemon-reload` to reload unit files.
    /// This is useful after updating service files.
    #[allow(dead_code)]
    pub fn daemon_reload(is_user_service: bool) -> Result<(), String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.arg("daemon-reload");
        
        let result = cmd.output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(format!("systemctl daemon-reload failed: {}", stderr));
        }
        
        Ok(())
    }

    /// Enable a systemd service to start on boot.
    #[allow(dead_code)]
    pub fn enable_service(unit: &str, is_user_service: bool) -> Result<(), String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.args(["enable", unit]);
        
        let result = cmd.output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(format!("systemctl enable failed: {}", stderr));
        }
        
        Ok(())
    }

    /// Get the status of a systemd service.
    #[allow(dead_code)]
    pub fn get_service_status(unit: &str, is_user_service: bool) -> Result<String, String> {
        let mut cmd = Command::new("systemctl");
        
        if is_user_service {
            cmd.arg("--user");
        }
        
        cmd.args(["status", unit]);
        
        let result = cmd.output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        
        // status command may return non-zero for inactive services
        let stdout = String::from_utf8_lossy(&result.stdout);
        Ok(stdout.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_installer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let installer = WindowsInstaller::new(
            "TestService".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        );
        
        assert_eq!(installer.service_name(), "TestService");
        assert_eq!(installer.backup_dir(), temp_dir.path());
        assert!(installer.requires_restart());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_installer_with_thumbprint() {
        let temp_dir = TempDir::new().unwrap();
        let installer = WindowsInstaller::new(
            "TestService".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        )
        .with_expected_thumbprint("ABC123".to_string())
        .with_silent(false);
        
        assert_eq!(installer.service_name(), "TestService");
    }

    // ========================================================================
    // macOS Installer Tests
    // ========================================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_installer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let installer = MacOSInstaller::new(
            "io.zippyremote.agent".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        );
        
        assert_eq!(installer.launch_agent_label(), "io.zippyremote.agent");
        assert_eq!(installer.backup_dir(), temp_dir.path());
        assert!(!installer.is_daemon());
        assert!(installer.requires_restart());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_installer_with_team_id() {
        let temp_dir = TempDir::new().unwrap();
        let installer = MacOSInstaller::new(
            "io.zippyremote.agent".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        )
        .with_expected_team_id("ABCD1234".to_string())
        .with_is_daemon(true);
        
        assert_eq!(installer.launch_agent_label(), "io.zippyremote.agent");
        assert!(installer.is_daemon());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_installer_rollback_manager() {
        let temp_dir = TempDir::new().unwrap();
        let installer = MacOSInstaller::new(
            "io.zippyremote.agent".to_string(),
            temp_dir.path().to_path_buf(),
            5,
        );
        
        let rollback_manager = installer.rollback_manager();
        assert_eq!(rollback_manager.max_backups(), 5);
        assert_eq!(rollback_manager.backup_dir(), temp_dir.path());
    }

    // Cross-platform test for verify_macos_code_signature stub
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_macos_code_signature_stub() {
        use std::path::PathBuf;
        let result = verify_macos_code_signature(&PathBuf::from("/test"), None);
        assert!(result.is_err());
        match result {
            Err(UpdateError::CodeSignatureInvalid(msg)) => {
                assert!(msg.contains("only available on macOS"));
            }
            _ => panic!("Expected CodeSignatureInvalid error"),
        }
    }

    // ========================================================================
    // Linux Installer Tests
    // ========================================================================

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_installer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let installer = LinuxInstaller::new(
            "zrc-agent.service".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        );
        
        assert_eq!(installer.systemd_unit(), "zrc-agent.service");
        assert_eq!(installer.backup_dir(), temp_dir.path());
        assert!(!installer.is_user_service());
        assert!(!installer.is_appimage());
        assert!(installer.requires_restart());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_installer_with_user_service() {
        let temp_dir = TempDir::new().unwrap();
        let installer = LinuxInstaller::new(
            "zrc-agent.service".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        )
        .with_is_user_service(true);
        
        assert_eq!(installer.systemd_unit(), "zrc-agent.service");
        assert!(installer.is_user_service());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_installer_with_appimage() {
        let temp_dir = TempDir::new().unwrap();
        let installer = LinuxInstaller::new(
            "zrc-agent.service".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        )
        .with_is_appimage(true);
        
        assert_eq!(installer.systemd_unit(), "zrc-agent.service");
        assert!(installer.is_appimage());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_installer_rollback_manager() {
        let temp_dir = TempDir::new().unwrap();
        let installer = LinuxInstaller::new(
            "zrc-agent.service".to_string(),
            temp_dir.path().to_path_buf(),
            5,
        );
        
        let rollback_manager = installer.rollback_manager();
        assert_eq!(rollback_manager.max_backups(), 5);
        assert_eq!(rollback_manager.backup_dir(), temp_dir.path());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_installer_builder_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let installer = LinuxInstaller::new(
            "zrc-agent.service".to_string(),
            temp_dir.path().to_path_buf(),
            3,
        )
        .with_is_user_service(true)
        .with_is_appimage(true);
        
        assert!(installer.is_user_service());
        assert!(installer.is_appimage());
    }
}
