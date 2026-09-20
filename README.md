# Clickless


Outer grid remains 10x30 wide two-character cells. After two letters, the chosen cell contains a 3x10 keyboard subgrid, with the surrounding grid still visible. Press/release a third key to click that small target; Space clicks the parent center. Default subgrid keys: QWERTYUIOP / ASDFGHJKL; / ZXCVBNM,./. Old square-outer behavior is removed.

## Default dense grid

Hold CapsLock for 200ms, press Space, then type the two-character label at your target. Space clicks that cell's center. Alternatively press and hold a subgrid key, nudge with H/J/K/L or A/S/W/D, then release to click. Nudge directions are left/down/up/right. Backspace undoes selection; Escape or releasing CapsLock cancels. Release each selection key before typing it again.

```sh
# Dense grid is the application default.
cargo run -p clickless-cli
# Explicit dense preset with editable row/column and subgrid keys:
cargo run -p clickless-cli -- --config configs/grid.toml
# Previous simple 3x3 workflow:
cargo run -p clickless-cli -- --config configs/grid-simple.toml
```

Set `[grid] layout = "dense"` or `"simple"`. Dense `column_keys` and `row_keys` supply the first and second label characters. They determine the outer columns and rows. `rows`, `cols`, and `keys` describe the subgrid. Choice is currently TOML-based; no graphical settings editor is included.

Native dense-grid UI still needs a live check. Renderer previews use the real rasterizer:

```sh
cargo run -p clickless-backend-api --example grid_preview
cargo run -p clickless-backend-api --example grid_preview -- --selected
```

See [Mouseless and Wayland research](../docs/mouseless-research.md). Input permissions alone do not make Clickless Wayland-ready.

Cross-platform keyboard-driven pointer control in Rust.

Current version includes a pure layer/motion engine, TOML configuration validator, Enigo output backend, and platform input hooks. On Windows, `clickless` provides a functional runtime hook (`WH_KEYBOARD_LL`) with hold-to-activate leader key (CapsLock), continuous HJKL navigation with speed ramping, left/right clicks, and scrolling.

## Usage

From this directory:

```sh
# Run pointer runtime (Windows)
cargo run -p clickless-cli

# Run with custom config
cargo run -p clickless-cli -- -c clickless.toml

# Validate configuration without starting runtime
cargo run -p clickless-cli -- --check-config
cargo run -p clickless-cli -- --check-config --config clickless.toml

# Help & version
cargo run -p clickless-cli -- --help
cargo run -p clickless-cli -- --version
```

## Default bindings

When running in mouse layer (hold CapsLock for 200ms):
- `H`: Move left
- `J`: Move right
- `K`: Move up
- `L`: Move down
- `F`: Click left button
- `D`: Click right button
- `W`: Scroll up
- `S`: Scroll down
- `U`: Halve speed multiplier (SpeedDown)
- `O`: Double speed multiplier (SpeedUp)
- `Esc` or release `CapsLock`: Exit mouse layer

## Configuration

Example `clickless.toml`:

```toml
[settings]
leader = "capslock"
start_speed_px_s = 300
max_speed_px_s = 3000
ramp_ms = 500

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
```

## Checks & verification

```sh
cargo fmt --all --check
cargo build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

See [PARITY.md](PARITY.md) for platform support details.



For normal use, prefer a release build and run only one Clickless instance. See [grid lag audit](../docs/grid-lag-audit.md) for measured raster costs and remaining hook-path findings.


