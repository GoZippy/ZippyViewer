# zrc-ci Design

- GitHub Actions or self-hosted runners
- Separate workflows:
  - PR: lint/test
  - nightly: integration tests
  - release: build/sign/publish
- Security:
  - secrets in protected env
  - provenance (SLSA-ish) optional

