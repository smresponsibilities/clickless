# Linux and macOS implementation plan

Date: 2026-10-03. Baseline: 7c908f5. This is an implementation roadmap, not a support claim.

## Current evidence

[CI run 37057097006](https://github.com/smresponsibilities/clickless/actions/runs/37057097006) passed Windows, Ubuntu and macOS build, strict Clippy and tests. Hosted runners do not prove desktop input, permissions, overlays or accessibility. Windows local suite passed 383 tests, with four existing tests ignored. Linux/macOS native user acceptance has not run.

| Area | Linux now | macOS now |
| --- | --- | --- |
| Core/config | Shared state machine, validated TOML, grid rasterizer | Same |
| Input | Single-device evdev/uinput forwarding implemented; native acceptance pending | Raw public event-tap callback and modifier ownership repaired; native acceptance pending |
| Output | Enigo adapter; live session unverified | Enigo adapter; live permission/output unverified |
| Grid | X11 window renderer; no native Wayland renderer | AppKit window renderer requiring main thread |
| UI | No Home, Practice, Settings or tray implementation | No Home, Practice, Settings or menu-bar implementation |
| Lifecycle | Stop predicate, signal handling, UID ownership lock and nonblocking Settings IPC implemented | Same; native Mac ownership proof pending |
| Installation | Cargo workspace builds; no desktop installer | No signed app bundle or notarized installer |

Code anchors: `crates/clickless-linux/src/lib.rs::run_event_loop`, `crates/clickless-macos/src/lib.rs::run_event_loop`, their `overlay.rs` modules, `crates/clickless-cli/src/lib.rs` platform branches, and `src/bin/clickless-settings.rs`. The Settings binary currently does nothing outside Windows. `PARITY.md` contains historical statements; this dated table supersedes them for these platforms.

## Required behavior

- Ordinary typing and Ctrl/Shift/Alt/Super or Command/Option chords pass through exactly once. CapsLock state does not change unexpectedly. Short unused built-in modifier taps follow shared core behavior; delayed held chords remain normal typing.
- Every forwarded or injected press has a release. Pause, cancel, shutdown, device removal, permission loss and display changes clear active movement and held buttons.
- Home, Practice and Settings suspend pointer capture while focused. Practice never moves the actual cursor or clicks.
- Apply changes running configuration only after validation and runtime acknowledgement. Save atomically persists valid configuration. Invalid drafts survive page changes and search. Cancel leaves runtime unchanged.
- One runtime instance owns input. A second launch opens the existing UI. Settings failures leave pointer control recoverable.
- Unsupported environments fail before keyboard grab. Never show invisible grids or silently claim Wayland support.
- Backend crates own OS APIs. Reuse core, config validation, settings descriptors and rasterizer. Keep native platform widgets; WinUI remains Windows-only.

## Delivery order

Complete shared safety contracts, then Linux X11 safety and macOS input safety. Native UI follows working capture and cleanup. Wayland follows X11 acceptance as a separate capability track. Packaging follows native acceptance. Do not hold X11/macOS releases for speculative GNOME/KDE global-input parity.

Each ticket records failing existing coverage when applicable, implementation evidence, gate results and a native checklist. Owner currently waived new regression tests. Run existing tests and manual checks; request a changed testing preference only when new behavior cannot be demonstrated by those checks. Do not invent passing coverage.

## Shared tickets

### P01: runtime lifecycle and UI ownership

Reuse existing state-machine pause/reset behavior and backend contracts. Expose only commands needed by actual platform callers: open UI, pause/resume, apply config, quit. Keep native transport in each backend. Implement graceful signal handling and stop predicates in place of `|| true`. Guard against concurrent instances with OS-local ownership. Define errors for unavailable runtime and failed Apply. Start paused after malformed configuration.

Acceptance: second launch creates no extra hook; quit releases every button and hides overlay; Settings crash does not stop runtime; failed Apply leaves old configuration intact; normal exit and signal exit terminate within two seconds on an idle desktop. Windows remains green.

### P02: display geometry and parity record

Use each backend's existing overlay/output boundaries. Enumerate displays with stable identities, origins, work areas and scale. Define conversion once from shared grid coordinates to native coordinates. Recompute on display changes, clamp selections to surviving displays, and cancel an active drag when its display disappears. Carry existing theme settings to both renderers.

Acceptance: correct targets on a display left of the primary, mixed scale, rotated display, scale change and unplug during grid/drag. Publish a dated feature matrix distinguishing code, CI and real desktop proof.

## Linux tickets

### L01: keyboard passthrough before exclusive grab

2026-10-03 implementation status: IN PROGRESS. Native input loop moved to `clickless-linux/src/input.rs`. Virtual output uses stable name `Clickless virtual keyboard`, BUS_VIRTUAL, vendor/product 0, version 1. Exclude this exact name in remapper configuration where applicable; no vendor ID is claimed. Setup writes a synchronization report before grab and explicitly enables nonblocking reads. Per-key ownership preserves forwarding across mode transitions; idle polling advances holds. Short unused nonmodifier leader taps replay. Output errors trigger pointer/key release and ungrab. Owner requested no tests; final test gate skipped. Existing checks do not establish native acceptance.

Limits: only first accessible compatible keyboard is selected. Unsupported hybrid event capabilities fail before capture. LED feedback and repeat-configuration events are not mirrored. `SYN_DROPPED` fails safely; L02 owns resynchronization. Virtual-device desktop readiness, duplicate-repeat behavior, CapsLock indicators, actual chords, remapper behavior and unplug recovery require native checks below.

Highest priority. Current `Device::grab` suppresses ordinary keyboard events, but the loop never reinjects them. Inspect evdev 0.12 virtual-device support and reuse it. Create a virtual keyboard and validate output before grabbing any physical keyboard. Ignore Clickless-created virtual devices during discovery to prevent a feedback loop. Forward unconsumed raw events, repeats and synchronization boundaries. Preserve non-key events needed by keyboard devices. Decide suppression using shared state transitions and paired-event ownership, not just whether Mouse mode is active after an event.

Verify whether `Device::open` is blocking; current comment claiming nonblocking reads is not proof. Use polling/nonblocking APIs so hold timers and shutdown advance while no key events arrive. Propagate process/tick/output failures instead of discarding them. On failure, release virtual pressed keys/buttons, ungrab physical devices, then exit with a useful diagnostic.

Acceptance: type a paragraph before/after activation and while paused; Ctrl+C/V/A, Ctrl+Shift+arrows, Alt+Tab, Super shortcuts, left/right modifiers and CapsLock still work. Complete twenty activation/release rounds plus delayed holds. Unplug keyboard during activation and verify recovery through a second keyboard. No duplicate characters or stuck keys.

Remapper acceptance: identify Clickless virtual devices with a stable documented identity before capture. Exclude them from Clickless discovery and document exclusions for keyd/kanata where needed. Verify a physical keyboard and remapper together, with events delivered once and no recursive capture. Mouseless's vendor ID 0x736e belongs to its documented Sonuscape identity; do not reuse it or present its keyd exclusion as a Clickless rule. Do not infer exclusive-grab ownership merely from another process having an input device open.

### L02: device selection, hotplug and permissions

Replace first-device selection with explicit discovery/selection using existing config conventions. Support multiple keyboard devices without combining unrelated modifier states incorrectly. Track per-device pressed keys and aggregate logical ownership. Handle hotplug and `SYN_DROPPED` resynchronization. Prefer session-scoped access where supported; document narrow input/uinput permissions if required. Do not recommend running the entire GUI as root or granting world-writable device access.

Acceptance: built-in plus USB/Bluetooth keyboards, device loss/reconnect, inaccessible device, another grab owner, and device enumeration changes. Failed permissions leave keyboard usable and show exact required action. Permission changes require owner action, not silent escalation.

Permission guidance must distinguish physical event-device access from uinput virtual-device creation. Adding a desktop user to an input-access group also gives other processes running as that user access; a dedicated group name does not isolate one application. Explain affected device nodes and revocation before recommending a proved setup. Avoid changing ownership of every event device as the default. Verify hotplug permissions separately from login/reboot behavior. Preserve pre-existing group membership/rules during rollback rather than copying competitor removal commands.

### L03: X11 pointer and overlay acceptance

Retain existing x11rb overlay and Enigo output if live proof passes. Check ARGB visual selection, compositor dependence, click-through input shape, topmost behavior, screen origins and repaint pacing. Distinguish display/session failure from optional theme effects. Disable grid activation if no working overlay can be shown; free movement may remain available when its output works.

Acceptance: real Xorg GNOME/KDE or XFCE session, with and without compositing; dense/simple grid, nested selection, Backspace, Escape, nudge, click, drag, wheel and speed ramp. Grid receives no pointer input and does not take typing focus. Twenty ordinary-user cycles include editor selection and browser scrolling.

### L04: native Home, Settings and Practice

Prototype GTK4 and libadwaita against distribution packaging before adopting them. GTK4 is proposed for native Linux controls; libadwaita appearance must be reviewed on KDE rather than assumed universal. No webview and no recreation of the Windows XAML tree. Render the existing Settings descriptors as navigation pages and native switches, spin controls, shortcut recording and color controls. Reuse parser/editor drafts and Apply/Save semantics. Keep separate Settings process only where runtime messaging and packaging are proved.

Acceptance: every shipped settings descriptor editable; Ctrl+F search; draft retention; inline errors; Reset/Cancel; narrow window; keyboard-only navigation; light/dark/high contrast; AT-SPI names/roles and actual Orca output. Home offers state, practice, settings, pause/resume and quit. Practice uses simulated output.

### L05: desktop integration

Add desktop file/icon and user autostart only after explicit enablement. Integrate StatusNotifierItem where available; do not make tray availability a prerequisite because GNOME may lack an extension. Closing Home leaves clear recovery through second launch. Add service/helper only if restricted input access cannot be solved safely without one; specify authenticated local IPC and minimize privileged operations before implementation.

Acceptance: tray present/absent, session logout/login, pause at startup, user-disabled autostart, malformed config, and clean uninstall without deleting user configuration.

### L06: Wayland capability spike

Treat output injection and overlay placement as separate capabilities. evdev/uinput may provide privileged input/output independently of compositor protocols; a layer-shell overlay alone does not grant capture or injection. Evaluate existing Enigo Wayland support, compositor protocols and portal RemoteDesktop/libei support for intended sessions. Check whether accurate pointer position and per-output geometry are obtainable. No XWayland fallback may claim global native Wayland coverage.

Deliver a matrix for Sway/wlroots, KDE Wayland and GNOME Wayland. For each, record capture permission, injection method, pointer coordinates, overlay stacking, input transparency, scale and consent lifetime. Layer-shell is a candidate on supporting compositors; GNOME support requires a separately verified method. Portal flows may require interactive consent and may not suit unattended always-on capture.

Acceptance: one complete supported compositor path passes L01/L03 user checks; other sessions explicitly report unsupported capabilities before capture. Then write small compositor-specific tickets using proven APIs. Do not promise full Wayland parity from protocol availability alone.

Diagnostics acceptance: separate capture denial/conflict, unavailable output, coordinate mismatch and renderer/compositor delay. Record native session type and compositor. Test supported layer-surface animation policy separately from raster timing. Mouseless's Cairo, DMABUF and webview switches are specific to its renderers; do not add them to Clickless without a corresponding implementation and failing case.

### L07: Linux distribution package

Start with tarball and one documented distro-native package matching the proved toolkit/session baseline. Include desktop resources and permission instructions. Add more formats only from verified installation needs. Flatpak is deferred until input/uinput sandbox constraints have a working consent-based solution.

Acceptance: clean-machine install/run/upgrade/uninstall; no root GUI; settings/runtime binaries found reliably; retained configuration; X11/Wayland support boundaries displayed before activation.

## macOS tickets

### M01: event-tap and modifier correctness

2026-10-03 status: IN PROGRESS. Native loop lives in `clickless-macos/src/input.rs`. core-graphics0.24 returns the original event when its Rust callback returns None, so suppression now uses the public C callback API and returns null for consumed pairs. Box-owned callback state replaces the global stack pointer. Tap RAII disables/invalidate/removes its run-loop source before state destruction, including source-creation errors. Current default run-loop mode dispatches callbacks; common modes remain source registration only.

FlagsChanged uses aggregate flags plus seeded per-key side state. Modifier events retain OS ownership. Raw repeats reuse press ownership. Command/Option/Ctrl chords pass through; engine-owned releases still reach core. CapsLock lock-state notifications pass unchanged and cannot arm a physical hold. Native startup rejects a CapsLock leader before tap creation; configure `ShiftLeft` or `ControlLeft` instead. Generic hook unit scenarios describing CapsLock physical presses remain synthetic core exercises, not native support evidence.

Own-process keyboard events pass by source PID. Current output emits pointer events outside the keyboard tap mask. Tap-disabled notifications, callback panic/reentrancy, pointer/overlay/tick failures end capture and preserve diagnostics. Cleanup attempts drag release, uncertain click release and overlay hide. No automatic tap re-enable or secure-input bypass added.

Build/cross-target checks cannot prove left/right event sequencing, tap retention, modifier ownership, source PID behavior, permissions or desktop output. Native acceptance below and M02 permission preflight remain open. Tests skipped at owner's request; no passing native checks claimed.

Earlier `FlagsChanged` logic treated CapsLock transitions as presses and other modifier changes as releases. Current code uses flags and previous per-key state; native sequencing remains unverified. CapsLock hold is rejected. ShiftLeft and ControlLeft require native hold/release acceptance.

Preserve key pairs for ordinary Command/Ctrl/Option/Shift chords. Track events generated by Clickless to avoid feedback. Handle tap-disabled timeout/user-input notifications, permission revocation and secure-input restrictions. Recover a tap only when authorized and valid; otherwise pause and notify. Clear the global hook pointer on every construction/error/teardown path, including tap and run-loop-source failures. Never leave a pointer to stack state after return. Propagate output errors.

Acceptance: Command+C/V/A, Command+Shift selection, Option navigation, Ctrl shortcuts, input sources and CapsLock toggle still work. Twenty activation/release cycles on built-in and external keyboards. Timeout/permission failure ends interception safely. Ordinary applications receive no orphan modifier events.

### M02: permissions and main-thread lifecycle

Use public Accessibility/Input Monitoring preflight APIs appropriate to actual capture and injection behavior. Present separate reasons for required permissions. User grants permissions in System Settings; denied/revoked access leaves capture off. Reuse AppKit main-thread ownership for windows and run-loop coordination. Enforce one runtime instance. Secure input must produce truthful status without attempts to bypass it.

Acceptance: fresh profile, deny, grant, revoke, restart, locked screen and secure-input application. No permission loop, no silent capture, no callback accessing destroyed state. Quit releases buttons and removes tap.

Recovery guidance must stop capture/remove the event tap and release owned output before telling users to remove/re-add Accessibility permission. Do not instruct permission removal while capture remains active. Distinguish a denied permission, disabled tap and Secure Input only when native evidence supports that distinction. Sensitive-field input restrictions are expected; offer leaving the field, not bypassing protection. Test a remapper hiding a chord's main key while forwarding modifier press/release. Lowering a tap threshold may reduce false activation but cannot recover hidden events. Event-tap placement/launch-order workarounds require native compatibility proof before becoming settings or recommended fixes.

### M03: native display and output proof

Reuse Enigo if it correctly handles macOS coordinate conversion and permissions. Existing AppKit overlay uses main-thread checks; retain them. Verify CGDisplay/AppKit coordinate origins, logical points versus backing pixels, Retina scale, multiple displays, full-screen Spaces and overlay window collection behavior. Apply shared theme values rather than renderer defaults.

Acceptance: dense/simple grid, nested selection, nudge, release click, drag, scroll and cancellation; Retina plus external non-Retina display; negative display origins; disconnect during drag; Mission Control/Spaces and full-screen app. Overlay remains click-through and does not activate the app unexpectedly.

### M04: native Home, Settings and safe Practice

Use existing objc2/AppKit dependencies before adding a UI toolkit. Prototype NSWindow, native controls, sidebar and toolbar search against shared Settings descriptors. Keep all view mutations on main thread; run capture/output separately without blocking UI. Use local authenticated runtime messaging if Settings is a separate process. Preserve validation, dirty drafts and atomic saves. Add menu-bar state and pause/resume/open/quit using native menus.

Acceptance: same functional checklist as Windows settings, Command+F search and normal Command shortcuts; shortcuts capture restores focus; theme/system appearance; resizing; VoiceOver reading order/names/errors; Increase Contrast and reduced motion. Practice completes without real pointer output. Closing/reopening never creates another tap.

### M05: app identity and distribution

Build a stable `.app` bundle with icons, Info.plist and executable identity so permission grants survive routine updates. Produce Apple Silicon and Intel artifacts only when both architectures compile/test; universal bundle is optional if both thin bundles meet install needs. Signing/notarization needs owner's Apple Developer identity and credentials. Do not invent certificates or claim notarization from unsigned CI artifacts. Login-item opt-in follows bundle identity and supported OS APIs.

Acceptance: clean Mac install, permission grants, update without unexpected permission reset, launch from Finder, menu-bar recovery, disabled login item, quarantine/Gatekeeper behavior, uninstall preserving user config. Signed releases pass codesign and notarization/stapling checks when credentials exist.

## Verification and release gates

For each code ticket: `cargo fmt --all --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-targets --all-features`. Retain Windows no-default-feature Clippy. Native targets must run on their own OS; Windows cross-checks cannot substitute for AppKit/X11 linkage or desktop behavior.

CI keeps Windows, Linux and macOS gates. Add architecture builds once an actual release target is selected. Log bounded explorer budgets explicitly; full-budget runs remain distinct. Native input/permission tests require a dedicated desktop session and must not inject into an unrelated user's session.

Run twenty native functionality rounds per supported desktop combination. Include activation hold/release, escape/cancel, plain text, ordinary modifier chords, app switching, pause/resume, grid click, drag release and scrolling. Also run separate failure checks for unplug, permission loss, backend error and shutdown. Record expected and observed behavior; a round is not passed by process survival alone.

Release requires all mandatory desktop checks, no stuck modifiers/buttons, usable permission recovery, supported platform matrix, fresh-machine install proof and documented unsupported environments. Linux X11 and macOS can release independently. Wayland support is advertised per compositor only after its native checks pass. Keep Windows' current physical-held-key and Light-caption contrast gaps recorded separately.

## Dependencies and estimates

Critical paths: P01 -> L01 -> L02/L03 -> L04/L05 -> L07; P01 -> M01 -> M02/M03 -> M04 -> M05. P02 supports both display tickets. L06 follows safe Linux input and has a discovery gate before implementation commitments.

Planning ranges for one developer with target machines available: shared safety/geometry 3-5 days; Linux X11 input/output 5-8 days; Linux UI/integration/package 6-10 days; macOS input/permissions/display 5-9 days; macOS UI/bundle 6-10 days. Wayland spike 2-4 days, implementation estimate only after compositor capability results. Signing account setup and hardware access are external dependencies. Re-estimate after L01 and M01.

## Platform references to verify during implementation

- [Linux uinput documentation](https://www.kernel.org/doc/html/latest/input/uinput.html), [evdev crate](https://docs.rs/evdev/0.12.2/evdev/).
- [Wayland protocols](https://wayland.app/protocols/), [xdg-desktop-portal RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html).
- [GTK4](https://docs.gtk.org/gtk4/), [libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/).
- [Apple CGEvent taps](https://developer.apple.com/documentation/coregraphics/cgevent), [AppKit](https://developer.apple.com/documentation/appkit), [notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

These are implementation references, not evidence that a specific compositor or OS version already works. Verify API/version availability in each ticket.
