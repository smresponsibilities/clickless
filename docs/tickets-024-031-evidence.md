# Tickets 024-031 evidence

Status recorded 2026-09-21. `open` means evidence still requires owner or CI action.

| Ticket | Code | Targeted tests | Workspace gates | Native Windows | Cross-target | Remote CI | Commit |
|---|---|---|---|---|---|---|---|
| 024 config/settings seams | present | settings editor, config persistence | pass | open | pass | open | not committed |
| 025 nested labels | present | nested-label regressions, overlay tests | pass | owner confirmed | pass | open | not committed |
| 026 settings editor | present | editor/help/settings tests | pass | open | pass | open | not committed |
| 027 settings shell | present | settings window tests | pass | open | pass | open | not committed |
| 028 transactional save | present | validate/save and failure tests | pass | open | pass | open | not committed |
| 029 GUI/console split | present | startup, console split, log tests | pass | open | pass | open | not committed |
| 030 practice flow | present | practice tests | pass | open | pass | open | not committed |
| 031 shell rebuild | present | settings shell, placement, help tests | pass | open | pass | open | not committed |

## Commands run

From `clickless/`:

```text
cargo fmt --all --check                                      PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings PASS
cargo test --workspace --all-targets --all-features -- --skip bfs_covers_canonical_states --skip seeded_fuzz_reports_every_failure PASS
cargo test -p clickless-windows reopening_settings_resets_scroll_and_shows_the_window -- --nocapture PASS
```

The bounded workspace run skips only the two known slow explorer tests. Run those separately before release. Native screenshots, Narrator, DPI, high-contrast, foreground behavior, remote CI, and commit remain open.

## Owner release checks

1. Build a fresh release in a separate target directory.
2. Confirm `clickless.exe` has Windows GUI subsystem and `clicklessctl.exe` has console subsystem.
3. Run one GUI instance through tray, Settings reopen/top, grid, nested grid, screenshot chord, Apply/Save/Cancel, failed save, and Quit.
4. Capture screenshots and record binary paths, monitor/DPI, and pass/fail.
