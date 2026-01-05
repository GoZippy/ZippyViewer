#!/bin/bash
# Build script for AppImage

set -e

VERSION=${1:-0.1.0}
APP_NAME="zrc-agent"

# Create AppDir structure
APPDIR="${APP_NAME}.AppDir"
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Build binary
cargo build --release --package zrc-agent
cp target/release/zrc-agent "${APPDIR}/usr/bin/"

# Copy dependencies (simplified - in practice, use ldd to find all deps)
# This is a placeholder - you'd need to bundle all required libraries

# Create desktop file
cat > "${APPDIR}/usr/share/applications/${APP_NAME}.desktop" <<EOF
[Desktop Entry]
Name=ZRC Agent
Comment=Zippy Remote Control Agent
Exec=zrc-agent
Icon=zrc-agent
Type=Application
Categories=Network;
EOF

# Create AppRun
cat > "${APPDIR}/AppRun" <<'EOF'
#!/bin/bash
HERE="$(dirname "$(readlink -f "${0}")")"
exec "${HERE}/usr/bin/zrc-agent" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

# Create icon (placeholder)
# In practice, you'd include an actual icon file

# Download and use appimagetool
# wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
# chmod +x appimagetool-x86_64.AppImage
# ./appimagetool-x86_64.AppImage "${APPDIR}"

echo "AppDir created: ${APPDIR}"
echo "To create AppImage, use appimagetool:"
echo "  appimagetool ${APPDIR}"
