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
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"
if [[ "$CONFIG_DIR" != /* ]]; then
    echo "XDG_CONFIG_HOME must be an absolute path" >&2
    exit 1
fi
case "${1:-}" in
    --enable-autostart)
        mkdir -p "$CONFIG_DIR/autostart"
        temporary=$(mktemp "$CONFIG_DIR/autostart/clickless.desktop.XXXXXX")
        trap 'rm -f -- "$temporary"' EXIT
        sed 's/X-GNOME-Autostart-enabled=false/X-GNOME-Autostart-enabled=true/' crates/clickless-linux/clickless.desktop > "$temporary"
        chmod 644 "$temporary"
        mv -f -- "$temporary" "$CONFIG_DIR/autostart/clickless.desktop"
        ;;
    --disable-autostart) rm -f -- "$CONFIG_DIR/autostart/clickless.desktop" ;;
    '') ;;
    *) echo "Usage: $0 [--enable-autostart|--disable-autostart]" >&2; exit 1 ;;
esac

install -Dm644 crates/clickless-linux/clickless.desktop "$APP_DIR/clickless.desktop"
for source in assets/icons/linux/hicolor/*/apps/clickless.png; do
    size=$(basename "$(dirname "$(dirname "$source")")")
    install -Dm644 "$source" "$ICON_DIR/$size/apps/clickless.png"
done

update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true

echo "Desktop files installed. Install clickless on PATH separately."
