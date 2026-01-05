# Implementation Plan: zrc-updater

## Overview

Implementation tasks for the secure automatic update system. This crate handles update checking, downloading, verification, and installation across all supported platforms with security as the primary concern.

## Tasks

- [x] 1. Set up crate structure
  - [x] 1.1 Create Cargo.toml with dependencies
    - Add reqwest, sha2, ed25519-dalek, semver
    - Add platform-specific dependencies
    - _Requirements: 1.1, 2.2_
  - [x] 1.2 Create lib.rs with module structure
    - Define manifest, artifact, download, install modules
    - _Requirements: 1.1_
  - [x] 1.3 Define configuration structures
    - UpdateConfig, SecurityConfig, RollbackConfig
    - _Requirements: 3.4, 12.1_

- [x] 2. Implement manifest verification
  - [x] 2.1 Create ManifestVerifier struct
    - Pinned public keys storage
    - Signature threshold
    - _Requirements: 1.1, 1.6_
  - [x] 2.2 Implement verify_and_parse method
    - Timestamp validation
    - Multi-signature verification
    - Platform matching
    - _Requirements: 1.2, 1.3, 1.4, 1.5_
  - [x] 2.3 Define SignedManifest and UpdateManifest structs
    - _Requirements: 1.1_
  - [x]* 2.4 Write property test for manifest signature verification
    - **Property 1: Manifest Signature Verification**
    - **Validates: Requirements 1.1, 1.2, 1.6**

- [x] 3. Implement artifact verification
  - [x] 3.1 Create ArtifactVerifier struct
    - Hash computation
    - Code signature verification
    - _Requirements: 2.1, 2.2_
  - [x] 3.2 Implement verify method
    - SHA-256 hash comparison
    - Constant-time comparison
    - _Requirements: 2.1, 2.5_
  - [x] 3.3 Implement platform-specific code verifiers
    - WindowsCodeVerifier (Authenticode)
    - MacOSCodeVerifier (codesign)
    - _Requirements: 2.3_
  - [x]* 3.4 Write property test for artifact hash verification
    - **Property 2: Artifact Hash Verification**
    - **Validates: Requirements 2.1, 2.2, 2.5**

- [x] 4. Implement downloader
  - [x] 4.1 Create Downloader struct
    - reqwest client with timeout
    - Progress callback
    - _Requirements: 5.1, 5.3_
  - [x] 4.2 Implement download_with_resume method
    - Range header for resume
    - Partial download verification
    - _Requirements: 5.2, 5.7_
  - [x] 4.3 Implement progress reporting
    - DownloadProgress struct
    - _Requirements: 5.3_

- [x] 5. Implement update channels
  - [x] 5.1 Create ChannelManager struct
    - Channel persistence
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - [x] 5.2 Implement manifest_url method
    - URL per channel
    - _Requirements: 3.7_
  - [x] 5.3 Implement set_channel method
    - Downgrade warning
    - _Requirements: 3.5, 3.6_
  - [x]* 5.4 Write property test for channel isolation
    - **Property 5: Channel Isolation**
    - **Validates: Requirements 3.1, 3.7**

- [x] 6. Implement update manager
  - [x] 6.1 Create UpdateManager struct
    - Combine all components
    - _Requirements: 4.1, 4.4_
  - [x] 6.2 Implement check_for_updates method
    - Download and verify manifest
    - Compare versions
    - _Requirements: 4.1, 4.2, 4.3_
  - [x] 6.3 Implement install_update method
    - Backup, download, verify, install
    - Rollback on failure
    - _Requirements: 9.1, 9.2_

- [x] 7. Implement rollback manager
  - [x] 7.1 Create RollbackManager struct
    - Backup directory management
    - _Requirements: 9.1, 9.4_
  - [x] 7.2 Implement backup_current method
    - Copy executable and metadata
    - _Requirements: 9.1_
  - [x] 7.3 Implement list_backups method
    - _Requirements: 9.4_
  - [x] 7.4 Implement rollback_to method
    - Restore from backup
    - _Requirements: 9.3, 9.5_
  - [x]* 7.5 Write property test for rollback availability
    - **Property 3: Rollback Availability**
    - **Validates: Requirements 9.1, 9.2, 9.3**

- [x] 8. Implement Windows installer
  - [x] 8.1 Create WindowsInstaller struct
    - Service management
    - _Requirements: 6.2_
  - [x] 8.2 Implement install method
    - Stop service, replace exe, start service
    - _Requirements: 6.1, 6.5_
  - [x] 8.3 Implement rollback method
    - _Requirements: 9.3_
  - [x] 8.4 Implement Authenticode verification
    - _Requirements: 6.4_

- [x] 9. Implement macOS installer
  - [x] 9.1 Create MacOSInstaller struct
    - LaunchAgent/Daemon management
    - _Requirements: 7.2_
  - [x] 9.2 Implement install method
    - Handle authorization
    - _Requirements: 7.1, 7.3_
  - [x] 9.3 Implement code signature verification
    - _Requirements: 7.4_

- [x] 10. Implement Linux installer
  - [x] 10.1 Create LinuxInstaller struct
    - systemd service management
    - _Requirements: 8.2_
  - [x] 10.2 Implement install method
    - Binary replacement
    - _Requirements: 8.1, 8.4_
  - [x] 10.3 Implement AppImage self-update
    - _Requirements: 8.6_

- [x] 11. Implement update notifications
  - [x] 11.1 Create notification system
    - Native notifications per platform
    - _Requirements: 11.1, 11.5_
  - [x] 11.2 Implement notification content
    - Version, release notes, urgency
    - _Requirements: 11.2, 11.7_
  - [x] 11.3 Implement defer and remind
    - _Requirements: 11.3, 11.8_

- [x] 12. Implement offline updates
  - [x] 12.1 Implement update file import
    - Manual file selection
    - _Requirements: 10.1, 10.2_
  - [x] 12.2 Implement update file export
    - For distribution
    - _Requirements: 10.4_
  - [x] 12.3 Document offline update process
    - _Requirements: 10.5_

- [x] 13. Checkpoint - Verify all tests pass
  - Run all unit and integration tests
  - Test update flow on all platforms
  - Test rollback scenarios
  - Test offline updates
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- Security is paramount - no code execution before verification
- Platform-specific installers handle service/daemon management
- Rollback must always be available
