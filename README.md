# Clickless

Rust keyboard-driven pointer control prototype. No runnable pointer-control session yet.

Current code includes a pure layer/motion engine, TOML parser, output adapter, and mock-tested input adapters. CLI validates configuration only. Settings and bindings are not wired into a running backend.

## Usage

From this directory:

```sh
cargo run -p clickless-cli -- --check-config
cargo run -p clickless-cli -- --check-config --config config.toml
cargo run -p clickless-cli -- --help
```

Invoking without `--check-config` reports the missing runtime and exits nonzero. Unknown arguments, including URLs, are rejected.

## Configuration

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
```

These values can be parsed and validated; they do not yet control a live pointer.

## Experimental grid and URLs

`GridNavigator` contains isolated grid calculations. No overlay renderer or backend dispatch consumes its actions. `[grid]` parses dimensions, keys, nudge settings, and auto-free-mode preferences. Dimensions must be positive with a representable cell count; key count must match, keys must be unique, and nudge step must be positive. This is not a working grid feature.

`parse_deep_link` recognizes `clickless://` and `mouseless://` command strings. No OS scheme registration, running-instance transport, or command handler exists. Launcher integration is not available. Do not register another application's `mouseless://` scheme.

## Next milestone

Complete current tri-platform build gates, then wire config into the flow engine and implement a real Windows input loop. Verify key passthrough, tap/hold, movement, button release, output errors, and cleanup before adding overlays or URL handling. Linux and macOS runtime support remain incomplete.

See [PARITY.md](PARITY.md) and [local tickets](../plan-baby-steps.md).

## Checks

```sh
cargo fmt --check
cargo build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Local Windows results do not establish Linux or macOS compilation or runtime support.
