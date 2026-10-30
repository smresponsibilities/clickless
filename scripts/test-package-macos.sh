#!/bin/bash
# Deterministic packaging orchestration checks; no Apple services or real signing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
FIXTURE=$(mktemp -d)
trap 'rm -rf "$FIXTURE"' EXIT

mkdir -p "$FIXTURE/scripts" "$FIXTURE/bin" "$FIXTURE/target/release"
cp "$ROOT/scripts/package-macos.sh" "$FIXTURE/scripts/"

export FIXTURE CALLS="$FIXTURE/calls" FAIL=""
cat > "$FIXTURE/bin/stub" <<'STUB'
#!/bin/bash
set -eu
name=${0##*/}
echo "$name $*" >> "$CALLS"
[[ "$name $*" != *"$FAIL"* || -z "$FAIL" ]] || exit 19
case "$name" in
uname) echo Darwin;;
codesign) exit 0;;
ditto)
    if [[ "$1" == -c ]]; then
        touch "${@: -1}"
    fi
    ;;
shasum|sha256sum)
    echo "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  ${@: -1}"
    ;;
esac
STUB
chmod +x "$FIXTURE/bin/stub"

# Dummy bundle-macos.sh stub inside fixture
cat > "$FIXTURE/scripts/bundle-macos.sh" <<'BUNDLE'
#!/bin/bash
set -eu
echo "bundle-macos.sh $*" >> "$CALLS"
mkdir -p target/release/Clickless.app/Contents/MacOS
touch target/release/Clickless.app/Contents/MacOS/Clickless
touch target/release/Clickless.app/Contents/MacOS/clicklessctl
touch target/release/Clickless.app/Contents/MacOS/clickless-settings
BUNDLE
chmod +x "$FIXTURE/scripts/bundle-macos.sh"

for name in uname codesign ditto shasum sha256sum; do
    ln -s stub "$FIXTURE/bin/$name"
done
export PATH="$FIXTURE/bin:$PATH"

run() {
    : > "$CALLS"
    (cd "$FIXTURE" && bash "$FIXTURE/scripts/package-macos.sh" "$@" > "$FIXTURE/output" 2>&1)
}

# Test 1: Malformed and unknown arguments rejected
for args in '--unknown' '--output' '--app-dir'; do
    if run $args; then
        echo "Malformed args accepted: $args" >&2
        exit 1
    fi
done

# Test 2: Help flag exits 0
run --help
grep -q "Usage: package-macos.sh" "$FIXTURE/output"

# Test 3: --skip-build fails when bundle does not exist
rm -rf "$FIXTURE/target/release/Clickless.app"
if run --skip-build; then
    echo "--skip-build succeeded when bundle was missing" >&2
    exit 1
fi

# Test 4: Normal build creates bundle, ad-hoc signs, archives, and checksums
run
grep -q '^bundle-macos.sh' "$CALLS"
grep -q 'codesign --force -s - .*/Clickless.app/Contents/MacOS/Clickless' "$CALLS"
grep -q 'codesign --force -s - .*/Clickless.app/Contents/MacOS/clicklessctl' "$CALLS"
grep -q 'codesign --force -s - .*/Clickless.app/Contents/MacOS/clickless-settings' "$CALLS"
grep -q 'codesign --force -s - .*/Clickless.app' "$CALLS"
grep -q 'codesign --verify --deep --strict .*/Clickless.app' "$CALLS"
grep -q 'ditto -c -k --keepParent' "$CALLS"
grep -q 'shasum -a 256' "$CALLS"
test -f "$FIXTURE/target/release/Clickless-macos.zip.sha256"
grep -q "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" "$FIXTURE/target/release/Clickless-macos.zip.sha256"

# Test 5: Failure during codesign aborts
for FAIL in 'codesign --force' 'codesign --verify' 'ditto -c' 'shasum'; do
    export FAIL
    if run; then
        echo "Failure ignored: $FAIL" >&2
        exit 1
    fi
done

echo "package-macos orchestration checks passed"
