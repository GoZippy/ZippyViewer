# zrc-crypto Tasks

- [ ] Add transcript test vectors (known inputs -> expected sha256).
- [ ] Add envelope golden vectors and cross-language verification.
- [ ] Implement directory record signing/verification helpers.
- [ ] Implement replay protection primitives:
  - [ ] monotonic counters per channel
  - [ ] nonce misuse guard rails
- [ ] Add fuzzing targets (cargo-fuzz) for decode/open paths.
- [ ] Add optional PQC feature flags (compile-time).
