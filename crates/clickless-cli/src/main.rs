use clickless_config::Config;
use std::{env, process};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn enable_grid_runtime(
    sm: &mut clickless_core::StateMachine,
    display: Option<(i32, i32)>,
    cursor: Option<(i32, i32)>,
    grid: clickless_core::grid::GridConfig,
) {
    if let (Some((w, h)), Some((x, y))) = (display, cursor) {
        sm.enable_grid_with_monitors(
            vec![clickless_core::grid::Rect::new(0, 0, w as i64, h as i64)],
            (x as i64, y as i64),
            grid,
        );
    } else {
        eprintln!("Grid mode disabled: display size or pointer position unavailable.");
    }
}

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
        let display = output.main_display().ok();
        let cursor = output.cursor_location().ok();
        let motion = MotionConfig {
            start_speed_px_s: config.settings.start_speed_px_s,
            max_speed_px_s: config.settings.max_speed_px_s,
            ramp_ms: config.settings.ramp_ms,
        };
        let mut hook = WindowsHook::with_config(
            output,
            config.settings.leader,
            config.mouse_bindings,
            motion,
        );
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone());
        match clickless_windows::overlay::WindowsOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(overlay)),
            Err(reason) => eprintln!("Grid overlay disabled: {reason}"),
        }

        println!(
            "Clickless running on Windows. Hold leader key (default CapsLock) to move pointer."
        );
        run_event_loop(hook, || true)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        use clickless_core::MotionConfig;
        use clickless_linux::{LinuxHook, run_event_loop};
        use clickless_output_enigo::EnigoAdapter;

        let output = EnigoAdapter::new().map_err(|e| format!("Output adapter error: {e}"))?;
        let display = output.main_display().ok();
        let cursor = output.cursor_location().ok();
        let motion = MotionConfig {
            start_speed_px_s: config.settings.start_speed_px_s,
            max_speed_px_s: config.settings.max_speed_px_s,
            ramp_ms: config.settings.ramp_ms,
        };
        let mut hook = LinuxHook::with_config(
            output,
            config.settings.leader,
            config.mouse_bindings,
            motion,
        );
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone());
        match clickless_linux::overlay::LinuxOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(overlay)),
            Err(reason) => eprintln!("Grid overlay disabled: {reason}"),
        }

        println!("Clickless running on Linux. Hold leader key (default CapsLock) to move pointer.");
        run_event_loop(hook, || true)?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        use clickless_core::MotionConfig;
        use clickless_macos::{MacosHook, run_event_loop};
        use clickless_output_enigo::EnigoAdapter;

        let output = EnigoAdapter::new().map_err(|e| format!("Output adapter error: {e}"))?;
        let display = output.main_display().ok();
        let cursor = output.cursor_location().ok();
        let motion = MotionConfig {
            start_speed_px_s: config.settings.start_speed_px_s,
            max_speed_px_s: config.settings.max_speed_px_s,
            ramp_ms: config.settings.ramp_ms,
        };
        let mut hook = MacosHook::with_config(
            output,
            config.settings.leader,
            config.mouse_bindings,
            motion,
        );
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone());
        match clickless_macos::overlay::MacosOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(overlay)),
            Err(reason) => eprintln!("Grid overlay disabled: {reason}"),
        }

        println!("Clickless running on macOS. Hold leader key (default CapsLock) to move pointer.");
        run_event_loop(hook, || true)?;
        Ok(())
    }

    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        let _ = config;
        Err("Pointer runtime is not supported on this platform.".to_string())
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}
