# Clickless

Clickless is a keyboard-driven pointer controller written in Rust. Hold an activation key to move, click, scroll, drag, or target an on-screen grid without reaching for a mouse.

Windows is the current native test platform. Linux and macOS backends compile, but native behavior remains unverified. Wayland is not supported. See [PARITY.md](PARITY.md).

## Features

- Windows global keyboard hook and pointer output
- Continuous H/J/K/L movement with configurable acceleration
- Click, scroll, speed control, and safe drag cleanup
- Dense two-letter grid with a 3×10 nested target grid
- Optional simple grid
- Tray lifecycle, pause/resume, single instance, and native Settings
- Validated TOML, runtime Apply, atomic Save, reset, search, diagnostics, and practice

Current Settings/lifecycle work is uncommitted and still needs the native checks in [CONTRIBUTING.md](CONTRIBUTING.md).

## Why 2 Windows executables?

Release builds produce:

- `clickless.exe`: normal GUI-subsystem tray app. Explorer launch creates no command window.
- `clicklessctl.exe`: console diagnostics. Help, validation, errors, and smoke output remain readable.

Both call the same `clickless_cli::run` implementation. Runtime behavior is not duplicated.

```powershell
cargo build --release -p clickless-cli
./target/release/clickless.exe
./target/release/clicklessctl.exe --help
```

## Basic use

1. Tap `Left Shift` to open the grid, then choose its labels.
2. Or hold `CapsLock` for 200 ms. The grid remains visible until you release CapsLock.
3. Choose an outer-grid label, then a nested-grid label for the precise target.
4. Press `Esc` to cancel a grid. Releasing CapsLock also closes a CapsLock-held grid.
5. Tap `Left Ctrl` to toggle free mode. Use `H`, `J`, `K`, `L` to move and `F`/`D` to click.

`Backspace` moves back one grid level. `Space` at a selected parent clicks its center. With nudge enabled, use `H/J/K/L` or `A/S/W/D` for pixel adjustment, then release the final selection key to click.

## Default pointer bindings

| Key | Action |
|---|---|
| H/J/K/L | Move left/right/up/down |
| F/D | Left/right click |
| W/S | Scroll up/down |
| U/O | Reduce/increase speed |
| Space | Open grid |
| Esc | Cancel capture |

## Settings and tray

Open Settings from the tray or launch `clickless.exe` again. The second process signals the existing instance; it must not install another hook or tray icon.

Settings covers activation, motion, bindings, grid behavior, appearance, validation, reset, search, diagnostics, and practice. Closing Settings hides it. Tray Quit releases drag, hides overlays, unregisters the hook, and exits.

## Configuration

Validate a file with the console tool:

```powershell
./target/release/clicklessctl.exe --check-config --config clickless.toml
```

Minimal example:

```toml
[settings]
enabled = true
leader = "capslock"
hold_ms = 200
start_speed_px_s = 300
max_speed_px_s = 3000
ramp_ms = 500
scroll_step = 1

[grid]
layout = "dense"
nudge_enabled = true
nudge_step_px = 1
drag_after_select = false
auto_free_mode_after_move = false

[layers.mouse]
h = "move_left"
j = "move_right"
k = "move_up"
l = "move_down"
f = "click_left"
d = "click_right"
w = "scroll_up"
s = "scroll_down"
u = "speed_down"
o = "speed_up"
space = "enter_grid"
```

Presets live in `configs/`.

## Diagnostics

```powershell
./target/release/clicklessctl.exe --help
./target/release/clicklessctl.exe --version
./target/release/clicklessctl.exe --check-config
./target/release/clicklessctl.exe --smoke-output
```

`--smoke-output` moves the real pointer, clicks once, scrolls, and restores the pointer. Run it only for an intentional native check.

## Development

```powershell
cargo fmt --all --check
cargo build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --skip bfs_covers_canonical_states --skip seeded_fuzz_reports_every_failure
```

The bounded state explorer and seeded fuzz test are slow in debug builds. Run them separately before release acceptance. [CONTRIBUTING.md](CONTRIBUTING.md) defines ticket-specific checks.

## Architecture

| Crate | Responsibility |
|---|---|
| `clickless-core` | Pure layers, movement, grid navigation, actions |
| `clickless-config` | TOML, validation, serialization, atomic save |
| `clickless-backend-api` | Backend interfaces and shared rasterizer |
| `clickless-output-enigo` | Pointer output adapter |
| `clickless-windows` | Windows hook, overlay, tray, lifecycle, Settings |
| `clickless-linux` | Linux backend |
| `clickless-macos` | macOS backend |
| `clickless-cli` | Shared runtime and both entry points |

OS-specific code stays in platform crates. Core and configuration remain platform-independent.

## Known gaps

- Dirty Settings work needs a native keyboard, focus, foreground, DPI, and Narrator pass.
- Remote CI has not verified the latest revision.
- Linux/macOS native behavior is unverified.
- Windows theme/high-contrast polish remains open.
- No license file exists yet; add one before public distribution or external contributions.
