#!/bin/bash
set -euo pipefail

echo "Installing Clickless desktop files..."

cd "$(dirname "${BASH_SOURCE[0]}")/.."
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
if [[ "$DATA_DIR" != /* ]]; then
    echo "XDG_DATA_HOME must be an absolute path" >&2
    exit 1
fi
APP_DIR="$DATA_DIR/applications"
ICON_DIR="$DATA_DIR/icons/hicolor"

install -Dm644 crates/clickless-linux/clickless.desktop "$APP_DIR/clickless.desktop"
for source in assets/icons/linux/hicolor/*/apps/clickless.png; do
    size=$(basename "$(dirname "$(dirname "$source")")")
    install -Dm644 "$source" "$ICON_DIR/$size/apps/clickless.png"
done

update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true

echo "Desktop files installed. Install clickless on PATH separately. Autostart remains disabled."
