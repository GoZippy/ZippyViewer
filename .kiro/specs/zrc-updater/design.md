# Design Document: zrc-updater

## Overview

The zrc-updater crate implements secure automatic updates for the ZRC system. It handles update checking, downloading, verification, and installation across all supported platforms with security as the primary concern.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            zrc-updater                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Update Flow                                      │   │
│  │                                                                        │   │
│  │  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐           │   │
│  │  │  Check  │───►│Download │───►│ Verify  │───►│ Install │           │   │
│  │  │ Manifest│    │ Artifact│    │Signature│    │ Update  │           │   │
│  │  └─────────┘    └─────────┘    └─────────┘    └─────────┘           │   │
│  │       │              │              │              │                  │   │
│  │       ▼              ▼              ▼              ▼                  │   │
│  │  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐           │   │
│  │  │ Verify  │    │ Verify  │    │ Verify  │    │ Backup  │           │   │
│  │  │Manifest │    │  Hash   │    │  Code   │    │ Current │           │   │
│  │  │Signature│    │         │    │Signature│    │ Version │           │   │
│  │  └─────────┘    └─────────┘    └─────────┘    └─────────┘           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Platform Installers                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Windows   │  │    macOS    │  │    Linux    │                  │   │
│  │  │  Installer  │  │  Installer  │  │  Installer  │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```


## Components and Interfaces

### Update Manager

```rust
/// Main update manager
pub struct UpdateManager {
    config: UpdateConfig,
    manifest_verifier: ManifestVerifier,
    artifact_verifier: ArtifactVerifier,
    downloader: Downloader,
    installer: Box<dyn PlatformInstaller>,
    state: UpdateState,
}

impl UpdateManager {
    /// Check for available updates
    pub async fn check_for_updates(&self) -> Result<Option<UpdateInfo>, UpdateError> {
        let manifest_url = self.config.manifest_url_for_channel();
        
        // Download manifest
        let manifest_bytes = self.downloader.fetch(&manifest_url).await?;
        
        // Verify manifest signature
        let manifest = self.manifest_verifier.verify_and_parse(&manifest_bytes)?;
        
        // Check if update is available
        let current_version = self.current_version();
        if manifest.version > current_version {
            Ok(Some(UpdateInfo {
                version: manifest.version.clone(),
                release_notes: manifest.release_notes.clone(),
                size: manifest.artifact_size,
                is_security_update: manifest.is_security_update,
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Download and install update
    pub async fn install_update(&mut self, info: &UpdateInfo) -> Result<(), UpdateError> {
        // Backup current version
        self.backup_current()?;
        
        // Download artifact
        let artifact_path = self.download_artifact(info).await?;
        
        // Verify artifact
        self.artifact_verifier.verify(&artifact_path, &info.expected_hash)?;
        
        // Install
        match self.installer.install(&artifact_path).await {
            Ok(_) => {
                self.cleanup_backup()?;
                Ok(())
            }
            Err(e) => {
                // Rollback on failure
                self.rollback()?;
                Err(e)
            }
        }
    }
    
    /// Rollback to previous version
    pub fn rollback(&self) -> Result<(), UpdateError> {
        self.installer.rollback()
    }
}

pub struct UpdateInfo {
    pub version: Version,
    pub release_notes: String,
    pub size: u64,
    pub is_security_update: bool,
    pub expected_hash: [u8; 32],
}
```

### Manifest Verifier

```rust
/// Verifies update manifest signatures
pub struct ManifestVerifier {
    /// Pinned public keys for manifest signing
    trusted_keys: Vec<VerifyingKey>,
    /// Minimum required signatures
    threshold: usize,
}

impl ManifestVerifier {
    /// Create verifier with pinned keys
    pub fn new(trusted_keys: Vec<VerifyingKey>, threshold: usize) -> Self {
        Self { trusted_keys, threshold }
    }
    
    /// Verify manifest signature and parse
    pub fn verify_and_parse(&self, data: &[u8]) -> Result<UpdateManifest, UpdateError> {
        let signed_manifest: SignedManifest = serde_json::from_slice(data)?;
        
        // Verify timestamp is recent (within 7 days)
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if signed_manifest.timestamp < now - 7 * 24 * 3600 {
            return Err(UpdateError::ManifestTooOld);
        }
        if signed_manifest.timestamp > now + 3600 {
            return Err(UpdateError::ManifestFromFuture);
        }
        
        // Verify signatures
        let manifest_bytes = signed_manifest.manifest.as_bytes();
        let mut valid_signatures = 0;
        
        for sig in &signed_manifest.signatures {
            for key in &self.trusted_keys {
                if key.verify(manifest_bytes, &sig.signature).is_ok() {
                    valid_signatures += 1;
                    break;
                }
            }
        }
        
        if valid_signatures < self.threshold {
            return Err(UpdateError::InsufficientSignatures {
                required: self.threshold,
                found: valid_signatures,
            });
        }
        
        // Parse manifest
        let manifest: UpdateManifest = serde_json::from_str(&signed_manifest.manifest)?;
        
        // Verify platform matches
        if manifest.platform != current_platform() {
            return Err(UpdateError::PlatformMismatch);
        }
        
        Ok(manifest)
    }
}

#[derive(Deserialize)]
pub struct SignedManifest {
    pub manifest: String,  // JSON string of UpdateManifest
    pub signatures: Vec<ManifestSignature>,
    pub timestamp: u64,
}

#[derive(Deserialize)]
pub struct ManifestSignature {
    pub key_id: String,
    pub signature: Signature,
}

#[derive(Deserialize)]
pub struct UpdateManifest {
    pub version: Version,
    pub platform: String,
    pub channel: UpdateChannel,
    pub artifact_url: String,
    pub artifact_hash: String,  // SHA-256 hex
    pub artifact_size: u64,
    pub release_notes: String,
    pub is_security_update: bool,
    pub min_version: Option<Version>,  // Minimum version for delta update
}
```

### Artifact Verifier

```rust
/// Verifies downloaded artifacts
pub struct ArtifactVerifier {
    /// Optional code signing verification
    code_signer_verifier: Option<CodeSignerVerifier>,
}

impl ArtifactVerifier {
    /// Verify artifact integrity and authenticity
    pub fn verify(&self, path: &Path, expected_hash: &[u8; 32]) -> Result<(), UpdateError> {
        // Verify hash
        let actual_hash = self.compute_hash(path)?;
        if !constant_time_eq(&actual_hash, expected_hash) {
            return Err(UpdateError::HashMismatch {
                expected: hex::encode(expected_hash),
                actual: hex::encode(&actual_hash),
            });
        }
        
        // Verify code signature (platform-specific)
        if let Some(verifier) = &self.code_signer_verifier {
            verifier.verify(path)?;
        }
        
        Ok(())
    }
    
    fn compute_hash(&self, path: &Path) -> Result<[u8; 32], UpdateError> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }
        
        Ok(hasher.finalize().into())
    }
}

/// Platform-specific code signature verification
pub trait CodeSignerVerifier: Send + Sync {
    fn verify(&self, path: &Path) -> Result<(), UpdateError>;
}

#[cfg(target_os = "windows")]
pub struct WindowsCodeVerifier {
    expected_thumbprint: String,
}

#[cfg(target_os = "windows")]
impl CodeSignerVerifier for WindowsCodeVerifier {
    fn verify(&self, path: &Path) -> Result<(), UpdateError> {
        // Use WinVerifyTrust API
        // Verify Authenticode signature
        // Check certificate thumbprint matches expected
        todo!()
    }
}

#[cfg(target_os = "macos")]
pub struct MacOSCodeVerifier {
    expected_team_id: String,
}

#[cfg(target_os = "macos")]
impl CodeSignerVerifier for MacOSCodeVerifier {
    fn verify(&self, path: &Path) -> Result<(), UpdateError> {
        // Use codesign -v to verify
        // Check notarization status
        // Verify team ID matches
        todo!()
    }
}
```

### Downloader

```rust
/// Secure artifact downloader
pub struct Downloader {
    client: reqwest::Client,
    progress_callback: Option<Box<dyn Fn(DownloadProgress) + Send>>,
}

impl Downloader {
    /// Download with progress reporting and resume support
    pub async fn download_with_resume(
        &self,
        url: &str,
        dest: &Path,
        expected_size: u64,
    ) -> Result<(), UpdateError> {
        let mut file = if dest.exists() {
            // Resume download
            let existing_size = dest.metadata()?.len();
            if existing_size >= expected_size {
                return Ok(());  // Already complete
            }
            
            OpenOptions::new().append(true).open(dest)?
        } else {
            File::create(dest)?
        };
        
        let start_byte = file.metadata()?.len();
        
        let response = self.client
            .get(url)
            .header("Range", format!("bytes={}-", start_byte))
            .send()
            .await?;
        
        if !response.status().is_success() && response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(UpdateError::DownloadFailed {
                status: response.status().as_u16(),
            });
        }
        
        let mut stream = response.bytes_stream();
        let mut downloaded = start_byte;
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            if let Some(callback) = &self.progress_callback {
                callback(DownloadProgress {
                    downloaded,
                    total: expected_size,
                });
            }
        }
        
        Ok(())
    }
}

pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
}
```

### Platform Installers

```rust
/// Platform-specific installation
#[async_trait]
pub trait PlatformInstaller: Send + Sync {
    /// Install update from artifact
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError>;
    
    /// Rollback to previous version
    fn rollback(&self) -> Result<(), UpdateError>;
    
    /// Check if restart is required
    fn requires_restart(&self) -> bool;
}

#[cfg(target_os = "windows")]
pub struct WindowsInstaller {
    service_name: String,
    backup_dir: PathBuf,
}

#[cfg(target_os = "windows")]
#[async_trait]
impl PlatformInstaller for WindowsInstaller {
    async fn install(&self, artifact: &Path) -> Result<(), UpdateError> {
        // Stop service
        stop_windows_service(&self.service_name)?;
        
        // Replace executable
        let exe_path = std::env::current_exe()?;
        let backup_path = self.backup_dir.join("zrc-agent.exe.bak");
        
        std::fs::rename(&exe_path, &backup_path)?;
        std::fs::copy(artifact, &exe_path)?;
        
        // Verify new executable signature
        verify_authenticode(&exe_path)?;
        
        // Start service
        start_windows_service(&self.service_name)?;
        
        Ok(())
    }
    
    fn rollback(&self) -> Result<(), UpdateError> {
        let exe_path = std::env::current_exe()?;
        let backup_path = self.backup_dir.join("zrc-agent.exe.bak");
        
        if backup_path.exists() {
            stop_windows_service(&self.service_name)?;
            std::fs::rename(&backup_path, &exe_path)?;
            start_windows_service(&self.service_name)?;
        }
        
        Ok(())
    }
    
    fn requires_restart(&self) -> bool {
        true
    }
}

#[cfg(target_os = "macos")]
pub struct MacOSInstaller {
    launch_agent_label: String,
    backup_dir: PathBuf,
}

#[cfg(target_os = "linux")]
pub struct LinuxInstaller {
    systemd_unit: String,
    backup_dir: PathBuf,
}
```

### Update Channels

```rust
/// Update channel management
pub struct ChannelManager {
    current_channel: UpdateChannel,
    config_path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
    Custom(String),
}

impl ChannelManager {
    /// Get manifest URL for current channel
    pub fn manifest_url(&self) -> String {
        match &self.current_channel {
            UpdateChannel::Stable => "https://updates.zippyremote.io/stable/manifest.json".into(),
            UpdateChannel::Beta => "https://updates.zippyremote.io/beta/manifest.json".into(),
            UpdateChannel::Nightly => "https://updates.zippyremote.io/nightly/manifest.json".into(),
            UpdateChannel::Custom(url) => url.clone(),
        }
    }
    
    /// Switch update channel
    pub fn set_channel(&mut self, channel: UpdateChannel) -> Result<(), UpdateError> {
        // Warn if switching to less stable channel
        if self.is_downgrade(&channel) {
            log::warn!("Switching to less stable update channel: {:?}", channel);
        }
        
        self.current_channel = channel;
        self.save_config()?;
        Ok(())
    }
    
    fn is_downgrade(&self, new_channel: &UpdateChannel) -> bool {
        let stability = |c: &UpdateChannel| match c {
            UpdateChannel::Stable => 3,
            UpdateChannel::Beta => 2,
            UpdateChannel::Nightly => 1,
            UpdateChannel::Custom(_) => 0,
        };
        stability(new_channel) < stability(&self.current_channel)
    }
}
```

### Rollback Manager

```rust
/// Manages version backups for rollback
pub struct RollbackManager {
    backup_dir: PathBuf,
    max_backups: usize,
}

impl RollbackManager {
    /// Backup current version before update
    pub fn backup_current(&self) -> Result<BackupInfo, UpdateError> {
        let current_exe = std::env::current_exe()?;
        let current_version = self.detect_version(&current_exe)?;
        
        let backup_name = format!("backup-{}-{}", current_version, Utc::now().timestamp());
        let backup_path = self.backup_dir.join(&backup_name);
        
        std::fs::create_dir_all(&backup_path)?;
        std::fs::copy(&current_exe, backup_path.join("executable"))?;
        
        // Save metadata
        let info = BackupInfo {
            version: current_version,
            created_at: Utc::now(),
            path: backup_path.clone(),
        };
        
        let metadata_path = backup_path.join("metadata.json");
        std::fs::write(&metadata_path, serde_json::to_string(&info)?)?;
        
        // Cleanup old backups
        self.cleanup_old_backups()?;
        
        Ok(info)
    }
    
    /// List available backups
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, UpdateError> {
        let mut backups = Vec::new();
        
        for entry in std::fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let metadata_path = entry.path().join("metadata.json");
            if metadata_path.exists() {
                let info: BackupInfo = serde_json::from_str(
                    &std::fs::read_to_string(&metadata_path)?
                )?;
                backups.push(info);
            }
        }
        
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }
    
    /// Rollback to specific backup
    pub fn rollback_to(&self, backup: &BackupInfo) -> Result<(), UpdateError> {
        let backup_exe = backup.path.join("executable");
        let current_exe = std::env::current_exe()?;
        
        // Verify backup integrity
        if !backup_exe.exists() {
            return Err(UpdateError::BackupCorrupted);
        }
        
        std::fs::copy(&backup_exe, &current_exe)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct BackupInfo {
    pub version: Version,
    pub created_at: DateTime<Utc>,
    pub path: PathBuf,
}
```

## Data Models

### Configuration

```toml
# updater.toml

[update]
channel = "stable"
check_interval_hours = 24
auto_download = true
auto_install = false

[security]
manifest_keys = [
    "ed25519:abc123...",  # Primary key
    "ed25519:def456...",  # Backup key
]
signature_threshold = 1
verify_code_signature = true

[rollback]
max_backups = 3
backup_dir = ""  # Empty = default location

[network]
timeout_seconds = 30
max_retries = 3
proxy = ""  # Empty = system proxy
```

## Correctness Properties

### Property 1: Manifest Signature Verification
*For any* update manifest, at least `threshold` valid signatures from trusted keys SHALL be verified before processing.
**Validates: Requirements 1.1, 1.2, 1.6**

### Property 2: Artifact Hash Verification
*For any* downloaded artifact, the SHA-256 hash SHALL match the hash specified in the signed manifest.
**Validates: Requirements 2.1, 2.2, 2.5**

### Property 3: Rollback Availability
*For any* update installation, a backup of the previous version SHALL exist until the update is confirmed successful.
**Validates: Requirements 9.1, 9.2, 9.3**

### Property 4: No Pre-Verification Execution
*For any* downloaded artifact, no code from the artifact SHALL be executed before signature and hash verification.
**Validates: Requirement 12.4**

### Property 5: Channel Isolation
*For any* update check, only manifests from the configured channel SHALL be accepted.
**Validates: Requirements 3.1, 3.7**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| Manifest signature invalid | Reject update | Retry later |
| Artifact hash mismatch | Delete artifact | Re-download |
| Download interrupted | Resume download | Automatic |
| Installation failed | Rollback | Automatic |
| Service restart failed | Log error | Manual intervention |
| Backup creation failed | Abort update | Notify user |

## Testing Strategy

### Unit Tests
- Manifest signature verification
- Hash computation
- Version comparison
- Channel URL generation

### Integration Tests
- Full update flow with mock server
- Rollback scenarios
- Resume download
- Platform-specific installation

### Security Tests
- Invalid signature rejection
- Hash mismatch detection
- Tampered manifest detection
- Downgrade attack prevention
