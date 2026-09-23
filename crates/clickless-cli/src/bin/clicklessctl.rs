//! Console diagnostic target. Always keeps stdout/stderr so `--help`,
//! `--version`, `--check-config`, `--smoke-output` and `--no-tray` stay
//! readable from a terminal. Shares `clickless_cli::run` with the GUI target.

use std::process;

fn main() {
    if let Err(err) = clickless_cli::run() {
        eprintln!("{err}");
        process::exit(1);
    }
}
