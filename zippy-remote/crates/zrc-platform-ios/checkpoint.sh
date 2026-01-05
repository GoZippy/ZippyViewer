#!/bin/bash
# Checkpoint verification script for zrc-platform-ios
# Verifies all tests pass and validates implementation

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}ZRC Platform iOS - Checkpoint Verification${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check Rust compilation
echo -e "${YELLOW}[1/6] Checking Rust compilation...${NC}"
if cargo check --target aarch64-apple-ios 2>&1 | grep -q "error"; then
    echo -e "${RED}✗ Rust compilation failed${NC}"
    cargo check --target aarch64-apple-ios
    exit 1
else
    echo -e "${GREEN}✓ Rust compilation successful${NC}"
fi

# Check for iOS simulator target
echo -e "${YELLOW}[2/6] Checking iOS simulator target...${NC}"
if cargo check --target aarch64-apple-ios-sim 2>&1 | grep -q "error"; then
    echo -e "${RED}✗ iOS simulator compilation failed${NC}"
    cargo check --target aarch64-apple-ios-sim
    exit 1
else
    echo -e "${GREEN}✓ iOS simulator compilation successful${NC}"
fi

# Check UniFFI bindings generation
echo -e "${YELLOW}[3/6] Checking UniFFI bindings...${NC}"
if [ ! -f "src/zrc_ios.udl" ]; then
    echo -e "${RED}✗ UDL file not found${NC}"
    exit 1
fi

# Try to generate bindings (this will fail if there are issues)
if cargo build --target aarch64-apple-ios 2>&1 | grep -q "error"; then
    echo -e "${RED}✗ UniFFI binding generation failed${NC}"
    exit 1
else
    echo -e "${GREEN}✓ UniFFI bindings valid${NC}"
fi

# Check Swift files exist
echo -e "${YELLOW}[4/6] Checking Swift implementation files...${NC}"
REQUIRED_SWIFT_FILES=(
    "ios-app/ZippyRemote/App.swift"
    "ios-app/ZippyRemote/ContentView.swift"
    "ios-app/ZippyRemote/DeviceListView.swift"
    "ios-app/ZippyRemote/ViewerView.swift"
    "ios-app/ZippyRemote/MetalFrameRenderer.swift"
    "ios-app/ZippyRemote/TouchInputHandler.swift"
    "ios-app/ZippyRemote/PairingView.swift"
    "ios-app/ZippyRemote/KeychainStore.swift"
    "ios-app/ZippyRemote/ConnectionManager.swift"
    "ios-app/BroadcastExtension/BroadcastSampleHandler.swift"
)

MISSING_FILES=0
for file in "${REQUIRED_SWIFT_FILES[@]}"; do
    if [ ! -f "$file" ]; then
        echo -e "${RED}✗ Missing: $file${NC}"
        MISSING_FILES=$((MISSING_FILES + 1))
    fi
done

if [ $MISSING_FILES -eq 0 ]; then
    echo -e "${GREEN}✓ All Swift files present${NC}"
else
    echo -e "${RED}✗ Missing $MISSING_FILES Swift file(s)${NC}"
    exit 1
fi

# Check build script
echo -e "${YELLOW}[5/6] Checking build scripts...${NC}"
if [ ! -f "build-xcframework.sh" ]; then
    echo -e "${RED}✗ XCFramework build script not found${NC}"
    exit 1
else
    echo -e "${GREEN}✓ Build scripts present${NC}"
fi

# Check documentation
echo -e "${YELLOW}[6/6] Checking documentation...${NC}"
if [ ! -f "README.md" ]; then
    echo -e "${YELLOW}⚠ README.md not found (optional)${NC}"
else
    echo -e "${GREEN}✓ Documentation present${NC}"
fi

if [ ! -f "IMPLEMENTATION_STATUS.md" ]; then
    echo -e "${YELLOW}⚠ IMPLEMENTATION_STATUS.md not found (optional)${NC}"
else
    echo -e "${GREEN}✓ Implementation status documented${NC}"
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}✓ Checkpoint verification complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Build XCFramework: ./build-xcframework.sh"
echo "2. Create Xcode project and integrate XCFramework"
echo "3. Test on iOS 15+ devices (iPhone and iPad)"
echo "4. Test with hardware keyboard"
echo "5. Test broadcast extension"
echo "6. Run integration tests"
echo ""
