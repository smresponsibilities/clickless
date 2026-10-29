# Current candidate and your next steps

Project remains incomplete. Published registry version is 0.1.1. Working source is candidate 0.1.3, with uncommitted UI and platform repairs.

## Windows walkthrough

1. Open PowerShell in `D:\opensource\17-mouseless-rs\clickless`.
2. Run `cargo build -p clickless --bins --locked`. Keep all three executables together in `target\debug`.
3. Run `.\target\debug\clickless-settings.exe` to inspect Settings without starting pointer capture. Check all eight sidebar icons, left-aligned labels, and window/taskbar icon.
4. Check Tab navigation, light/dark themes, and 100/150/200 percent scaling. Confirm invalid values show errors and Save preserves valid changes after reopening.
5. Close standalone Settings. Run `.\target\debug\clickless.exe`. Home should open. Read activation key from your saved settings; existing configurations can differ from defaults.
6. Complete Practice. Open Settings through Home or tray. Apply one reversible change, verify acknowledgement, then Save. Close and reopen Settings.
7. Test grid selection and Esc cancellation. Test normal typing and Ctrl shortcuts after leaving capture. Quit through tray and confirm keyboard/pointer control returns normally.
8. Record failed action, expected result, actual result, theme, display scale, and executable build used. These observations close desktop checks that compilation cannot prove.

## Remaining engineering work

- Complete pending Linux setup, permission, fairness and runtime lifecycle regressions.
- Complete pending macOS permission, display/output and companion/menu lifecycle regressions.
- Review coordinated source, commit it without scratch/object artifacts, and run current-revision Windows/Linux/macOS ARM/Intel CI. Preserve full explorer coverage separately from bounded local checks.
- Run Linux X11 and macOS desktop acceptance on suitable native sessions. WSL systemd user-session failure currently prevents local tray proof.
- Build verified unsigned Mac bundle before handing it to signing owner. Credentialed signing/notarization remains external. Signing alone does not close desktop acceptance.
- Keep Wayland discovery optional; current Wayland support remains unavailable.

Detailed acceptance lives in the workspace's `.scratch/platform-pending-2026-10-04/spec.md`. Current source must pass native CI before release readiness is claimed. No new publication occurred during this check.
