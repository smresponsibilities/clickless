#!/bin/bash
set -euo pipefail

echo "Installing Clickless desktop files..."

APP_DIR="C:\Users\sm/.local/share/applications"
ICON_DIR="C:\Users\sm/.local/share/icons/hicolor/scalable/apps"

mkdir -p ""
mkdir -p ""

if [ -f "crates/clickless-linux/clickless.desktop" ]; then
    cp "crates/clickless-linux/clickless.desktop" "/clickless.desktop"
fi

if [ -f "assets/icons/linux/clickless.svg" ]; then
    cp "assets/icons/linux/clickless.svg" "/clickless.svg"
fi

update-desktop-database "" 2>/dev/null || true
gtk-update-icon-cache -f -t "C:\Users\sm/.local/share/icons/hicolor" 2>/dev/null || true

echo "Installation complete. Autostart is disabled by default per L05."
