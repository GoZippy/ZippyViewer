# iOS Platform Setup Guide

## Overview

This guide explains how to set up the iOS project structure for `zrc-platform-ios` using UniFFI.

## Prerequisites

1. **macOS** (required for iOS development)
2. **Xcode** (14.0 or later)
3. **Rust toolchain** with iOS targets:
   ```bash
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
   ```
4. **uniffi-bindgen** for generating Swift bindings:
   ```bash
   cargo install uniffi-bindgen
   ```

## Project Structure

The iOS project structure is already partially set up:

```
ios-app/
├── ZippyRemote/
│   ├── App.swift
│   ├── ContentView.swift
│   ├── DeviceListView.swift
│   ├── PairingView.swift
│   └── ViewerView.swift
├── ZippyRemote.xcodeproj/
└── ZippyRemote.xcworkspace/
```

## Setup Steps

### 1. Generate XCFramework

Run the build script:
```bash
cd crates/zrc-platform-ios
./build-xcframework.sh
```

This creates `ZrcIos.xcframework` with:
- arm64 (device)
- arm64-simulator
- x86_64-simulator

### 2. Add XCFramework to Xcode

1. Open `ZippyRemote.xcodeproj` in Xcode
2. Select project target
3. Go to "General" → "Frameworks, Libraries, and Embedded Content"
4. Add `ZrcIos.xcframework`

### 3. Configure Build Settings

**Build Phases:**
- Add `ZrcIos.xcframework` to "Link Binary With Libraries"
- Add to "Embed Frameworks"

**Build Settings:**
- Set "Enable Bitcode" to No
- Set "Always Embed Swift Standard Libraries" to Yes

### 4. UniFFI Bindings

The UDL file (`src/zrc_ios.udl`) defines the interface. Generate Swift bindings:

```bash
uniffi-bindgen generate src/zrc_ios.udl --language swift --out-dir ios-app/ZippyRemote/
```

### 5. Swift Integration

The generated bindings provide Swift classes:

```swift
import ZrcIos

let core = ZrcCore()
try core.init(configJson: config)
let sessionId = try core.startSession(deviceIdHex: deviceId)
let frame = try core.pollFrame(sessionId: sessionId)
```

## Next Steps

1. Implement Metal frame rendering
2. Implement touch input handling
3. Implement iOS Keychain integration
4. Complete SwiftUI views

## Notes

- Minimum iOS version: 15.0
- UniFFI handles memory management automatically
- Test on iPhone and iPad
- Support Dark Mode and Dynamic Type
