# Design Document: zrc-ci

## Overview

The zrc-ci module defines the continuous integration and deployment pipeline for the ZRC system using GitHub Actions. It covers build automation, testing, code signing, artifact publishing, and release management across all supported platforms.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              zrc-ci Pipeline                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Trigger Events                                   │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐                │   │
│  │  │   PR    │  │  Push   │  │  Tag    │  │ Schedule│                │   │
│  │  │ Created │  │ to main │  │ Created │  │ (nightly)│               │   │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘                │   │
│  └───────┼───────────┼───────────┼───────────┼────────────────────────┘   │
│          └───────────┴───────────┴───────────┘                              │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Quality Gates                                    │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐                │   │
│  │  │  Lint   │  │  Test   │  │ Security│  │Coverage │                │   │
│  │  │ (clippy)│  │ (cargo) │  │ (audit) │  │ (llvm)  │                │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Build Matrix                                     │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  │   │
│  │  │ Windows │  │  macOS  │  │  Linux  │  │ Android │  │   iOS   │  │   │
│  │  │ x86_64  │  │ universal│ │x64/arm64│  │arm64/x64│  │  arm64  │  │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Signing & Notarization                           │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Windows    │  │   macOS     │  │  Android    │                  │   │
│  │  │ Authenticode│  │ Notarization│  │  APK Sign   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Publishing                                       │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   GitHub    │  │   Update    │  │   SLSA     │                  │   │
│  │  │  Releases   │  │  Manifests  │  │ Provenance │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## GitHub Actions Workflows

### PR Validation Workflow

```yaml
# .github/workflows/pr.yml
name: PR Validation

on:
  pull_request:
    branches: [main, develop]

concurrency:
  group: pr-${{ github.head_ref }}
  cancel-in-progress: true

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      
      - name: Run tests
        run: cargo test --all-features --workspace

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
      
      - name: Security audit
        run: cargo audit

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview
      
      - name: Install cargo-llvm-cov
        run: cargo install cargo-llvm-cov
      
      - name: Generate coverage
        run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
          fail_ci_if_error: true
```

### Build Matrix Workflow

```yaml
# .github/workflows/build.yml
name: Build

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc
      - uses: Swatinem/rust-cache@v2
      
      - name: Build
        run: cargo build --release --target x86_64-pc-windows-msvc
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: windows-x64
          path: target/x86_64-pc-windows-msvc/release/*.exe

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-apple-darwin, aarch64-apple-darwin
      - uses: Swatinem/rust-cache@v2
      
      - name: Build x86_64
        run: cargo build --release --target x86_64-apple-darwin
      
      - name: Build arm64
        run: cargo build --release --target aarch64-apple-darwin
      
      - name: Create universal binary
        run: |
          lipo -create \
            target/x86_64-apple-darwin/release/zrc-agent \
            target/aarch64-apple-darwin/release/zrc-agent \
            -output zrc-agent-universal
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: macos-universal
          path: zrc-agent-universal

  build-linux:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu
      
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: linux-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/zrc-*

  build-android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-linux-android, x86_64-linux-android
      
      - name: Setup Android NDK
        uses: nttld/setup-ndk@v1
        with:
          ndk-version: r25c
      
      - name: Build native libraries
        run: |
          cargo build --release --target aarch64-linux-android
          cargo build --release --target x86_64-linux-android
      
      - name: Setup Java
        uses: actions/setup-java@v4
        with:
          distribution: temurin
          java-version: 17
      
      - name: Build APK
        working-directory: android
        run: ./gradlew assembleRelease
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: android
          path: android/app/build/outputs/apk/release/*.apk

  build-ios:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-ios, aarch64-apple-ios-sim
      
      - name: Build XCFramework
        run: |
          cargo build --release --target aarch64-apple-ios
          cargo build --release --target aarch64-apple-ios-sim
          
          xcodebuild -create-xcframework \
            -library target/aarch64-apple-ios/release/libzrc_ios.a \
            -library target/aarch64-apple-ios-sim/release/libzrc_ios.a \
            -output ZrcCore.xcframework
      
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ios-xcframework
          path: ZrcCore.xcframework
```

### Release Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  create-release:
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Generate changelog
        id: changelog
        run: |
          # Extract changelog for this version
          VERSION=${GITHUB_REF#refs/tags/v}
          # ... changelog extraction logic
      
      - name: Create Release
        id: create_release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Release ${{ github.ref }}
          body: ${{ steps.changelog.outputs.content }}
          draft: true
          prerelease: ${{ contains(github.ref, 'beta') || contains(github.ref, 'rc') }}

  sign-windows:
    needs: [create-release, build-windows]
    runs-on: windows-latest
    steps:
      - name: Download artifact
        uses: actions/download-artifact@v4
        with:
          name: windows-x64
      
      - name: Sign with Authenticode
        env:
          SIGNING_CERT: ${{ secrets.WINDOWS_SIGNING_CERT }}
          SIGNING_KEY: ${{ secrets.WINDOWS_SIGNING_KEY }}
        run: |
          # Import certificate
          $cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
            [System.Convert]::FromBase64String($env:SIGNING_CERT),
            $env:SIGNING_KEY
          )
          
          # Sign executables
          Set-AuthenticodeSignature -FilePath *.exe -Certificate $cert -TimestampServer "http://timestamp.digicert.com"
      
      - name: Upload signed artifact
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: zrc-agent.exe
          asset_name: zrc-agent-windows-x64.exe
          asset_content_type: application/octet-stream

  notarize-macos:
    needs: [create-release, build-macos]
    runs-on: macos-latest
    steps:
      - name: Download artifact
        uses: actions/download-artifact@v4
        with:
          name: macos-universal
      
      - name: Import certificates
        env:
          APPLE_CERT: ${{ secrets.APPLE_DEVELOPER_CERT }}
          APPLE_CERT_PASSWORD: ${{ secrets.APPLE_CERT_PASSWORD }}
        run: |
          echo "$APPLE_CERT" | base64 --decode > cert.p12
          security create-keychain -p "" build.keychain
          security import cert.p12 -k build.keychain -P "$APPLE_CERT_PASSWORD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple: -s -k "" build.keychain
      
      - name: Sign and notarize
        env:
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_APP_PASSWORD }}
          TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: |
          # Sign with hardened runtime
          codesign --force --options runtime --sign "Developer ID Application" zrc-agent-universal
          
          # Create ZIP for notarization
          zip zrc-agent.zip zrc-agent-universal
          
          # Submit for notarization
          xcrun notarytool submit zrc-agent.zip \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id "$TEAM_ID" \
            --wait
          
          # Staple ticket
          xcrun stapler staple zrc-agent-universal

  generate-provenance:
    needs: [create-release]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Generate SLSA provenance
        uses: slsa-framework/slsa-github-generator/.github/workflows/generator_generic_slsa3.yml@v1.9.0
        with:
          base64-subjects: ${{ needs.build.outputs.hashes }}
      
      - name: Generate SBOM
        run: |
          cargo install cargo-sbom
          cargo sbom --output-format spdx-json > sbom.json
      
      - name: Upload provenance
        uses: actions/upload-release-asset@v1
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: sbom.json
          asset_name: sbom.json
          asset_content_type: application/json

  publish-update-manifest:
    needs: [sign-windows, notarize-macos]
    runs-on: ubuntu-latest
    steps:
      - name: Generate update manifest
        run: |
          VERSION=${GITHUB_REF#refs/tags/v}
          cat > manifest.json << EOF
          {
            "version": "$VERSION",
            "channel": "stable",
            "platforms": {
              "windows-x64": {
                "url": "https://github.com/org/zrc/releases/download/v$VERSION/zrc-agent-windows-x64.exe",
                "sha256": "$(sha256sum zrc-agent-windows-x64.exe | cut -d' ' -f1)",
                "size": $(stat -c%s zrc-agent-windows-x64.exe)
              },
              "macos-universal": {
                "url": "https://github.com/org/zrc/releases/download/v$VERSION/zrc-agent-macos-universal",
                "sha256": "$(sha256sum zrc-agent-macos-universal | cut -d' ' -f1)",
                "size": $(stat -c%s zrc-agent-macos-universal)
              }
            }
          }
          EOF
      
      - name: Sign manifest
        run: |
          # Sign with release key
          openssl dgst -sha256 -sign release-key.pem -out manifest.sig manifest.json
      
      - name: Upload to update server
        run: |
          # Upload manifest and signature
          aws s3 cp manifest.json s3://updates.zippyremote.io/stable/manifest.json
          aws s3 cp manifest.sig s3://updates.zippyremote.io/stable/manifest.sig
```

## Correctness Properties

### Property 1: Build Reproducibility
*For any* tagged release, building from the same commit with the same toolchain SHALL produce byte-identical artifacts (excluding signatures).
**Validates: Requirement 10.3**

### Property 2: Signature Verification
*For any* signed artifact, the signature SHALL be verifiable with the published public key.
**Validates: Requirements 4.5, 5.4, 6.6**

### Property 3: Test Gate Enforcement
*For any* PR merge, all quality gate checks SHALL pass before merge is allowed.
**Validates: Requirement 2.8**

### Property 4: Provenance Completeness
*For any* release, SLSA provenance SHALL include builder identity, source commit, and build parameters.
**Validates: Requirements 10.1, 10.7**

## Testing Strategy

### Workflow Tests
- Validate YAML syntax
- Test matrix expansion
- Verify artifact paths
- Check secret references

### Integration Tests
- End-to-end release simulation
- Signing verification
- Update manifest validation
- Provenance verification
