# CI/CD Workflows Verification Checklist

This checklist helps verify that all CI/CD workflows are functioning correctly.

## Prerequisites

Before running verification, ensure:
- [ ] GitHub repository secrets are configured (if testing signing/notarization)
- [ ] Codecov token is configured (if testing coverage upload)
- [ ] All required dependencies are available

## Task 7: Checkpoint - Verify all workflows

### 1. PR Validation Workflow (`.github/workflows/pr.yml`)

#### Test Steps:
1. [ ] Create a test PR to `main` or `develop` branch
2. [ ] Verify workflow triggers automatically
3. [ ] Check that all jobs run:
   - [ ] `lint` job completes successfully
   - [ ] `test` job runs on all three platforms (ubuntu, windows, macos)
   - [ ] `security` job completes cargo audit
   - [ ] `coverage` job generates and uploads coverage
4. [ ] Verify concurrency cancellation works (push another commit to same PR)
5. [ ] Test failure scenarios:
   - [ ] Introduce a formatting error - lint should fail
   - [ ] Introduce a clippy warning - lint should fail
   - [ ] Introduce a test failure - test job should fail
   - [ ] Verify failure notifications are triggered

#### Expected Results:
- All jobs complete successfully on valid code
- Jobs fail appropriately on invalid code
- Failure notifications are sent
- Workflow cancels previous runs when new commits are pushed

### 2. Build Matrix Workflow (`.github/workflows/build.yml`)

#### Test Steps:
1. [ ] Push a commit to `main` branch
2. [ ] Verify workflow triggers automatically
3. [ ] Check that all build jobs run:
   - [ ] `build-windows` produces `.exe` artifacts
   - [ ] `build-macos` creates universal binaries
   - [ ] `build-linux` builds for both x86_64 and aarch64
   - [ ] `build-android` produces APK
   - [ ] `build-ios` creates XCFramework
4. [ ] Verify artifacts are uploaded correctly
5. [ ] Check artifact retention (7 days for build workflow)
6. [ ] Test manual trigger via `workflow_dispatch`

#### Expected Results:
- All platforms build successfully
- Artifacts are correctly named and uploaded
- Artifacts are accessible for download
- Build caching reduces build times on subsequent runs

### 3. Release Workflow (`.github/workflows/release.yml`)

#### Test Steps (Dry Run):
1. [ ] Create a test tag: `git tag v0.1.0-test && git push origin v0.1.0-test`
2. [ ] Verify workflow triggers on tag push
3. [ ] Check release creation:
   - [ ] Draft release is created
   - [ ] Changelog is extracted (if CHANGELOG.md exists)
   - [ ] Version is extracted correctly from tag
4. [ ] Verify all build jobs complete
5. [ ] Test signing jobs (if secrets configured):
   - [ ] Windows signing produces signed executables
   - [ ] macOS notarization completes (may take time)
   - [ ] Android signing produces signed APK and AAB
   - [ ] iOS signing signs XCFramework
6. [ ] Verify provenance generation:
   - [ ] SLSA provenance is generated
   - [ ] SBOM is generated with cargo-sbom
7. [ ] Verify update manifest:
   - [ ] Manifest JSON is generated with correct structure
   - [ ] Checksums are calculated correctly
   - [ ] Manifest is signed (if key configured)
   - [ ] Manifest is uploaded to release
8. [ ] Clean up test tag: `git tag -d v0.1.0-test && git push origin :refs/tags/v0.1.0-test`

#### Expected Results:
- Release is created as draft
- All artifacts are built and signed (if configured)
- Provenance and SBOM are generated
- Update manifest is created and signed
- All artifacts are attached to release

### 4. Nightly Workflow (`.github/workflows/nightly.yml`)

#### Test Steps:
1. [ ] Trigger workflow manually via `workflow_dispatch`
2. [ ] Verify all jobs run:
   - [ ] Extended test suite runs on all platforms
   - [ ] Property-based tests for crypto run
   - [ ] Dependency update check runs
   - [ ] Fuzz testing runs (if fuzz targets exist)
   - [ ] Performance benchmarks run (if benchmarks exist)
3. [ ] Check that artifacts are uploaded:
   - [ ] Outdated dependencies report (if any)
   - [ ] Benchmark results (if benchmarks exist)

#### Expected Results:
- All test suites complete
- Property tests validate crypto properties
- Dependency updates are detected and reported
- Benchmarks run successfully (if implemented)

### 5. Infrastructure Configuration

#### Verify:
- [ ] Build caching is working (check cache hit rates in workflow logs)
- [ ] Jobs are parallelized correctly (multiple jobs run simultaneously)
- [ ] Timeouts are configured appropriately (no jobs hang indefinitely)
- [ ] Failure alerts are triggered on job failures

### 6. Signature Verification (Property Test)

#### Test Steps:
1. [ ] Run property tests: `cargo test --package zrc-updater --lib proptests`
2. [ ] Verify all property tests pass:
   - [ ] `prop_valid_signature_verification`
   - [ ] `prop_invalid_signature_verification`
   - [ ] `prop_corrupted_signature_verification`
   - [ ] `prop_multi_signature_threshold`
   - [ ] `prop_timestamp_validation`
3. [ ] Run with more iterations: `PROPTEST_CASES=1000 cargo test --package zrc-updater --lib proptests`

#### Expected Results:
- All property tests pass
- Valid signatures verify successfully
- Invalid signatures fail verification
- Threshold verification works correctly
- Timestamp validation works correctly

## Common Issues and Troubleshooting

### Issue: Workflow doesn't trigger
- Check branch protection rules
- Verify workflow file syntax: `yamllint .github/workflows/*.yml`
- Check GitHub Actions settings

### Issue: Build fails
- Check Rust toolchain version compatibility
- Verify all dependencies are available
- Check for platform-specific build requirements

### Issue: Signing fails
- Verify secrets are configured correctly
- Check certificate/key formats (base64 encoded)
- Verify certificate hasn't expired
- Check timestamp server availability (Windows)

### Issue: Notarization fails
- Verify Apple Developer credentials
- Check certificate validity
- Verify app-specific password is correct
- Check Apple's notarization service status

### Issue: Coverage upload fails
- Verify Codecov token is set
- Check file paths are correct
- Verify lcov.info is generated

## Notes

- Self-hosted runners (Task 6.1) require infrastructure setup and are not verified in this checklist
- Property test (Task 4.6) is optional but recommended for security validation
- Some tests require actual secrets/certificates to fully verify signing functionality
