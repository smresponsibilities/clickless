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
                    "clickless - keyboard-driven pointer control\n\nUsage: clickless [OPTIONS]\n\n    --check-config        Validate TOML configuration only\n    -c, --config FILE     Configuration file, otherwise defaults\n    -h, --help            Print help\n    -V, --version         Print version"
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

    let config = if let Some(path) = config_path {
        Config::load_from_file(path).map_err(|err| err.to_string())?
    } else {
        Config::default()
    };

    if check_config {
        println!("Configuration valid. Pointer runtime is not started.");
        return Ok(());
    }

    #[cfg(windows)]
    {
        use clickless_core::MotionConfig;
        use clickless_output_enigo::EnigoAdapter;
        use clickless_windows::{WindowsHook, run_event_loop};

        let output = EnigoAdapter::new().map_err(|e| format!("Output adapter error: {e}"))?;
        let motion = MotionConfig {
            start_speed_px_s: config.settings.start_speed_px_s,
            max_speed_px_s: config.settings.max_speed_px_s,
            ramp_ms: config.settings.ramp_ms,
        };
        let hook = WindowsHook::with_config(
            output,
            config.settings.leader,
            config.mouse_bindings,
            motion,
        );

        println!(
            "Clickless running on Windows. Hold leader key (default CapsLock) to move pointer."
        );
        run_event_loop(hook, || true)?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = config;
        Err("Pointer runtime is only implemented for Windows currently.".to_string())
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}
