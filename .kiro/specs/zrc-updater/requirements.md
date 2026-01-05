# Requirements Document: zrc-updater

## Introduction

The zrc-updater crate implements secure automatic updates for the Zippy Remote Control (ZRC) system. This module handles update checking, downloading, verification, and installation for agent and desktop applications across all supported platforms. Security is paramount as auto-update is a common attack vector.

## Glossary

- **Update_Manifest**: Signed metadata describing available updates
- **Update_Channel**: Release track (stable, beta, nightly)
- **Delta_Update**: Incremental update containing only changes
- **Rollback**: Reverting to previous version after failed update
- **TUF**: The Update Framework, secure update specification
- **Root_Key**: Long-term key for signing update infrastructure
- **Targets_Key**: Key for signing individual update artifacts

## Requirements

### Requirement 1: Update Manifest Verification

**User Story:** As a user, I want verified updates, so that I'm protected from malicious updates.

#### Acceptance Criteria

1. THE Updater SHALL verify manifest signature using pinned public key
2. THE Updater SHALL reject manifests with invalid signatures
3. THE Updater SHALL verify manifest timestamp is recent (within 7 days)
4. THE Updater SHALL verify manifest version is newer than current
5. THE Updater SHALL verify manifest targets the correct platform
6. THE Updater SHALL support multiple signing keys for rotation
7. THE Updater SHALL log all verification failures
8. THE Updater SHALL alert user on verification failure

### Requirement 2: Artifact Verification

**User Story:** As a user, I want verified artifacts, so that downloaded files are authentic.

#### Acceptance Criteria

1. THE Updater SHALL verify artifact hash matches manifest
2. THE Updater SHALL use SHA-256 for artifact hashing
3. THE Updater SHALL verify artifact signature (optional, in addition to hash)
4. THE Updater SHALL verify artifact size matches manifest
5. THE Updater SHALL reject artifacts with mismatched hashes
6. THE Updater SHALL delete failed downloads
7. THE Updater SHALL log verification results
8. THE Updater SHALL support resumable downloads with verification

### Requirement 3: Update Channels

**User Story:** As a user, I want update channel selection, so that I can choose my update cadence.

#### Acceptance Criteria

1. THE Updater SHALL support stable channel (production releases)
2. THE Updater SHALL support beta channel (pre-release testing)
3. THE Updater SHALL support nightly channel (development builds)
4. THE Updater SHALL persist channel selection
5. THE Updater SHALL allow channel switching via settings
6. THE Updater SHALL warn when switching to less stable channel
7. THE Updater SHALL check correct manifest URL per channel
8. THE Updater SHALL support custom/enterprise channels

### Requirement 4: Update Checking

**User Story:** As a user, I want automatic update checks, so that I stay current without manual effort.

#### Acceptance Criteria

1. THE Updater SHALL check for updates on application startup
2. THE Updater SHALL check for updates periodically (configurable, default: daily)
3. THE Updater SHALL respect user preference to disable auto-check
4. THE Updater SHALL support manual update check trigger
5. THE Updater SHALL handle network failures gracefully
6. THE Updater SHALL cache update check results
7. THE Updater SHALL minimize bandwidth for update checks
8. THE Updater SHALL work behind proxies

### Requirement 5: Update Download

**User Story:** As a user, I want efficient downloads, so that updates don't consume excessive bandwidth.

#### Acceptance Criteria

1. THE Updater SHALL download updates in background
2. THE Updater SHALL support download pause/resume
3. THE Updater SHALL show download progress
4. THE Updater SHALL support delta updates (optional)
5. THE Updater SHALL respect bandwidth limits (configurable)
6. THE Updater SHALL use HTTPS for all downloads
7. THE Updater SHALL verify partial downloads before resume
8. THE Updater SHALL clean up incomplete downloads

### Requirement 6: Update Installation - Windows

**User Story:** As a Windows user, I want seamless updates, so that updates install without issues.

#### Acceptance Criteria

1. THE Updater SHALL support MSI-based installation
2. THE Updater SHALL handle service restart during update
3. THE Updater SHALL request elevation when needed
4. THE Updater SHALL verify Windows code signature post-install
5. THE Updater SHALL support silent installation
6. THE Updater SHALL handle files in use
7. THE Updater SHALL create restore point before update (optional)
8. THE Updater SHALL log installation events to Event Log

### Requirement 7: Update Installation - macOS

**User Story:** As a macOS user, I want seamless updates, so that updates install correctly.

#### Acceptance Criteria

1. THE Updater SHALL support .pkg or app bundle replacement
2. THE Updater SHALL handle LaunchAgent/Daemon restart
3. THE Updater SHALL request authorization when needed
4. THE Updater SHALL verify code signature and notarization post-install
5. THE Updater SHALL support Sparkle-compatible updates (optional)
6. THE Updater SHALL handle quarantine attribute
7. THE Updater SHALL preserve user preferences
8. THE Updater SHALL log installation events

### Requirement 8: Update Installation - Linux

**User Story:** As a Linux user, I want seamless updates, so that updates work with my system.

#### Acceptance Criteria

1. THE Updater SHALL support in-place binary replacement
2. THE Updater SHALL handle systemd service restart
3. THE Updater SHALL support package manager integration (optional)
4. THE Updater SHALL handle permission requirements
5. THE Updater SHALL verify file permissions post-install
6. THE Updater SHALL support AppImage self-update
7. THE Updater SHALL preserve configuration files
8. THE Updater SHALL log installation events to syslog

### Requirement 9: Rollback Support

**User Story:** As a user, I want rollback capability, so that I can recover from bad updates.

#### Acceptance Criteria

1. THE Updater SHALL backup current version before update
2. THE Updater SHALL support automatic rollback on update failure
3. THE Updater SHALL support manual rollback to previous version
4. THE Updater SHALL retain one previous version minimum
5. THE Updater SHALL verify rollback integrity
6. THE Updater SHALL log rollback events
7. THE Updater SHALL notify user of rollback
8. THE Updater SHALL clean up old backups (configurable retention)

### Requirement 10: Offline Updates

**User Story:** As a user in restricted environments, I want offline updates, so that I can update without internet.

#### Acceptance Criteria

1. THE Updater SHALL support manual update file import
2. THE Updater SHALL verify imported update files
3. THE Updater SHALL support USB/network share update sources
4. THE Updater SHALL provide update file export for distribution
5. THE Updater SHALL document offline update process
6. THE Updater SHALL verify offline update signatures
7. THE Updater SHALL support enterprise update servers
8. THE Updater SHALL log offline update events

### Requirement 11: Update Notifications

**User Story:** As a user, I want update notifications, so that I know when updates are available.

#### Acceptance Criteria

1. THE Updater SHALL notify user when update is available
2. THE Updater SHALL show update version and release notes summary
3. THE Updater SHALL allow user to defer update
4. THE Updater SHALL respect "do not disturb" settings
5. THE Updater SHALL use native notification system
6. THE Updater SHALL not spam notifications
7. THE Updater SHALL indicate update urgency (security vs feature)
8. THE Updater SHALL provide "remind me later" option

### Requirement 12: Security Hardening

**User Story:** As a security-conscious user, I want hardened updates, so that the update process is secure.

#### Acceptance Criteria

1. THE Updater SHALL pin root signing keys in binary
2. THE Updater SHALL support key rotation with overlap period
3. THE Updater SHALL verify update server TLS certificate
4. THE Updater SHALL not execute downloaded code before verification
5. THE Updater SHALL run with minimal privileges
6. THE Updater SHALL protect update staging directory
7. THE Updater SHALL detect and report tampering attempts
8. THE Updater SHALL support TUF-style metadata (optional)
