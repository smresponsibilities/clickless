#!/bin/bash
set -euo pipefail

APP_NAME="Clickless"
APP_DIR=""
OUTPUT_ZIP=""
SKIP_BUILD=0

usage() {
    echo "Usage: package-macos.sh [--output <archive.zip>] [--app-dir <dir>] [--skip-build]" >&2
    exit "${1:-2}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            OUTPUT_ZIP="$2"
            shift 2
            ;;
        --app-dir)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            APP_DIR="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --help|-h)
            usage 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage 2
            ;;
    esac
done

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -z "$APP_DIR" ]]; then
    APP_DIR="target/release/$APP_NAME.app"
fi

if [[ -z "$OUTPUT_ZIP" ]]; then
    OUTPUT_ZIP="target/release/$APP_NAME-macos.zip"
fi

# Build app bundle if missing and build not skipped
if [[ ! -d "$APP_DIR" ]]; then
    if [[ "$SKIP_BUILD" -eq 1 ]]; then
        echo "Error: App bundle '$APP_DIR' does not exist and --skip-build was specified" >&2
        exit 1
    fi
    echo "Building macOS app bundle..."
    bash scripts/bundle-macos.sh
fi

if [[ ! -d "$APP_DIR" ]]; then
    echo "Error: App bundle not found at '$APP_DIR'" >&2
    exit 1
fi

echo "Applying ad-hoc codesign for \$0 open source distribution..."
for executable in clicklessctl clickless-settings "$APP_NAME"; do
    if [[ -f "$APP_DIR/Contents/MacOS/$executable" ]]; then
        codesign --force -s - "$APP_DIR/Contents/MacOS/$executable"
    fi
done
codesign --force -s - "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"

echo "Creating distribution archive at '$OUTPUT_ZIP'..."
mkdir -p "$(dirname "$OUTPUT_ZIP")"
rm -f "$OUTPUT_ZIP"

if command -v ditto >/dev/null 2>&1; then
    ditto -c -k --keepParent "$APP_DIR" "$OUTPUT_ZIP"
elif command -v zip >/dev/null 2>&1; then
    parent_dir="$(dirname "$APP_DIR")"
    base_app="$(basename "$APP_DIR")"
    abs_output="$(cd "$(dirname "$OUTPUT_ZIP")" && pwd)/$(basename "$OUTPUT_ZIP")"
    (cd "$parent_dir" && zip -r -q -y "$abs_output" "$base_app")
else
    echo "Error: Neither ditto nor zip found to create archive" >&2
    exit 1
fi

if [[ ! -f "$OUTPUT_ZIP" ]]; then
    echo "Error: Failed to create archive '$OUTPUT_ZIP'" >&2
    exit 1
fi

echo "Generating SHA256 checksum..."
if command -v shasum >/dev/null 2>&1; then
    SHA256=$(shasum -a 256 "$OUTPUT_ZIP" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
    SHA256=$(sha256sum "$OUTPUT_ZIP" | awk '{print $1}')
else
    echo "Error: Neither shasum nor sha256sum found" >&2
    exit 1
fi

printf "%s  %s\n" "$SHA256" "$(basename "$OUTPUT_ZIP")" > "$OUTPUT_ZIP.sha256"

echo "=============================================="
echo "macOS Distribution Bundle Packaged Successfully"
echo "Archive: $OUTPUT_ZIP"
echo "SHA256:  $SHA256"
echo "Checksum file: $OUTPUT_ZIP.sha256"
echo "=============================================="
