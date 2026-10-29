# Shared desktop UI candidate

Source candidate 0.1.3 selects GPUI Settings, Home and Practice by default. Tusk gpui-kit stays pinned to 0.6.6. Native visual, accessibility and mixed-DPI acceptance has not passed; this candidate is not a validated release.

Build all companion binaries together:

```text
cargo build --release -p clickless --bins
```

Keep clickless-settings beside clickless and clicklessctl. Normal clickless launch opens Home. clicklessctl retains console diagnostics; --no-tray omits the tray and startup Home. Closing Home leaves runtime running. Quit Clickless requests runtime cleanup. Tray opens the existing window or starts a new one after closure.

Home shows activation and enabled state. First-run Practice offers platform-supported activation keys. Returning Practice uses the saved activation key. Both preserve configured target keys through the same pure state machine on a simulated display. No output adapter exists in practice. Focus loss cancels the lesson. Finish saves its version only after completion. Unsaved Settings drafts block practice until Save or Discard.

Settings keeps invalid raw drafts across navigation and search. Apply requires runtime acknowledgement. Save persists atomically after acknowledgement; standalone Save applies at next launch. Runtime state updates preserve edited drafts and replace clean drafts. Window close and Quit protect pending operations, edited drafts and applied changes that have not been saved. Discard acknowledges session-only changes; Save persists them. Reopen selects its requested page and resets scroll. Parent death closes the connected UI. Apply acknowledgement has a five-second limit; failed acknowledgement retains the draft.

## Platform integration

Linux uses StatusNotifierItem through tray-icon's ksni backend, without a GTK loop. It requires a desktop D-Bus session and a tray host. Input remains X11-only; Wayland is rejected before exclusive capture. Install desktop files with scripts/install-linux.sh. Pass --enable-autostart or --disable-autostart to control this user's XDG autostart entry. Startup is disabled until explicitly enabled.

macOS uses native status menus on the runtime main thread. scripts/bundle-macos.sh bundles all three executables and sets the deployment floor to macOS 11. Use scripts/startup-macos.sh --enable /absolute/path/to/Clickless to install a user LaunchAgent; --disable removes it. Accessibility and Input Monitoring permissions remain manual. New defaults use ControlLeft; existing files are preserved. CapsLock remains rejected for capture and live Apply.

Windows retains the existing hook, tray and inherited-pipe process boundary. Set CLICKLESS_SETTINGS_UI=native before launch to select the retained native host. Builds with --no-default-features omit GPUI. No config format conversion or deletion occurs when switching hosts.

## Acceptance still required

Run native visual and keyboard checks on all three desktops. Check Settings, Home and Practice at minimum size, light/dark themes and 100/150/200 percent scaling. Complete practice using saved modifier keys. Check tab order, accessible names, screen-reader announcements, high contrast, close/reopen, tray state, startup disable, parent death, permission revocation and held-button cleanup. Cross-compilation cannot prove these behaviors.
