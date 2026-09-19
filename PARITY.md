# Parity ledger

Last corrected: 2026-09-19. Earlier CI labels overstated evidence for current code.

| Component | Current implementation | Runtime proof |
|---|---|---|
| Core layers and movement | Pure Rust, unit tests | No end-to-end session |
| TOML configuration | Parser and validation | Not applied to live backend |
| Windows input | Key translation and mock output dispatch | No WH_KEYBOARD_LL registration/message loop |
| Linux input | Key translation and mock output dispatch | No evdev grab/reinjection loop |
| macOS input | Key translation and mock output dispatch | No CGEventTap installation or permission flow |
| Enigo output | Adapter implementation | No recorded human smoke test |
| Grid/nudge | Isolated core model | No renderer or runtime wiring |
| Deep links | String parser | No scheme registration or command execution |
| CLI | Config validation, help, version | Pointer runtime unavailable |

## Verification boundaries

Local checks run on Windows. Current Linux and macOS CI results are unverified. The historical scaffold CI run does not prove this source tree builds on those platforms.

All platform flow smoke checks remain open. Missing hook implementations are engineering work, not tests an owner can perform on the existing binary.

Known runtime work includes applying parsed config, balanced button down/up, output error propagation, event suppression/passthrough, and cleanup. Grid models remain experimental and do not establish competitor parity.

Recovery checks on Windows, 2026-09-19: fmt, build, full-workspace/all-feature Clippy and tests passed. 127 tests passed. No platform crate excluded. Native Linux/macOS dependencies are target-scoped; their native builds remain unverified.
