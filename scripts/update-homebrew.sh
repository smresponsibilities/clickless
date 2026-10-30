#!/bin/bash
set -euo pipefail

VERSION=""
SHA256=""
FORMULA_SHA256=""
ZIP_PATH=""
TAR_GZ_PATH=""
CASK_PATH=""
FORMULA_PATH=""
TAP_DIR=""

usage() {
    echo "Usage: update-homebrew.sh [--version <version>] [--sha256 <sha256>] [--formula-sha256 <sha256>] [--zip <path>] [--tar-gz <path>] [--cask <path>] [--formula <path>] [--tap-dir <path>]" >&2
    exit "${1:-2}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            VERSION="$2"
            shift 2
            ;;
        --sha256)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            SHA256="$2"
            shift 2
            ;;
        --formula-sha256)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            FORMULA_SHA256="$2"
            shift 2
            ;;
        --zip)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            ZIP_PATH="$2"
            shift 2
            ;;
        --tar-gz)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            TAR_GZ_PATH="$2"
            shift 2
            ;;
        --cask)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            CASK_PATH="$2"
            shift 2
            ;;
        --formula)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            FORMULA_PATH="$2"
            shift 2
            ;;
        --tap-dir)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || usage 2
            TAP_DIR="$2"
            shift 2
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

ORIG_PWD="$PWD"
if [[ -n "$TAP_DIR" && "$TAP_DIR" != /* && "$TAP_DIR" != [A-Za-z]:* ]]; then
    TAP_DIR="$ORIG_PWD/$TAP_DIR"
fi
if [[ -n "$ZIP_PATH" && "$ZIP_PATH" != /* && "$ZIP_PATH" != [A-Za-z]:* ]]; then
    ZIP_PATH="$ORIG_PWD/$ZIP_PATH"
fi
if [[ -n "$TAR_GZ_PATH" && "$TAR_GZ_PATH" != /* && "$TAR_GZ_PATH" != [A-Za-z]:* ]]; then
    TAR_GZ_PATH="$ORIG_PWD/$TAR_GZ_PATH"
fi
if [[ -n "$CASK_PATH" && "$CASK_PATH" != /* && "$CASK_PATH" != [A-Za-z]:* && -f "$ORIG_PWD/$CASK_PATH" ]]; then
    CASK_PATH="$ORIG_PWD/$CASK_PATH"
fi
if [[ -n "$FORMULA_PATH" && "$FORMULA_PATH" != /* && "$FORMULA_PATH" != [A-Za-z]:* && -f "$ORIG_PWD/$FORMULA_PATH" ]]; then
    FORMULA_PATH="$ORIG_PWD/$FORMULA_PATH"
fi

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -z "$VERSION" ]]; then
    if [[ -f Cargo.toml ]]; then
        VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/' || true)
    fi
    if [[ -z "$VERSION" ]]; then
        echo "Error: Could not determine version from Cargo.toml. Specify --version <ver>" >&2
        exit 1
    fi
fi

if [[ -z "$ZIP_PATH" ]]; then
    ZIP_PATH="target/release/Clickless-macos.zip"
fi

if [[ -z "$SHA256" ]]; then
    if [[ -f "$ZIP_PATH.sha256" ]]; then
        SHA256=$(awk '{print $1}' "$ZIP_PATH.sha256")
    elif [[ -f "$ZIP_PATH" ]]; then
        if command -v shasum >/dev/null 2>&1; then
            SHA256=$(shasum -a 256 "$ZIP_PATH" | awk '{print $1}')
        elif command -v sha256sum >/dev/null 2>&1; then
            SHA256=$(sha256sum "$ZIP_PATH" | awk '{print $1}')
        fi
    fi
fi

if [[ -z "$FORMULA_SHA256" && -n "$TAR_GZ_PATH" && -f "$TAR_GZ_PATH" ]]; then
    if command -v shasum >/dev/null 2>&1; then
        FORMULA_SHA256=$(shasum -a 256 "$TAR_GZ_PATH" | awk '{print $1}')
    elif command -v sha256sum >/dev/null 2>&1; then
        FORMULA_SHA256=$(sha256sum "$TAR_GZ_PATH" | awk '{print $1}')
    fi
fi

if [[ -z "$CASK_PATH" ]]; then
    CASK_PATH="Casks/clickless.rb"
fi

if [[ -z "$FORMULA_PATH" ]]; then
    FORMULA_PATH="Formula/clickless.rb"
fi

if [[ ! -f "$CASK_PATH" ]]; then
    echo "Error: Cask file '$CASK_PATH' not found" >&2
    exit 1
fi

echo "Updating Homebrew Cask '$CASK_PATH' to version $VERSION..."
sed -i.bak -E "s/version \"[^\"]+\"/version \"$VERSION\"/" "$CASK_PATH"
rm -f "$CASK_PATH.bak"

if [[ -n "$SHA256" ]]; then
    echo "Updating SHA256 in Cask: $SHA256"
    sed -i.bak -E "s/sha256 \"[^\"]+\"/sha256 \"$SHA256\"/" "$CASK_PATH"
    rm -f "$CASK_PATH.bak"
else
    echo "Warning: No SHA256 computed or provided. Retaining existing sha256." >&2
fi

if [[ -f "$FORMULA_PATH" ]]; then
    echo "Updating Homebrew Formula '$FORMULA_PATH' version reference..."
    sed -i.bak -E "s|/archive/refs/tags/v[0-9.]+\.tar\.gz|/archive/refs/tags/v$VERSION.tar.gz|" "$FORMULA_PATH"
    rm -f "$FORMULA_PATH.bak"

    if [[ -n "$FORMULA_SHA256" ]]; then
        echo "Updating SHA256 in Formula: $FORMULA_SHA256"
        sed -i.bak -E "s/sha256 \"[^\"]+\"/sha256 \"$FORMULA_SHA256\"/" "$FORMULA_PATH"
        rm -f "$FORMULA_PATH.bak"
    fi
fi

if [[ -n "$TAP_DIR" ]]; then
    echo "Synchronizing formulas to Tap directory '$TAP_DIR'..."
    mkdir -p "$TAP_DIR/Casks" "$TAP_DIR/Formula"
    cp "$CASK_PATH" "$TAP_DIR/Casks/clickless.rb"
    if [[ -f "$FORMULA_PATH" ]]; then
        cp "$FORMULA_PATH" "$TAP_DIR/Formula/clickless.rb"
    fi
fi

echo "Homebrew update completed successfully."
