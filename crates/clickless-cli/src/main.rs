use clickless_config::Config;
use std::{env, process};

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut config_path = None;
    let mut check_config = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "clickless - pointer control prototype\n\nUsage: clickless --check-config [--config FILE]\n\n    --check-config        Validate TOML configuration only\n    -c, --config FILE     Configuration file, otherwise defaults\n    -h, --help            Print help\n    -V, --version         Print version\n\nPointer runtime, overlays, and URL handlers are not implemented."
                );
                return Ok(());
            }
            "--version" | "-V" => {
                println!("clickless {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--check-config" => check_config = true,
            "--config" | "-c" => {
                config_path = Some(args.next().ok_or("--config requires a file path")?);
            }
            _ => return Err(format!("Unsupported argument: {arg}")),
        }
    }
    if !check_config {
        return Err(
            "Pointer runtime is not implemented. Use --check-config to validate configuration."
                .into(),
        );
    }
    if let Some(path) = config_path {
        Config::load_from_file(path).map_err(|err| err.to_string())?;
    } else {
        Config::default();
    }
    println!("Configuration valid. Pointer runtime is not started.");
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}
