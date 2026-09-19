# Clickless

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
