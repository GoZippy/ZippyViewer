# Offline Updates Guide

This document describes how to use the ZRC offline update system for environments without internet access.

## Overview

The offline update system allows you to:
- Export update packages from a machine with internet access
- Transfer packages via USB drive, network share, or other offline means
- Import and install updates on air-gapped or restricted machines

## Package Format

Offline update packages use the `.zrcu` extension (Zippy Remote Control Update). Each package contains:
- A cryptographically signed manifest with version info and release notes
- The update artifact (binary)
- SHA-256 hash verification data

## Exporting Updates

### From Command Line

```bash
# Export an update package for distribution
zrc-updater export --version 2.0.0 --output update.zrcu
```

### Programmatic Export

```rust
use zrc_updater::offline::OfflineUpdateManager;

let manager = OfflineUpdateManager::new(
    manifest_verifier,
    artifact_verifier,
    staging_dir,
);

// Export update package
let package_size = manager.export_update_file(
    &signed_manifest,
    &artifact_path,
    Path::new("zrc-update-2.0.0-windows-x86_64-stable.zrcu"),
)?;
```

### Package Naming Convention

Use the helper function to generate consistent filenames:

```rust
use zrc_updater::offline::generate_package_filename;
use semver::Version;
use zrc_updater::UpdateChannel;

let filename = generate_package_filename(
    &Version::new(2, 0, 0),
    "windows-x86_64",
    &UpdateChannel::Stable,
);
// Result: "zrc-update-2.0.0-windows-x86_64-stable.zrcu"
```

## Importing Updates

### Verification Before Import

Always verify an update package before importing:

```rust
// Verify without extracting
let info = manager.verify_update_file(Path::new("update.zrcu"))?;

println!("Version: {}", info.version);
println!("Platform: {}", info.platform);
println!("Size: {} bytes", info.artifact_size);
println!("Security update: {}", info.is_security_update);

// Check platform compatibility
if !info.is_current_platform() {
    println!("Warning: This update is for a different platform!");
}
```

### Full Import

```rust
// Import and extract the update
let (manifest, artifact_path) = manager.import_update_file(
    Path::new("update.zrcu")
)?;

println!("Extracted to: {:?}", artifact_path);
println!("Version: {}", manifest.version);

// The artifact is now ready for installation
```

## Security Considerations

### Signature Verification

All offline update packages are cryptographically signed:
- Manifest signatures are verified against pinned public keys
- At least one valid signature is required (configurable threshold)
- Manifests older than 7 days are rejected

### Hash Verification

Artifact integrity is verified using SHA-256:
- Hash is computed during import
- Constant-time comparison prevents timing attacks
- Mismatched hashes cause immediate rejection

### Platform Verification

The system verifies platform compatibility:
- Manifest platform must match the current system
- Cross-platform updates are rejected

## Workflow for Air-Gapped Environments

### Step 1: Download on Connected Machine

On a machine with internet access:

```bash
# Check for updates
zrc-updater check

# Download and export for offline distribution
zrc-updater export --channel stable --output /media/usb/update.zrcu
```

### Step 2: Transfer Package

Transfer the `.zrcu` file to the target machine via:
- USB drive
- Network share
- Secure file transfer
- Physical media

### Step 3: Verify on Target Machine

On the air-gapped machine:

```bash
# Verify the package integrity
zrc-updater verify /media/usb/update.zrcu
```

### Step 4: Install Update

```bash
# Import and install
zrc-updater install --offline /media/usb/update.zrcu
```

## Enterprise Deployment

For enterprise environments, you can set up an internal update server:

1. Configure a custom update channel pointing to your internal server
2. Download updates from the official server
3. Re-sign with your enterprise keys (optional)
4. Distribute via your internal infrastructure

### Custom Channel Configuration

```toml
# updater.toml
[update]
channel = "custom"
custom_manifest_url = "https://updates.internal.company.com/zrc/manifest.json"
```

## Troubleshooting

### "Invalid update package: wrong magic bytes"

The file is not a valid ZRC update package. Ensure you're using a `.zrcu` file exported by the ZRC updater.

### "Unsupported package version"

The package was created with a newer version of the updater. Update your ZRC installation first using a compatible package.

### "Manifest signature verification failed"

The package signature is invalid. This could indicate:
- Corrupted file during transfer
- Tampered package
- Expired signing keys

Re-download the package from a trusted source.

### "Hash mismatch"

The artifact was corrupted during transfer. Re-copy the file and try again.

### "Platform mismatch"

You're trying to install an update for a different platform. Download the correct package for your system:
- `windows-x86_64` - Windows 64-bit
- `macos-x86_64` - macOS Intel
- `macos-aarch64` - macOS Apple Silicon
- `linux-x86_64` - Linux 64-bit

## API Reference

### OfflineUpdateManager

Main class for offline update operations.

```rust
impl OfflineUpdateManager {
    /// Create a new manager
    fn new(
        manifest_verifier: ManifestVerifier,
        artifact_verifier: ArtifactVerifier,
        staging_dir: PathBuf,
    ) -> Self;

    /// Import and verify an update file
    fn import_update_file(&self, path: &Path) 
        -> Result<(UpdateManifest, PathBuf), UpdateError>;

    /// Export an update package
    fn export_update_file(
        &self,
        signed_manifest: &SignedManifest,
        artifact_path: &Path,
        output_path: &Path,
    ) -> Result<u64, UpdateError>;

    /// Verify without extracting
    fn verify_update_file(&self, path: &Path) 
        -> Result<OfflineUpdateInfo, UpdateError>;

    /// Clean up staging directory
    fn cleanup_staging(&self) -> Result<(), UpdateError>;
}
```

### OfflineUpdateInfo

Information about an offline update package.

```rust
struct OfflineUpdateInfo {
    version: Version,
    platform: String,
    channel: UpdateChannel,
    artifact_size: u64,
    package_size: u64,
    is_security_update: bool,
    release_notes: String,
}
```
