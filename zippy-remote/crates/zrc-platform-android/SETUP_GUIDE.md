# Android Platform Setup Guide

## Overview

This guide explains how to set up the Android project structure for `zrc-platform-android`.

## Prerequisites

1. **Android Studio** (latest stable version)
2. **Android NDK** (r25c or later)
3. **Rust toolchain** with Android targets:
   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```
4. **cargo-ndk** for cross-compilation:
   ```bash
   cargo install cargo-ndk
   ```

## Project Structure

The Android project should be structured as follows:

```
android-app/
├── app/
│   ├── build.gradle
│   ├── src/
│   │   ├── main/
│   │   │   ├── AndroidManifest.xml
│   │   │   ├── java/io/zippyremote/
│   │   │   │   └── core/
│   │   │   │       └── ZrcCore.kt
│   │   │   └── res/
│   │   └── test/
│   └── CMakeLists.txt
├── build.gradle
├── settings.gradle
└── gradle.properties
```

## Setup Steps

### 1. Create Android Project

1. Open Android Studio
2. Create new project with Kotlin
3. Minimum SDK: API 26 (Android 8.0)
4. Target SDK: Latest

### 2. Configure Gradle

**app/build.gradle:**
```gradle
android {
    ...
    externalNativeBuild {
        cmake {
            path "src/main/cpp/CMakeLists.txt"
        }
    }
    ndkVersion "25.2.9519653"
}

dependencies {
    implementation "org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3"
}
```

### 3. Build Rust Library

```bash
cd crates/zrc-platform-android
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 build --release
```

### 4. Create JNI Bindings

The JNI bindings are defined in `src/jni_bindings.rs`. The Kotlin wrapper class should match:

```kotlin
package io.zippyremote.core

class ZrcCore {
    external fun init(configJson: String): Long
    external fun startSession(deviceIdHex: String): Long
    external fun endSession(sessionId: Long)
    external fun pollFrame(sessionId: Long): ByteArray?
    external fun sendInput(sessionId: Long, eventJson: String)
    
    companion object {
        init {
            System.loadLibrary("zrc_android")
        }
    }
}
```

### 5. CMakeLists.txt

```cmake
cmake_minimum_required(VERSION 3.22.1)
project("zrc_android")

add_library(zrc_android SHARED
    IMPORTED)

set_target_properties(zrc_android PROPERTIES
    IMPORTED_LOCATION
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../target/${ANDROID_ABI}/release/libzrc_android.so)
```

## Next Steps

1. Implement frame rendering with SurfaceView
2. Implement touch input handling
3. Implement Android Keystore integration
4. Add UI components (Material Design 3)

## Notes

- The Rust library is built as a `cdylib` for JNI
- Memory safety is critical - ensure proper cleanup
- Use Kotlin coroutines for async operations
- Test on multiple Android versions (API 26+)
