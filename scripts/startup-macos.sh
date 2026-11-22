#!/bin/bash
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then echo "Run on macOS" >&2; exit 1; fi
plist="$HOME/Library/LaunchAgents/org.smresponsibilities.clickless.plist"
domain="gui/$(id -u)"
prior_loaded=0
if launchctl print "$domain/org.smresponsibilities.clickless" >/dev/null 2>&1; then
  prior_loaded=1
else
  query_status=$?
  # launchctl uses 113 for a service absent from this domain.
  if [[ "$query_status" -ne 113 ]]; then
    echo "Cannot query prior agent state; no changes made" >&2; exit 1
  fi
fi
case "${1:-}" in
  --disable)
    if [[ "$prior_loaded" -eq 1 ]]; then
      launchctl bootout "$domain" "$plist" || { echo "Cannot stop agent; plist retained" >&2; exit 1; }
    fi
    if ! rm -f -- "$plist"; then
      if [[ "$prior_loaded" -eq 1 ]] && ! launchctl bootstrap "$domain" "$plist"; then
        echo "Recovery failure: could not restore loaded agent; plist retained" >&2
      fi
      exit 1
    fi
    ;;
  --enable)
    executable="${2:-}"
    if [[ "$executable" != /* || ! -x "$executable" ]]; then echo "Supply absolute path to Clickless executable" >&2; exit 1; fi
    if [[ "$prior_loaded" -eq 1 && ( ! -f "$plist" || ! -r "$plist" ) ]]; then
      echo "Loaded agent has no readable prior plist; cannot update safely" >&2; exit 1
    fi
    mkdir -p "$(dirname "$plist")"
    prior_plist=$(mktemp "${plist}.prior.XXXXXX")
    temporary=""
    prior_existed=0
    changed=0
    preserve_backup=0
    cleanup() {
      result=$?
      trap - EXIT
      if [[ "$result" -ne 0 && "$changed" -eq 1 ]]; then
        echo "Update failed, attempting rollback" >&2
        if [[ "$prior_existed" -eq 1 ]]; then
          if ! cp -p -- "$prior_plist" "$plist"; then
            preserve_backup=1
          elif [[ "$prior_loaded" -eq 1 ]] && ! launchctl bootstrap "$domain" "$plist"; then
            preserve_backup=1
          fi
        elif ! rm -f -- "$plist"; then
          preserve_backup=1
        fi
        if [[ "$preserve_backup" -eq 1 ]]; then
          echo "Recovery failure: backup retained at $prior_plist" >&2
        fi
      fi
      [[ -z "$temporary" ]] || rm -f -- "$temporary"
      if [[ "$preserve_backup" -eq 0 ]]; then rm -f -- "$prior_plist"; fi
      exit "$result"
    }
    trap cleanup EXIT
    if [[ -e "$plist" ]]; then
      [[ -f "$plist" && -r "$plist" ]] || { echo "Prior plist is not a readable file" >&2; exit 1; }
      cp -p -- "$plist" "$prior_plist"
      prior_existed=1
    fi
    temporary=$(mktemp "${plist}.XXXXXX")
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict/></plist>' > "$temporary"
    plutil -insert Label -string org.smresponsibilities.clickless "$temporary"
    plutil -insert ProgramArguments -json '[""]' "$temporary"
    plutil -replace ProgramArguments.0 -string "$executable" "$temporary"
    plutil -insert RunAtLoad -bool YES "$temporary"
    plutil -lint "$temporary"
    if [[ "$prior_loaded" -eq 1 ]]; then
      launchctl bootout "$domain" "$plist" || { echo "Cannot stop agent; plist retained" >&2; exit 1; }
    fi
    changed=1
    mv -f -- "$temporary" "$plist"
    launchctl bootstrap "$domain" "$plist"
    ;;
  *) echo "Usage: $0 --enable /absolute/path/Clickless | --disable" >&2; exit 1 ;;
esac
