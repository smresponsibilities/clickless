# Contributing to Clickless

Clickless controls global input. A small regression can swallow typing, hide the grid, or leave a mouse button held. Tests and native checks belong to each ticket.

## Before editing

1. Read workspace `AGENTS.md`, `HANDOFF.md`, and the active ticket in `plan-baby-steps.md`.
2. Run `git status --short`; preserve unrelated work.
3. Record ticket, assumptions, red check, and next action in `HANDOFF.md`.
4. Keep exactly one live hook instance. Do not stop an owner process without approval.

## Workflow

1. Add one failing behavior test at a public seam.
2. Record the expected failure.
3. Implement the smallest fix.
4. Run the targeted test and required gates.
5. Perform the matching native check below.
6. Update `HANDOFF.md` with commands, exits, evidence, changed files, blockers, and one next action.

Use `code-complete`, `native-open`, and `CI-open` honestly. Missing evidence means the ticket is not done.

## Required gates

```powershell
cargo fmt --all --check
cargo build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --skip bfs_covers_canonical_states --skip seeded_fuzz_reports_every_failure
```

Run the slow explorer separately when changing input, lifecycle, Settings focus, overlay ordering, config application, or cleanup. Check current test attributes before using `--ignored`:

```powershell
cargo test -p clickless-windows --test app_explorer bfs_covers_canonical_states -- --nocapture
cargo test -p clickless-windows --test app_explorer seeded_fuzz_reports_every_failure -- --nocapture
```

Run cross-target check and Clippy for `x86_64-unknown-linux-gnu` and `x86_64-apple-darwin` when shared/platform code changes. Cross-compilation is not native verification.

## Native check after each ticket

| Change | Required native check |
|---|---|
| Movement/bindings | Leader hold/release, each changed binding, repeats, Esc, missing release |
| Grid state | Slow/fast 3-key selection, repeated letters, rollover, invalid keys, Backspace/Esc at each level, exact target |
| Grid rendering | Light/dark/busy background, smallest display, 100/150/200% DPI, nested-label visibility |
| Suppression | Type in Notepad before/during/after capture; bound keys do not leak; PrintScreen and Win+Shift+S work |
| Drag/output | Drag ends on Esc, leader release, Pause, Settings open, error, and Quit |
| Settings controls | Mouse/keyboard edit, Tab/Shift+Tab, errors, Apply/Save/Cancel, dirty close, reset, search, restart |
| Settings shell | First open centered; reopen raised and scrolled to top; minimum size; placement; no overlap |
| Accessibility | Narrator names/descriptions, visible focus, high contrast, text scaling, keyboard-only flow |
| Config/save | Valid/malformed/missing file, failed write/rename, old bytes preserved, paused recovery |
| Tray/lifecycle | First/second launch, one tray icon, Pause/Resume, Explorer restart, Quit cleanup |
| GUI/console | Explorer GUI launch has no console; `clicklessctl` output and errors remain readable |
| Practice | Fresh profile, Skip/Finish, Esc, focus loss, Practice again, no system click |
| Platform backend | Run on that OS; compilation alone does not close native status |

## Release acceptance

Build fresh release binaries and use exactly one GUI instance:

```powershell
cargo build --release -p clickless-cli
cargo run --release -p clickless-backend-api --example grid_bench -- --check-budget
```

Verify no GUI console, readable `clicklessctl`, single instance, foreground Settings reopen at top, capture-safe typing, nested grid after 2 letters, Apply/Save/Cancel, failed-save preservation, screenshot chords, and complete Quit cleanup. Record binary paths and screenshots.

A running executable may lock its target directory. Prefer a separate `--target-dir`; do not kill an owner process.

## Boundaries

- Core/config stay free of OS APIs.
- Platform code stays in platform crates.
- Hook callbacks only decide suppression and queue work.
- `Config` is the only settings model and source of defaults.
- Apply/Save are transactional.
- Every error/exit path releases capture and held buttons.
- No control ships without a runtime effect.
- No account, telemetry, browser frontend, or service without an explicit ticket.

## Commits and pull requests

- Do not commit or push without owner approval.
- Conventional Commit subject, at most 50 characters.
- One behavior change per commit.
- Report red/green, gates, native evidence, and unverified platforms.
- Never describe compilation as native support.
