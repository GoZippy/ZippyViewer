# zrc-platform-ios

iOS platform implementation for Zippy Remote Control (ZRC).

This crate provides:
- UniFFI bindings to expose ZRC core functionality to Swift
- XCFramework for integration with Xcode projects
- Swift/SwiftUI application structure

## Building

### Prerequisites

- Rust toolchain
- Xcode with iOS SDK
- UniFFI (`cargo install uniffi_bindgen`)

### Build XCFramework

```bash
./build-xcframework.sh
```

This will:
1. Build the Rust library for `aarch64-apple-ios` (device)
2. Build the Rust library for `aarch64-apple-ios-sim` (simulator)
3. Package as XCFramework

## Project Structure

```
zrc-platform-ios/
├── src/              # Rust source code
├── ios-app/          # Xcode project (Swift/SwiftUI)
│   ├── ZippyRemote/  # Main app target
│   └── BroadcastExtension/  # ReplayKit extension
└── build-xcframework.sh  # Build script
```

## Integration

Add the XCFramework to your Xcode project:
1. Drag `ZrcIos.xcframework` into your project
2. Add to "Frameworks, Libraries, and Embedded Content"
3. Import: `import ZrcIos`

## Requirements

- iOS 15.0+
- Xcode 14.0+
- Rust 1.70+
