# Parity ledger

| Component | Current implementation | Runtime proof |
|---|---|---|
| Core layers and movement | Pure Rust, configurable leader/bindings/motion, unit tests | Unit tests passing |
| TOML configuration | Parser and validation with error reporting | Applied to runtime and verified in unit tests |
| Windows input | Low-level hook (`WH_KEYBOARD_LL`), message loop, configurable bindings, key suppression | Hook loop implemented, unit tested, available in CLI |
| Linux input | Key translation, configurable bindings, mock dispatch | Native evdev loop scaffolded |
| macOS input | Key translation, configurable bindings, mock dispatch | Native CGEventTap loop scaffolded |
| Enigo output | Adapter implementation, balanced clicks, error propagation, bounded `--smoke-output` path | Unit tested; smoke path verified |
| Grid/nudge | Pure core model wired to layer transitions, absolute nudges, release-to-click and optional drag-after-select dispatched by all three hooks | Unit tested |
| Grid entry | Leader hold plus Space by default; any key bindable to `enter_grid` | Unit tested |
| Grid overlay pixels | Shared pure rasterizer in `clickless-core`: panels, borders, 5x7 labels, active-cell highlight, pointer marker | Unit tested on every platform |
| Grid overlay model | `OverlayFrame` carries cells, highlight and pointer; `OverlayBackend` trait with `NullOverlay` default | Unit tested |
| Windows grid overlay window | Transparent, click-through, topmost layered window, blitted from the shared rasterizer and installed by the CLI | Render path exercised in unit tests |
| Linux grid overlay window | X11 override-redirect ARGB window with an empty input shape, `put_image`, installed by the CLI | Module compiled and linted |
| Wayland grid overlay | Not implemented; needs a layer-shell client | None |
| macOS grid overlay window | Transparent borderless `NSWindow` with an `NSImageView` fed a `CGImage` of the shared buffer | Typechecked and linted |
| Deep links | String parser | Validated |
| CLI | Config validation, help, version, `--smoke-output`, runtime loop, GPUI settings | Validated and functional |

## Verification boundaries

- 3-crate consolidated workspace: `clickless-core`, `clickless-platform`, `clickless`.
- Windows gates: fmt exit 0, build exit 0, full-workspace Clippy with -D warnings exit 0, 440 tests passing.
- `--smoke-output` moves the real cursor by 40px each way, clicks left once at the start position and scrolls one notch down then up; available as a bounded check.
- Linux and macOS modules in `clickless-platform` compile cleanly under workspace tests with full scancode translation and hook state machine integration.
- The Windows overlay window is created, rendered through GDI, blitted with `UpdateLayeredWindow`, and hidden in `clickless-platform`. Pixel output is asserted in pure paint tests.


## Dense grid, 2026-09-20

Application default is now dense two-character selection with subgrid refinement. Simple layout remains configurable. Windows local gates passed with 181 tests; renderer previews inspected. Native dense-grid visual/input check and remote CI remain open. Linux input currently lacks reinjection and its overlay uses X11; do not advertise Wayland readiness. See ../docs/mouseless-research.md.
