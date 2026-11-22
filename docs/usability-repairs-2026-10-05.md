# Windows usability repairs

User reported delayed startup, missed CapsLock presses in Practice, pointer control unavailable from Home, slow Save, and an opaque dark grid.

## Confirmed causes and repairs

- Grid help used the entire free screen area as an opaque panel. Help now paints a compact centered card. Desktop remains visible around it. Renderer regression failed with alpha255 before repair and passed afterward.
- GPUI Windows reports CapsLock as a lock-state change, without key press/release events. Practice needs physical edges to measure a hold. Windows backend now supplies physical CapsLock state; shared UI samples it every16ms. Pure Practice regression verifies selection followed by one hold advances without repeated key events.
- Home deliberately suspends global capture while focused. Added Use pointer control button to minimize Home and restore runtime ownership. Home explains this focus rule.
- Shared UI starts in Light mode. Dark and Auto remain available.

## Verification scope

Final three-executable all-feature build, strict workspace all-target/all-feature Clippy, format and diff checks passed. Console reports clickless0.1.3. Core/renderer/config suites passed178 tests. Output is retained in `.scratch/usability-final-gates.log` and `.scratch/usability-config-tests.log`. Full workspace tests and native visual/timing acceptance remain open.

Startup eventually displays UI according to owner. Startup and Save latency remain unresolved until timed on final executable. Host free memory fell below200MB during build work; superseded full-suite compiler/linker processes were stopped. Memory pressure is observed, not proof of every reported delay.

Hidden-window startup probe found no visible main-window handle within30s. This is inconclusive for first paint because hidden windows need not expose that handle. Save/file-preservation regression suite passed18 tests in0.04s; this measures isolated file operations, not UI/runtime acknowledgement.

## Retest

1. Quit existing Clickless through tray.
2. Start fresh `target\debug\clickless.exe` after successful final build.
3. Confirm Light appearance and readable icons.
4. Start Practice. Choose CapsLock, release it, then hold once for at least200ms and release. Grid selection should advance.
5. Finish Practice and return Home. Press Use pointer control before trying activation or free movement.
6. Open grid and select first label. Confirm desktop remains visible around compact help. Esc cancels.
7. Compare Save in standalone Settings against Settings opened from running Clickless. Record elapsed seconds and whether confirmation appears.

For temporary native-host comparison, set `CLICKLESS_SETTINGS_UI=native` before launching from PowerShell. Remove that environment variable to return to GPUI. This preserves configuration and does not prove GPUI latency fixed.
