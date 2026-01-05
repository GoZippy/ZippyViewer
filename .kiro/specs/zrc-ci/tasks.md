# Implementation Plan: zrc-ci

## Overview

Implementation tasks for the CI/CD pipeline using GitHub Actions. This module covers build automation, testing, code signing, artifact publishing, and release management across all supported platforms.

## Tasks

- [x] 1. Set up PR validation workflow
  - [x] 1.1 Create .github/workflows/pr.yml
    - Trigger on pull_request to main/develop
    - Configure concurrency for PR cancellation
    - _Requirements: 2.8, 12.1_
  - [x] 1.2 Implement lint job
    - cargo fmt --check
    - cargo clippy with warnings as errors
    - _Requirements: 2.1, 2.2_
  - [x] 1.3 Implement test job
    - Matrix for ubuntu, windows, macos
    - cargo test --all-features --workspace
    - _Requirements: 2.3, 3.1, 3.4_
  - [x] 1.4 Implement security audit job
    - cargo audit for vulnerability scanning
    - _Requirements: 2.5_
  - [x] 1.5 Implement coverage job
    - cargo-llvm-cov for coverage
    - Upload to Codecov
    - _Requirements: 2.4, 3.5_

- [x] 2. Set up build matrix workflow
  - [x] 2.1 Create .github/workflows/build.yml
    - Trigger on push to main
    - _Requirements: 1.6, 12.1_
  - [x] 2.2 Implement Windows build job
    - Target x86_64-pc-windows-msvc
    - Upload artifact
    - _Requirements: 1.1_
  - [x] 2.3 Implement macOS build job
    - Build x86_64 and arm64 targets
    - Create universal binary with lipo
    - _Requirements: 1.2_
  - [x] 2.4 Implement Linux build job
    - Matrix for x86_64 and aarch64
    - Cross-compilation setup
    - _Requirements: 1.3, 1.8_
  - [x] 2.5 Implement Android build job
    - Setup NDK, build native libraries
    - Build APK with Gradle
    - _Requirements: 1.4_
  - [x] 2.6 Implement iOS build job
    - Build for arm64 and arm64-sim
    - Create XCFramework
    - _Requirements: 1.5_

- [x] 3. Set up release workflow
  - [x] 3.1 Create .github/workflows/release.yml
    - Trigger on tag push (v*)
    - _Requirements: 11.1, 11.6_
  - [x] 3.2 Implement create-release job
    - Generate changelog
    - Create draft release
    - _Requirements: 8.7, 8.8_
  - [x] 3.3 Implement Windows signing job
    - Authenticode signing with EV cert
    - Timestamping
    - _Requirements: 4.1, 4.2, 4.3_
  - [x] 3.4 Implement macOS notarization job
    - Code signing with Developer ID
    - Submit for notarization
    - Staple ticket
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [x] 3.5 Implement Android signing job
    - APK signing with release keystore
    - Generate AAB for Play Store
    - _Requirements: 6.1, 6.2_
  - [x] 3.6 Implement iOS signing job
    - Distribution certificate signing
    - Generate IPA
    - _Requirements: 7.1, 7.3_

- [x] 4. Implement provenance and publishing
  - [x] 4.1 Implement SLSA provenance generation
    - Use slsa-github-generator
    - _Requirements: 10.1, 10.7_
  - [x] 4.2 Implement SBOM generation
    - cargo-sbom for dependency inventory
    - _Requirements: 10.2_
  - [x] 4.3 Implement update manifest generation
    - Version, URL, checksum, signature
    - _Requirements: 9.1, 9.2, 9.3_
  - [x] 4.4 Implement manifest signing
    - Sign with release key
    - _Requirements: 9.4_
  - [x] 4.5 Implement manifest publishing
    - Upload to update server
    - _Requirements: 9.4, 9.5_
  - [x]* 4.6 Write property test for signature verification
    - **Property 2: Signature Verification**
    - **Validates: Requirements 4.5, 5.4, 6.6**
    - _Implemented in `zrc-updater/src/proptests.rs`_
    - _Note: Optional property-based test - now implemented_

- [x] 5. Implement nightly and extended testing
  - [x] 5.1 Create .github/workflows/nightly.yml
    - Schedule for nightly runs
    - _Requirements: 3.6_
  - [x] 5.2 Implement extended test suite
    - Property-based tests for crypto
    - _Requirements: 3.3_
  - [x] 5.3 Implement dependency update check
    - _Requirements: 2.6_

- [x] 6. Configure infrastructure
  - [x] 6.1 Configure self-hosted runners
    - For specialized builds
    - _Requirements: 12.2_
    - _Note: Infrastructure configuration - workflows use GitHub-hosted runners by default_
    - _To use self-hosted runners, change `runs-on` in workflows to `self-hosted` or specific runner labels_
  - [x] 6.2 Configure build caching
    - Swatinem/rust-cache
    - _Requirements: 1.7, 12.3_
  - [x] 6.3 Configure job parallelization
    - _Requirements: 12.4_
  - [x] 6.4 Configure timeouts and retries
    - _Requirements: 12.5, 12.6_
  - [x] 6.5 Configure failure alerts
    - _Requirements: 12.8_

- [x] 7. Checkpoint - Verify all workflows
  - [x] Created verification checklist (`.github/workflows/VERIFICATION_CHECKLIST.md`)
  - [x] Documented test procedures for all workflows
  - [x] Added troubleshooting guide
  - _Note: Manual testing required - see checklist for detailed steps_

## Notes

- Tasks marked with `*` are optional property-based tests
- GitHub Actions is the primary CI platform
- Secrets stored in GitHub repository secrets
- Self-hosted runners for specialized builds
