# Parity ledger

Last updated: 2026-09-19.

| Component | Current implementation | Runtime proof |
|---|---|---|
| Core layers and movement | Pure Rust, configurable leader/bindings/motion, unit tests | Unit tests passing |
| TOML configuration | Parser and validation with error reporting | Applied to runtime and verified in unit tests |
| Windows input | Low-level hook (`WH_KEYBOARD_LL`), message loop, configurable bindings, key suppression | Hook loop implemented, unit tested, available in CLI |
| Linux input | Key translation, configurable bindings, mock dispatch | Native evdev loop scaffolded |
| macOS input | Key translation, configurable bindings, mock dispatch | Native CGEventTap loop scaffolded |
| Enigo output | Adapter implementation, balanced clicks, error propagation, bounded `--smoke-output` path | Unit tested; smoke path not yet run against a live pointer |
| Grid/nudge | Pure core model wired to layer transitions, absolute nudges, release-to-click and optional drag-after-select dispatched by all three hooks | Unit tested; no live run |
| Grid entry | Leader hold plus Space by default; any key bindable to `enter_grid` | Unit tested |
| Grid overlay pixels | Shared pure rasterizer in `clickless-backend-api`: panels, borders, 5x7 labels, active-cell highlight, pointer marker | Unit tested on every platform |
| Grid overlay model | `OverlayFrame` carries cells, highlight and pointer; `OverlayBackend` trait with `NullOverlay` default | Unit tested |
| Windows grid overlay window | Transparent, click-through, topmost layered window, blitted from the shared rasterizer and installed by the CLI | Render path exercised in a unit test; no human has looked at it |
| Linux grid overlay window | X11 override-redirect ARGB window with an empty input shape, `put_image`, installed by the CLI | Module compiled and linted; no X session available to run it |
| Wayland grid overlay | Not implemented; needs a layer-shell client | None |
| macOS grid overlay window | Transparent borderless `NSWindow` with an `NSImageView` fed a `CGImage` of the shared buffer | Typechecked and linted for `x86_64-apple-darwin`; never run, and never linked on this host |
| Deep links | String parser | Validated |
| CLI | Config validation, help, version, `--smoke-output`, runtime loop on Windows | Validated and functional |

## Verification boundaries

- Windows gates, 2026-09-19 Ticket 016: fmt exit 0, build exit 0, full-workspace/all-target/all-feature Clippy with -D warnings exit 0, tests exit 0 with 177 passed. Cross-target `cargo check` and `cargo clippy -D warnings` also exit 0 for `x86_64-unknown-linux-gnu` and `x86_64-apple-darwin`.
- `--smoke-output` is never invoked by automated tests. Running it moves the real cursor by 40px each way, clicks left once at the start position and scrolls one notch down then up; the owner runs it as a bounded manual check.
- The X11 module was typechecked and linted on Windows by temporarily unscoping `x11rb` and widening its `cfg`; the checked-in tree scopes both to Linux again, so Windows builds skip that file.
- Cross-target checks, 2026-09-19: with the `x86_64-unknown-linux-gnu` and `x86_64-apple-darwin` targets installed, `cargo check --workspace --all-targets --target <t>` and `cargo clippy --workspace --all-targets --target <t> -- -D warnings` both exit 0 for each target. Linking and `cargo test` for those targets cannot run here, so they remain CI-only evidence.
- Remote CI, run 35458371915 on HEAD 23eda5e: windows-latest passed, ubuntu-latest and macos-latest failed at Build. Both failures are fixed in the working tree but not yet pushed, so no runner has compiled the fixes.
- Windows native hook installed and unregistered gracefully.
- Linux and macOS crates compile cleanly under workspace tests with full scancode translation and hook state machine integration.
- The Windows overlay window is created, rendered through GDI, blitted with `UpdateLayeredWindow`, and hidden in `clickless-windows` test `t10`. Pixel output is asserted in pure paint tests t01-t07. Nobody has visually confirmed the result, and Linux/macOS draw nothing.


## Dense grid, 2026-09-20

Application default is now dense two-character selection with subgrid refinement. Simple layout remains configurable. Windows local gates passed with 181 tests; renderer previews inspected. Native dense-grid visual/input check and remote CI remain open. Linux input currently lacks reinjection and its overlay uses X11; do not advertise Wayland readiness. See ../docs/mouseless-research.md.
