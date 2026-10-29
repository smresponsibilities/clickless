#!/bin/bash
set -euo pipefail

IDENTITY=""
PROFILE=""
case "$#" in
    0) ;;
    2|4)
        if [[ "$1" != --sign || -z "$2" || "$2" == --* ]]; then
            echo 'Usage: bundle-macos.sh [--sign IDENTITY [--notarize KEYCHAIN_PROFILE]]' >&2
            exit 2
        fi
        IDENTITY="$2"
        if [[ "$#" == 4 ]]; then
            if [[ "$3" != --notarize || -z "$4" || "$4" == --* ]]; then
                echo 'Notarization requires --notarize KEYCHAIN_PROFILE after --sign IDENTITY' >&2
                exit 2
            fi
            PROFILE="$4"
        fi
        ;;
    *) echo 'Usage: bundle-macos.sh [--sign IDENTITY [--notarize KEYCHAIN_PROFILE]]' >&2; exit 2 ;;
esac

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
export MACOSX_DEPLOYMENT_TARGET=11.0

echo "Building release binary..."
cargo build --locked --release -p clickless --features gpui-settings --bins

echo "Creating bundle structure..."
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

echo "Copying binary..."
cp target/release/clickless "$APP_DIR/Contents/MacOS/$APP_NAME"
cp target/release/clicklessctl "$APP_DIR/Contents/MacOS/clicklessctl"
cp target/release/clickless-settings "$APP_DIR/Contents/MacOS/clickless-settings"

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
    <string>11.0</string>
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


if [[ -n "$IDENTITY" ]]; then
    # Temporary archives cannot be confused with the final distributable.
    WORK_DIR=$(mktemp -d "target/release/bundle-verify.XXXXXX")
    trap 'rm -rf "$WORK_DIR"' EXIT
    echo "Signing companion binaries and main bundle..."
    for executable in clicklessctl clickless-settings "$APP_NAME"; do
        codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP_DIR/Contents/MacOS/$executable"
    done
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP_DIR"
    codesign --verify --deep --strict --verbose=2 "$APP_DIR"
    if [[ -n "$PROFILE" ]]; then
        echo "Submitting for notarization..."
        ditto -c -k --keepParent "$APP_DIR" "$WORK_DIR/submission.zip"
        xcrun notarytool submit "$WORK_DIR/submission.zip" --keychain-profile "$PROFILE" --wait --timeout 30m --output-format json > "$WORK_DIR/notary.json"
        STATUS=$(plutil -extract status raw -o - "$WORK_DIR/notary.json")
        if [[ "$STATUS" != Accepted ]]; then
            echo "Notarization not accepted: $STATUS" >&2
            exit 1
        fi
        echo "Stapling notarization ticket..."
        xcrun stapler staple "$APP_DIR"
        xcrun stapler validate "$APP_DIR"
    fi
    echo "Creating and verifying distribution ZIP..."
    ditto -c -k --keepParent "$APP_DIR" "$WORK_DIR/$APP_NAME.zip"
    mkdir "$WORK_DIR/extracted"
    ditto -x -k "$WORK_DIR/$APP_NAME.zip" "$WORK_DIR/extracted"
    codesign --verify --deep --strict --verbose=2 "$WORK_DIR/extracted/$APP_NAME.app"
    if [[ -n "$PROFILE" ]]; then
        xcrun stapler validate "$WORK_DIR/extracted/$APP_NAME.app"
        spctl --assess --type execute --verbose=2 "$WORK_DIR/extracted/$APP_NAME.app"
    fi
    mv "$WORK_DIR/$APP_NAME.zip" "target/release/$APP_NAME.zip"
    echo "Verified distribution ZIP: target/release/$APP_NAME.zip"
fi
