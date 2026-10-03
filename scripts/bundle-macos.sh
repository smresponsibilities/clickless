#!/bin/bash
set -euo pipefail

APP_NAME="Clickless"
BUNDLE_ID="org.smresponsibilities.clickless"
VERSION=$(grep -m1 "^version =" Cargo.toml | cut -d "\"" -f2)
if [ -z "$VERSION" ]; then VERSION="0.1.0"; fi
APP_DIR="target/release/$APP_NAME.app"
BIN_NAME="clickless"

echo "Building release binary..."
cargo build --release --target x86_64-apple-darwin

echo "Creating bundle structure..."
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

echo "Copying binary..."
cp "target/x86_64-apple-darwin/release/$BIN_NAME" "$APP_DIR/Contents/MacOS/$APP_NAME"

echo "Copying icon..."
if [ -f "assets/icons/macos/clickless.icns" ]; then
    cp "assets/icons/macos/clickless.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
fi

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

echo "App bundle created at $APP_DIR"

