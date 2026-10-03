#!/bin/bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
if [[ "$(uname -s)" != Darwin ]]; then
    echo "Run this script on macOS to build a native app bundle" >&2
    exit 1
fi

APP_NAME="Clickless"
BUNDLE_ID="org.smresponsibilities.clickless"
PACKAGE_ID=$(cargo pkgid -p clickless)
VERSION="${PACKAGE_ID##*@}"
APP_DIR="target/release/$APP_NAME.app"
export MACOSX_DEPLOYMENT_TARGET=10.15

echo "Building release binary..."
cargo build --locked --release -p clickless --bin clickless --bin clicklessctl

echo "Creating bundle structure..."
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

echo "Copying binary..."
cp target/release/clickless "$APP_DIR/Contents/MacOS/$APP_NAME"
cp target/release/clicklessctl "$APP_DIR/Contents/MacOS/clicklessctl"

echo "Copying icon..."
cp assets/icons/macos/clickless.icns "$APP_DIR/Contents/Resources/AppIcon.icns"

echo "Generating Info.plist..."
cat <<EOF > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSAccessibilityUsageDescription</key>
    <string>Clickless needs Accessibility access to control the pointer with the keyboard.</string>
</dict>
</plist>
EOF

plutil -lint "$APP_DIR/Contents/Info.plist"
echo "Unsigned native app bundle created at $APP_DIR. Signing/notarization required for distribution."

