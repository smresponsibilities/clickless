#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if let Err(error) = clickless_windows::settings_process::run() {
        clickless_windows::gui_error::gui_error(&format!("Settings failed: {error}"));
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("Settings UI is unavailable on this platform. Edit your config file manually.");
        std::process::exit(1);
    }
}
