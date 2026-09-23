# Next 3 prompts

Run one per session. Read `CONTRIBUTING.md`, workspace `AGENTS.md`, current `HANDOFF.md`, and `git status --short`. Preserve dirty work. No commit/push without approval.

## 1. Settings UX/UI acceptance

```text
Audit Ticket 031 against its spec and current native Win32 Settings. Keep the selected Win32 foundation and existing shell. Verify/fix only reproduced gaps.

Opening from tray or second launch must restore and raise the existing window, reset scroll to top, retain a valid selected page, and focus the first useful control. Verify sidebar, search, sticky footer, live dirty state, Save/Discard/Cancel close, Reset page, placement, and Segoe UI Variable fallback at 100/150/200% DPI. Every shipped setting needs persistent short help plus the same detail on hover and keyboard focus. Internal tokens never appear. Errors sit beside fields and focus the first invalid field. Apply/Save never claim success after runtime/disk failure.

Red first. Run Settings tests, workspace gates, then native mouse, keyboard, Narrator, high-contrast, foreground/reopen, and screenshot checks. Record PASS/FAIL. Do not claim theme work assigned to a later ticket.
```

## 2. Grid and full input-flow audit

```text
Run the complete matrix in the workspace docs/settings-grid-ux-audit.md and app-rebuild Prompts 18/19. Cover valid/invalid keys, repeats, lost releases, rollover, Backspace/Esc at every level, leader release, Pause, Settings open, Apply during grid/drag, overlay failures, screenshot chords, monitor changes, and DPI changes.

After every event prove: no held-button leak, at most one click, inactive navigator never remains in Grid, subgrid presentation precedes final selection, printable mistakes follow the documented suppression policy, screenshot chords preserve selection, and every error/quit hides overlay and releases capture.

Run bounded BFS, seeded fuzz, nested-label regressions, release benchmark, gates, and a one-instance native Notepad/screenshot/grid test. Turn every minimal failure into a named regression before fixing it.
```

## 3. Documentation and release-proof audit

```text
Review README.md, CONTRIBUTING.md, PARITY.md, config coverage, architecture, plan, and HANDOFF against the dirty code. Correct stale claims only with evidence. Verify the clickless.exe/clicklessctl.exe PE subsystem split in a fresh release and run every README command exactly as written.

Create a Tickets 024-031 evidence table: code, targeted tests, full gates, native Windows, cross-target compile, remote CI, commit. Keep missing evidence open. Run CONTRIBUTING checks relevant to each touched area and record exact commands, exits, binaries, screenshots, and owner actions.

Finish with separate standards and spec reviews of the dirty diff. Do not commit, push, stop an owner process, or claim Linux/macOS native support.
```
