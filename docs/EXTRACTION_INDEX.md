# ChatGPT Chat Extraction Index

This document tracks all files that were created/discussed in the ChatGPT conversation and need to be extracted to the project.

## Project Structure Created in Chat

### Workspace Root
- [x] `Cargo.toml` (workspace) - Already exists, needs update

### Core Crates

#### zrc-proto
- [ ] `crates/zrc-proto/Cargo.toml`
- [ ] `crates/zrc-proto/build.rs`
- [ ] `crates/zrc-proto/src/lib.rs`
- [ ] `crates/zrc-proto/proto/zrc_v1.proto` - Complete protobuf schema

#### zrc-crypto
- [ ] `crates/zrc-crypto/Cargo.toml`
- [ ] `crates/zrc-crypto/src/lib.rs`
- [ ] `crates/zrc-crypto/src/hash.rs`
- [ ] `crates/zrc-crypto/src/transcript.rs`
- [ ] `crates/zrc-crypto/src/sas.rs`
- [ ] `crates/zrc-crypto/src/pairing.rs`
- [ ] `crates/zrc-crypto/src/envelope.rs`
- [ ] `crates/zrc-crypto/src/ticket.rs`
- [ ] `crates/zrc-crypto/src/session_crypto.rs`

#### zrc-core
- [ ] `crates/zrc-core/Cargo.toml`
- [ ] `crates/zrc-core/src/lib.rs`
- [ ] `crates/zrc-core/src/errors.rs`
- [ ] `crates/zrc-core/src/types.rs`
- [ ] `crates/zrc-core/src/store.rs`
- [ ] `crates/zrc-core/src/pairing.rs`
- [ ] `crates/zrc-core/src/session.rs`
- [ ] `crates/zrc-core/src/dispatch.rs`
- [ ] `crates/zrc-core/src/keys.rs`
- [ ] `crates/zrc-core/src/harness.rs`
- [ ] `crates/zrc-core/src/http_mailbox.rs`
- [ ] `crates/zrc-core/src/quic.rs`
- [ ] `crates/zrc-core/src/quic_mux.rs`
- [ ] `crates/zrc-core/tests/flow.rs`

#### zrc-rendezvous
- [ ] `crates/zrc-rendezvous/Cargo.toml`
- [ ] `crates/zrc-rendezvous/src/main.rs`

#### zrc-demo
- [ ] `crates/zrc-demo/Cargo.toml`
- [ ] `crates/zrc-demo/src/main.rs`

#### zrc-platform-win
- [ ] `crates/zrc-platform-win/Cargo.toml`
- [ ] `crates/zrc-platform-win/src/lib.rs`
- [ ] `crates/zrc-platform-win/src/capture_gdi.rs`
- [ ] `crates/zrc-platform-win/src/input_sendinput.rs`

#### zrc-viewer
- [ ] `crates/zrc-viewer/Cargo.toml`
- [ ] `crates/zrc-viewer/src/lib.rs`

## Extraction Status

### Phase 1: Core Infrastructure (Priority)
1. Protobuf schema and build setup
2. Crypto primitives (hash, transcript, SAS, pairing)
3. Envelope and ticket handling
4. Core pairing/session state machines

### Phase 2: Transport & Networking
1. HTTP mailbox client
2. QUIC transport helpers
3. QUIC multiplexing
4. Rendezvous server

### Phase 3: Platform Implementation
1. Windows capture (GDI)
2. Windows input (SendInput)
3. Viewer application

### Phase 4: Integration & Demo
1. Demo application (host/controller)
2. Integration tests

## Notes

- All code uses Rust with `#![forbid(unsafe_code)]` except platform-specific modules
- Protobuf messages use versioned naming (V1 suffix)
- Security-critical code uses deterministic transcript hashing
- Platform crates depend on zrc-core, not vice versa

