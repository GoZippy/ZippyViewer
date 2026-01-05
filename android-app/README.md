# ZippyRemote Android App

Android controller application for Zippy Remote Control (ZRC) system.

## Features

- Remote desktop viewing and control
- Touch input mapping (tap, long-press, drag, scroll)
- Keyboard input (soft keyboard, hardware keyboard, special keys)
- Clipboard synchronization
- Device pairing via QR code or clipboard
- Secure key storage using Android Keystore
- Connection management with auto-reconnection
- Optional host mode (MediaProjection screen capture, AccessibilityService input injection)

## Requirements

- Android 8.0 (API 26) or higher
- ARM64-v8a or x86_64 architecture
- Rust toolchain with Android NDK support (for building native library)

## Building

### Prerequisites

1. Install Android Studio
2. Install Android NDK
3. Install Rust toolchain
4. Install `cargo-ndk`:
   ```bash
   cargo install cargo-ndk
   ```

### Build Steps

1. Build the Rust library:
   ```bash
   cd ../zippy-remote/crates/zrc-platform-android
   cargo ndk -t arm64-v8a -t x86_64 -o ../android-app/app/src/main/jniLibs build --release
   ```

2. Build the Android app:
   ```bash
   cd android-app
   ./gradlew assembleDebug
   ```

## Project Structure

- `app/src/main/java/io/zippyremote/` - Kotlin source code
  - `core/` - JNI wrapper (ZrcCore.kt)
  - `viewer/` - Frame rendering (ViewerSurfaceView.kt)
  - `input/` - Input handling (TouchInputHandler.kt, KeyboardInputHandler.kt)
  - `connection/` - Connection management
  - `pairing/` - Device pairing
  - `keystore/` - Secure key storage
  - `clipboard/` - Clipboard synchronization
  - `host/` - Host mode features (optional)

## Native Library

The native library (`libzrc_android.so`) is built from the `zrc-platform-android` Rust crate and provides JNI bindings to the ZRC core functionality.

## Configuration

The app uses JSON configuration for ZRC core initialization. Configuration can be loaded from SharedPreferences or provided at runtime.

## Permissions

- `INTERNET` - Network connectivity
- `ACCESS_NETWORK_STATE` - Network status monitoring
- `CAMERA` - QR code scanning for pairing
- `FOREGROUND_SERVICE` - Persistent session service
- `POST_NOTIFICATIONS` - Session notifications
- `READ_CLIPBOARD` - Clipboard synchronization (Android 10+)
- `RECORD_AUDIO` - MediaProjection (host mode)

## Host Mode (Optional)

Host mode allows the Android device to act as a host, sharing its screen and accepting remote input:

1. **MediaProjection**: Screen capture service for sharing the device screen
2. **AccessibilityService**: Input injection service for remote control

Both features require user permission and explicit enablement.

## Testing

Run unit tests:
```bash
./gradlew test
```

Run instrumented tests:
```bash
./gradlew connectedAndroidTest
```

## License

Apache-2.0 OR MIT
