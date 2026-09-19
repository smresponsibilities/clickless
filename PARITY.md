# Parity ledger

Last updated: 2026-09-19.

| Component | Current implementation | Runtime proof |
|---|---|---|
| Core layers and movement | Pure Rust, configurable leader/bindings/motion, unit tests | Unit tests passing |
| TOML configuration | Parser and validation with error reporting | Applied to runtime and verified in unit tests |
| Windows input | Low-level hook (`WH_KEYBOARD_LL`), message loop, configurable bindings, key suppression | Hook loop implemented, unit tested, available in CLI |
| Linux input | Key translation, configurable bindings, mock dispatch | Native evdev loop scaffolded |
| macOS input | Key translation, configurable bindings, mock dispatch | Native CGEventTap loop scaffolded |
| Enigo output | Adapter implementation, balanced clicks, error propagation | Integration tested |
| Grid/nudge | Pure core model | Isolated prototype |
| Deep links | String parser | Validated |
| CLI | Config validation, help, version, runtime loop on Windows | Validated and functional |

## Verification boundaries

- Windows gates: fmt, build, full-workspace/all-feature Clippy and 134 unit/integration tests passing.
- Windows native hook installed and unregistered gracefully.
- Linux and macOS crates compile cleanly under workspace tests with full scancode translation and hook state machine integration.

