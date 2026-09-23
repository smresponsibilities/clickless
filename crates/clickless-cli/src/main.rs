#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Normal Windows launch: no console window. Failures surface through a
//! concise dialog plus a local log line; see `clickless-windows::gui_error`.

use std::process;

fn main() {
    #[cfg(windows)]
    let result = clickless_cli::run_gui();
    #[cfg(not(windows))]
    let result = clickless_cli::run();
    if let Err(err) = result {
        #[cfg(windows)]
        {
            clickless_windows::gui_error::gui_error(&err);
        }
        #[cfg(not(windows))]
        {
            eprintln!("{err}");
        }
        process::exit(1);
    }
}
