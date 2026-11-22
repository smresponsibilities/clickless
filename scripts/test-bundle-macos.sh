#!/bin/bash
# Deterministic orchestration checks; no Apple services or real signing.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
FIXTURE=$(mktemp -d)
trap 'rm -rf "$FIXTURE"' EXIT
mkdir -p "$FIXTURE/scripts" "$FIXTURE/bin" "$FIXTURE/assets/icons/macos"
cp "$ROOT/scripts/bundle-macos.sh" "$FIXTURE/scripts/"
touch "$FIXTURE/assets/icons/macos/clickless.icns"
export CALLS="$FIXTURE/calls" FAIL="" STATUS=Accepted
cat > "$FIXTURE/bin/stub" <<'STUB'
#!/bin/bash
set -eu
name=${0##*/}
echo "$name $*" >> "$CALLS"
[[ "$name $*" != *"$FAIL"* || -z "$FAIL" ]] || exit 19
case "$name" in
uname) echo Darwin;;
cargo)
 if [[ "$1" == pkgid ]]; then echo 'clickless@0.1.3'; else
 mkdir -p target/release
 touch target/release/clickless target/release/clicklessctl target/release/clickless-settings
 fi;;
xcrun)
 if [[ "$1" == notarytool ]]; then printf '{"status":"%s"}\n' "$STATUS"; fi;;
plutil)
 if [[ "$1" == -extract ]]; then echo "$STATUS"; fi;;
ditto)
 if [[ "$1" == -x ]]; then cp -R target/release/Clickless.app "$4/"; else touch "${@: -1}"; fi;;
esac
STUB
chmod +x "$FIXTURE/bin/stub"
for name in uname cargo codesign xcrun plutil ditto spctl; do ln -s stub "$FIXTURE/bin/$name"; done
export PATH="$FIXTURE/bin:$PATH"
run() { : > "$CALLS"; bash "$FIXTURE/scripts/bundle-macos.sh" "$@" > "$FIXTURE/output" 2>&1; }
for args in '--sign' '--notarize profile' '--unknown' '--sign identity --notarize' '--sign identity extra'; do
 if run $args; then echo "Malformed args accepted: $args" >&2; exit 1; fi
 if grep -q '^cargo ' "$CALLS"; then echo 'Built before argument validation' >&2; exit 1; fi
done
run
! grep -q '^codesign ' "$CALLS"
run --sign identity
! grep -q '^xcrun ' "$CALLS"
run --sign identity --notarize profile
grep -q 'codesign .*--timestamp.*Contents/MacOS/clicklessctl' "$CALLS"
grep -q 'codesign --verify --deep --strict' "$CALLS"
grep -q 'stapler validate' "$CALLS"
staple=$(grep -n 'stapler staple' "$CALLS" | cut -d: -f1)
archive=$(grep -n 'ditto -c.*Clickless.zip' "$CALLS" | cut -d: -f1)
[[ "$staple" -lt "$archive" ]]
grep -q 'codesign --verify.*bundle-verify' "$CALLS"
companion=$(grep -n 'codesign --force.*Contents/MacOS/clicklessctl' "$CALLS" | cut -d: -f1)
bundle=$(grep -n 'codesign --force.*Clickless.app$' "$CALLS" | cut -d: -f1)
[[ "$companion" -lt "$bundle" ]]
[[ $(grep -c 'stapler validate' "$CALLS") == 2 ]]
for FAIL in 'codesign --force' 'codesign --verify' 'notarytool submit' 'stapler staple' 'stapler validate' 'ditto -c'; do
 export FAIL
 if run --sign identity --notarize profile; then echo "Failure ignored: $FAIL" >&2; exit 1; fi
 ! grep -q '^ditto -x' "$CALLS"
done
for FAIL in 'codesign --verify --deep --strict --verbose=2 target/release/bundle-verify' 'spctl --assess'; do
 export FAIL
 if run --sign identity --notarize profile; then echo "Artifact failure ignored: $FAIL" >&2; exit 1; fi
done
export FAIL="" STATUS=Invalid
if run --sign identity --notarize profile; then echo 'Rejected notarization accepted' >&2; exit 1; fi
! grep -q 'stapler staple' "$CALLS"
echo 'bundle orchestration checks passed'
