# Windows shared UI

GPUI Settings, Home and Practice are selected in source candidate 0.1.3. Build companion binaries together:

```powershell
cargo build --release -p clickless --bins
.\target\release\clickless.exe
```

Run clickless-settings.exe alone for atomic Save without live Apply. Pass --home to open its Home page. Keep it beside clickless.exe.

Existing inherited pipes carry configuration and acknowledgements. Runtime owns input capture and uses the child PID as its focus boundary. Blocking Apply and Save run on a background worker. Native Windows tray remains in the runtime process.

For rollback, set CLICKLESS_SETTINGS_UI=native before starting Clickless. Builds with --no-default-features omit GPUI. WinUI remains available through --features winui3.

Record key accepts supported single keys. Esc cancels. Enter left/right modifier names directly, such as controlleft. Practice handles modifier state events and uses saved target keys on a simulated display.

GPUI supplies the PerMonitorV2/common-controls manifest. Native visual, Narrator, high-contrast and mixed-DPI acceptance remains unverified. See [shared desktop candidate](shared-desktop-ui.md).
