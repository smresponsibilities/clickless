# Windows GPUI Settings

Build the separate Settings executable with the GPUI host:

```powershell
cargo build -p clickless --features gpui-settings --bin clickless-settings
.\target\debug\clickless-settings.exe --gpui
```

Standalone editing loads the selected configuration file. Apply is unavailable; Save writes configuration for the next launch. Use `--config PATH` to edit another file.

To use GPUI Settings from the running application, build both companions and select the host before starting Clickless:

```powershell
cargo build -p clickless --features gpui-settings
$env:CLICKLESS_SETTINGS_UI = 'gpui'
.\target\debug\clickless.exe
```

Home and the tray open the same Settings process. Existing inherited pipes carry configuration and runtime acknowledgements. The child PID remains the focus boundary that suspends pointer capture while editing. GPUI performs blocking Apply and atomic Save on a background worker.

Remove `CLICKLESS_SETTINGS_UI` to use the existing Windows host. GPUI is opt-in until native visual/accessibility acceptance passes. The sample executable now launches the same functional editor rather than a disconnected state sheet.

Record key captures a supported single key into the activation field. Record click shortcut appends an editable `key = click_left` entry. Esc cancels recording. Enter left/right modifier names directly, such as `controlleft`; GPUI key events do not identify every physical modifier side. Validation rejects reserved or duplicate bindings before Apply.

GPUI supplies the Windows common-controls and PerMonitorV2 manifest for builds with this feature. The backend keeps its icon resource and uses its existing manifest in builds without GPUI.

Check all six pages, multiline shortcuts, invalid numbers, search/navigation draft retention, reset/discard, runtime rejection, and Save failure/retry. Verify Tab/Shift+Tab, large text, Narrator, mixed DPI, system appearance changes, close/reopen, and shutdown while Apply is pending. Compilation does not establish these desktop results.
