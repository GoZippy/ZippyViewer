# Code Signing and Notarization Guide

This document describes the code signing requirements and workflow for distributing `zrc-platform-mac` applications.

## Requirements

### Hardened Runtime

All macOS applications must be signed with the hardened runtime enabled for distribution outside the Mac App Store. This is enforced by macOS Gatekeeper and required for notarization.

### Entitlements

The following entitlements may be required depending on functionality:

#### Required Entitlements

```xml
<!-- Hardened Runtime (required for distribution) -->
<key>com.apple.security.cs.allow-jit</key>
<false/>
<key>com.apple.security.cs.allow-unsigned-executable-memory</key>
<false/>
<key>com.apple.security.cs.allow-dyld-environment-variables</key>
<false/>
<key>com.apple.security.cs.disable-library-validation</key>
<false/>
```

#### Optional Entitlements (as needed)

```xml
<!-- Screen Recording (runtime permission, no entitlement needed) -->
<!-- Accessibility (runtime permission, no entitlement needed) -->

<!-- Network (if needed) -->
<key>com.apple.security.network.client</key>
<true/>
<key>com.apple.security.network.server</key>
<true/>
```

### Runtime Permissions

The following permissions are requested at runtime and do not require entitlements:

1. **Screen Recording Permission**
   - Required for screen capture functionality
   - Requested via `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`
   - User must grant in System Preferences > Security & Privacy > Privacy > Screen Recording

2. **Accessibility Permission**
   - Required for input injection (mouse/keyboard)
   - Requested via `AXIsProcessTrusted`
   - User must grant in System Preferences > Security & Privacy > Privacy > Accessibility

## Code Signing Workflow

### 1. Build the Application

```bash
cargo build --release --package zrc-platform-mac
```

### 2. Code Sign the Binary

```bash
# Sign with your Developer ID certificate
codesign --force --deep --sign "Developer ID Application: Your Name (TEAM_ID)" \
  --options runtime \
  --entitlements entitlements.plist \
  target/release/zrc-platform-mac
```

### 3. Verify Code Signing

```bash
# Verify the signature
codesign --verify --verbose target/release/zrc-platform-mac

# Check entitlements
codesign --display --entitlements - target/release/zrc-platform-mac
```

### 4. Notarize the Application

```bash
# Create a zip archive for notarization
ditto -c -k --keepParent target/release/zrc-platform-mac zrc-platform-mac.zip

# Submit for notarization
xcrun notarytool submit zrc-platform-mac.zip \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "app-specific-password" \
  --wait

# Staple the notarization ticket
xcrun stapler staple target/release/zrc-platform-mac
```

### 5. Verify Notarization

```bash
# Check notarization status
spctl --assess --verbose target/release/zrc-platform-mac

# Verify stapling
stapler validate target/release/zrc-platform-mac
```

## Entitlements File Template

Create `entitlements.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Hardened Runtime -->
    <key>com.apple.security.cs.allow-jit</key>
    <false/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <false/>
    <key>com.apple.security.cs.allow-dyld-environment-variables</key>
    <false/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <false/>
    
    <!-- Network (if needed) -->
    <key>com.apple.security.network.client</key>
    <true/>
    <key>com.apple.security.network.server</key>
    <true/>
</dict>
</plist>
```

## Distribution Checklist

- [ ] Application built in release mode
- [ ] Hardened runtime enabled
- [ ] Code signed with Developer ID certificate
- [ ] Entitlements file configured
- [ ] Signature verified
- [ ] Application notarized with Apple
- [ ] Notarization ticket stapled
- [ ] Final verification passed

## Troubleshooting

### Common Issues

1. **"code object is not signed at all"**
   - Ensure the binary is code signed before notarization

2. **"hardened runtime violations"**
   - Check entitlements file and ensure hardened runtime is properly configured

3. **"notarization failed"**
   - Check notarization logs: `xcrun notarytool log <submission-id> --apple-id <email> --team-id <team-id> --password <password>`
   - Common causes: missing hardened runtime, unsigned dependencies

4. **"Gatekeeper blocks execution"**
   - Ensure notarization ticket is stapled
   - Verify signature: `codesign --verify --verbose <binary>`

## References

- [Apple Code Signing Guide](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/)
- [Hardened Runtime](https://developer.apple.com/documentation/security/hardened_runtime)
- [Notarization](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
