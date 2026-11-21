#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        #[cfg(feature = "gpui-settings")]
        if clickless_platform::windows::settings_process::use_gpui()
            || std::env::args().any(|arg| arg == "--gpui")
        {
            if let Err(error) = clickless_cli::ui_process::run() {
                clickless_platform::windows::gui_error::gui_error(&format!(
                    "Settings failed: {error}"
                ));
                std::process::exit(1);
            }
            return;
        }
        if let Err(error) = clickless_platform::windows::settings_process::run() {
            clickless_platform::windows::gui_error::gui_error(&format!("Settings failed: {error}"));
            std::process::exit(1);
        }
    }
    #[cfg(all(
        feature = "gpui-settings",
        any(target_os = "linux", target_os = "macos")
    ))]
    if let Err(error) = clickless_cli::ui_process::run() {
        eprintln!("Settings failed: {error}");
        std::process::exit(1);
    }
    #[cfg(all(
        not(windows),
        not(all(
            feature = "gpui-settings",
            any(target_os = "linux", target_os = "macos")
        ))
    ))]
    {
        eprintln!("Settings UI is unavailable on this platform. Edit your config file manually.");
        std::process::exit(1);
    }
}
