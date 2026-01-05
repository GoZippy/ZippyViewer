# zrc-platform-android

Android platform layer for Zippy Remote Control (ZRC) system.

This crate provides JNI bindings to expose ZRC core functionality to Kotlin/Java Android applications.

## Features

- JNI bridge for Rust-Kotlin interop
- Session management
- Frame polling
- Input event sending
- Connection status monitoring

## Building

### Prerequisites

- Rust toolchain
- Android NDK
- `cargo-ndk`:
  ```bash
  cargo install cargo-ndk
  ```

### Build for Android

```bash
# Build for ARM64
cargo ndk -t arm64-v8a -o ../android-app/app/src/main/jniLibs build --release

# Build for x86_64
cargo ndk -t x86_64 -o ../android-app/app/src/main/jniLibs build --release

# Build for both
cargo ndk -t arm64-v8a -t x86_64 -o ../android-app/app/src/main/jniLibs build --release
```

## JNI Bindings

The crate exposes the following JNI functions:

- `Java_io_zippyremote_core_ZrcCore_init` - Initialize ZRC core
- `Java_io_zippyremote_core_ZrcCore_destroy` - Destroy ZRC core instance
- `Java_io_zippyremote_core_ZrcCore_startSession` - Start a session
- `Java_io_zippyremote_core_ZrcCore_endSession` - End a session
- `Java_io_zippyremote_core_ZrcCore_pollFrame` - Poll for frames
- `Java_io_zippyremote_core_ZrcCore_sendInput` - Send input events
- `Java_io_zippyremote_core_ZrcCore_getConnectionStatus` - Get connection status

## Architecture

The crate follows a layered architecture:

1. **JNI Layer** (`jni_bindings.rs`) - JNI function exports
2. **Core Layer** (`core.rs`) - ZRC core wrapper
3. **Session Layer** (`session.rs`) - Session management
4. **Frame Layer** (`frame.rs`) - Frame data structures
5. **Input Layer** (`input.rs`) - Input event structures

## Dependencies

- `zrc-core` - Core ZRC functionality
- `zrc-crypto` - Cryptographic primitives
- `zrc-proto` - Protocol definitions
- `jni` - JNI bindings for Rust

## Testing

Run tests:
```bash
cargo test
```

Note: Some tests may require Android environment or emulator.

## License

Apache-2.0 OR MIT
