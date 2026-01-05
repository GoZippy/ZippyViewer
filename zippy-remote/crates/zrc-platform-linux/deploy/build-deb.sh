#!/bin/bash
# Build script for .deb package

set -e

VERSION=${1:-0.1.0}
ARCH=$(dpkg --print-architecture)

# Create package directory
PACKAGE_DIR="zrc-agent_${VERSION}_${ARCH}"
mkdir -p "${PACKAGE_DIR}/DEBIAN"
mkdir -p "${PACKAGE_DIR}/usr/bin"
mkdir -p "${PACKAGE_DIR}/usr/lib/systemd/system"
mkdir -p "${PACKAGE_DIR}/var/lib/zrc-agent"

# Build binary
cargo build --release --package zrc-agent
cp target/release/zrc-agent "${PACKAGE_DIR}/usr/bin/"

# Copy systemd service
cp deploy/zrc-agent.service "${PACKAGE_DIR}/usr/lib/systemd/system/"

# Create control file
cat > "${PACKAGE_DIR}/DEBIAN/control" <<EOF
Package: zrc-agent
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: ZRC Team <zrc@example.com>
Description: Zippy Remote Control Agent
 ZRC Agent provides remote desktop control capabilities.
Depends: libc6, systemd
EOF

# Create postinst script
cat > "${PACKAGE_DIR}/DEBIAN/postinst" <<EOF
#!/bin/bash
set -e
systemctl daemon-reload
systemctl enable zrc-agent.service || true
EOF
chmod +x "${PACKAGE_DIR}/DEBIAN/postinst"

# Create prerm script
cat > "${PACKAGE_DIR}/DEBIAN/prerm" <<EOF
#!/bin/bash
set -e
systemctl stop zrc-agent.service || true
systemctl disable zrc-agent.service || true
EOF
chmod +x "${PACKAGE_DIR}/DEBIAN/prerm"

# Build package
dpkg-deb --build "${PACKAGE_DIR}"

echo "Package built: ${PACKAGE_DIR}.deb"
