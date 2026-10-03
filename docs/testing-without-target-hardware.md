# Test without owning Linux or macOS hardware

## What each check proves

| Check | Proves | Does not prove |
| --- | --- | --- |
| Windows unit tests | Shared engine/config behavior and mock backend cleanup | Linux/macOS native calls |
| Windows `cargo check --target` | Production source type-checks for target OS | Linking, executable tests, permissions, input capture |
| GitHub Ubuntu/macOS runners | Native builds, linking, unit/integration tests | Interactive capture, real keyboard behavior, permission grants, VoiceOver |
| WSL2 Ubuntu | Linux executable tests and Unix lifecycle behavior | Physical evdev keyboard, full Linux desktop acceptance |
| Linux desktop VM | X11 overlay/input checks with a dedicated virtual or USB-passthrough keyboard | Every physical device or Wayland compositor |
| Remote Mac desktop | Interactive AppKit, permission and keyboard checks on Apple hardware | Every keyboard/display combination |

Cross-platform completion requires both automated checks and recorded native desktop acceptance. A green build cannot close L01/M01 permission and input gates.

## Routine Windows validation and exhaustive explorer

The default Windows explorer uses 10,000 seeds with 1,000 events each and repeatedly saves files. For a bounded local audit run, keep all tests but record the reduced explorer budget:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:CLICKLESS_EXPLORER_SEEDS = '100'
$env:CLICKLESS_EXPLORER_EVENTS = '1000'
cargo test --workspace --all-targets --all-features
```

This is 100,000 explorer events, not the full 10-million-event proof. To run the exhaustive explorer separately:

```powershell
Remove-Item Env:CLICKLESS_EXPLORER_SEEDS -ErrorAction SilentlyContinue
Remove-Item Env:CLICKLESS_EXPLORER_EVENTS -ErrorAction SilentlyContinue
cargo test -p clickless-windows --test app_explorer seeded_fuzz_reports_every_failure
```

The audit's full-budget run was stopped during the filesystem explorer without completing it. Four existing screenshot/focus tests remain ignored by the workspace suite. Neither check is reported as passing.

## Run existing GitHub matrix

The checked-in workflow runs Ubuntu, Windows and macOS. This audit adds manual dispatch, Linux desktop build dependencies and a two-job compilation limit. Publish a reviewed branch before dispatching its workflow. Dispatching an older branch tests that branch's source, not local edits.

```powershell
gh workflow run ci.yml --ref <published-branch>
gh run list --workflow ci.yml --branch <published-branch> --limit 1
gh run watch <run-id> --exit-status
gh run view <run-id> --log-failed
```

Record commit SHA, run URL and each job result. Latest observed green run during audit was [37109509984](https://github.com/smresponsibilities/clickless/actions/runs/37109509984), commit `8ad572d14569926652fe4c1b77642d8cc55df8f8`. It does not cover local HEAD `3eedd7c` or dirty edits.

## Local Linux executable tests through WSL

Ubuntu is installed on this Windows machine. Audit found no Linux Rust installation and a failing systemd user-session startup. Resolve WSL startup, then open Ubuntu and install Rust with its official rustup installer. Keep compilation in Linux storage if mounted Windows storage is slow.

```bash
sudo apt-get update
sudo apt-get install -y build-essential clang cmake pkg-config curl libasound2-dev libfontconfig1-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcb-render0-dev libxcb-randr0-dev libxcb-keysyms1-dev libegl1-mesa-dev libvulkan-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/clickless-rustup.sh
sh /tmp/clickless-rustup.sh -y --profile minimal
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
cd /mnt/d/opensource/17-mouseless-rs/clickless
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR="$HOME/.cache/clickless-linux-target"
cargo fmt --all --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Focused Unix lifecycle proof:

```bash
cargo test -p clickless --test unix_settings_request
cargo test -p clickless-linux --test input_cleanup
```

These tests use child processes and mock pointer output. Do not launch normal Clickless capture in WSL to infer physical keyboard support. WSLg GUI rendering does not prove evdev/uinput access or native Wayland support.

## Windows compile checks

```powershell
rustup target add x86_64-unknown-linux-gnu x86_64-apple-darwin
$env:CARGO_BUILD_JOBS = '2'
cargo check --workspace --exclude clickless-gpui-sample --all-targets --all-features --target x86_64-unknown-linux-gnu
cargo check --workspace --exclude clickless-gpui-sample --all-targets --all-features --target x86_64-apple-darwin
```

GPUI sample depends on native desktop tooling. It stays in native CI gates; its exclusion here limits Windows cross-check claims to production crates. `cargo test --target` cannot execute a Linux or macOS binary on Windows.

## Desktop checks that remain

Use a Linux X11 VM with an isolated desktop and a recovery path outside the captured keyboard. Test typing, modifier chords, leader taps/holds, grid, drag, scroll, Escape, SIGTERM, device disconnect and injected output failures. Ordinary typing must arrive exactly once. Every held button must release. An unavailable overlay must reject startup before keyboard grab. Wayland needs a separate compositor-specific acceptance run.

Borrow a Mac or use an interactive remote Mac on Apple hardware. Test Accessibility deny/grant/revoke, supported ShiftLeft/ControlLeft leader, Command/Option/Ctrl shortcuts, Secure Input activation during movement/drag, quit cleanup, Retina/external displays and VoiceOver. Default CapsLock hold is currently rejected by the backend. A rented Mac reached only through SSH supplies builds/tests but cannot supply these interactive checks without a desktop session.

The GPUI sample remains a visual prototype. Linux Settings currently logs requests; macOS Settings is an AppKit prototype. Neither platform has a completed Home/Settings/Practice/tray product flow. App signing, notarization, fresh installation and update permission retention remain separate acceptance gates.

## Packaging checks

Ubuntu CI validates both Bash scripts with ShellCheck. It installs desktop metadata and every shipped hicolor icon into an isolated `XDG_DATA_HOME`, compares installed files, rejects relative paths and checks autostart stays absent. `scripts/install-linux.sh` installs desktop resources only. Install the executable on `PATH` separately.

macOS CI runs `bash scripts/bundle-macos.sh` on its native architecture. The script builds only `clickless` and `clicklessctl`, installs the required icon, and validates Info.plist. CI checks executable identity and runs the bundled console binary's `--version`. The Settings binary currently has no macOS implementation, so the bundle excludes it. Bundle remains unsigned. These checks establish bundle structure and linkage, not Finder launch, permission retention or Gatekeeper acceptance.
