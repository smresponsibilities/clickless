#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if let Err(error) = clickless_windows::settings_process::run() {
        clickless_windows::gui_error::gui_error(&format!("Settings failed: {error}"));
    }
}
