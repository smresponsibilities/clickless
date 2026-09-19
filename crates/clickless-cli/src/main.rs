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

/// Distance in pixels for each leg of the bounded output smoke path. Any
/// cursor movement stays within this offset of the starting position.
const SMOKE_OFFSET_PX: i32 = 40;

/// Bounded, explicitly invoked output smoke path. It moves the real pointer,
/// clicks the left button once and scrolls one notch down then back up, then
/// prints what it observed. It is never called by automated tests.
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn smoke_output() -> Result<(), String> {
    use clickless_backend_api::{Button, Dir, OutputBackend};
    use clickless_output_enigo::EnigoAdapter;

    let mut output = EnigoAdapter::new().map_err(|e| format!("Output adapter error: {e}"))?;
    let (x0, y0) = output.cursor_location()?;
    println!("smoke: start cursor ({x0}, {y0})");

    output.move_rel(SMOKE_OFFSET_PX, SMOKE_OFFSET_PX)?;
    let (x1, y1) = output.cursor_location()?;
    println!("smoke: relative move by ({SMOKE_OFFSET_PX}, {SMOKE_OFFSET_PX}) -> ({x1}, {y1})");

    output.move_abs(x0, y0)?;
    let (x2, y2) = output.cursor_location()?;
    println!("smoke: absolute restore -> ({x2}, {y2}), expected ({x0}, {y0})");

    output.button(Button::Left, Dir::Down)?;
    output.button(Button::Left, Dir::Up)?;
    println!("smoke: left button down then up released");

    output.scroll(0, 1)?;
    output.scroll(0, -1)?;
    println!("smoke: scrolled down one notch then back up");

    println!(
        "smoke: done. Movement stays within {SMOKE_OFFSET_PX}px of the start and the pointer is back at ({x2}, {y2})."
    );
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut config_path = None;
    let mut check_config = false;
    let mut smoke = false;
    let mut no_tray = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "clickless - keyboard-driven pointer control\n\nUsage: clickless [OPTIONS]\n\n    --check-config        Validate TOML configuration only\n    --smoke-output        Move the cursor once, click left, scroll, then restore\n    --no-tray             Run without the tray icon and settings window\n    -c, --config FILE     Configuration file, otherwise defaults\n    -h, --help            Print help\n    -V, --version         Print version"
                );
                return Ok(());
            }
            "--version" | "-V" => {
                println!("clickless {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--check-config" => check_config = true,
            "--smoke-output" => smoke = true,
            "--no-tray" => no_tray = true,
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

    if smoke {
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            return smoke_output();
        }
        #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
        {
            return Err("Output smoke path is not supported on this platform.".to_string());
        }
    }

    if check_config {
        println!("Configuration valid. Pointer runtime is not started.");
        return Ok(());
    }

    #[cfg(windows)]
    {
        use clickless_core::MotionConfig;
        use clickless_output_enigo::EnigoAdapter;
        use clickless_windows::lifecycle::SettingsRequest;
        use clickless_windows::tray::TrayMenu;
        use clickless_windows::{WindowsHook, lifecycle, run_event_loop};

        // Single instance: a second launch contacts the first instead of
        // registering another hook.
        if let Err(reason) = lifecycle::SingleInstance::acquire() {
            eprintln!("Clickless is already running ({reason}). Opening Settings there.");
            lifecycle::notify_open_settings()?;
            return Ok(());
        }
        let settings_request = SettingsRequest::create()?;

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

        // Tray: optional so CI and headless runs keep working.
        let tray = if no_tray {
            None
        } else {
            match TrayMenu::new() {
                Ok(tray) => Some(tray),
                Err(reason) => {
                    eprintln!("Tray unavailable, continuing without it: {reason}");
                    None
                }
            }
        };

        println!(
            "Clickless running on Windows. Hold leader key (default CapsLock) to move pointer."
        );

        // Cleanup on any exit path: unhook and overlay hide happen inside
        // run_event_loop; Quit releases app-held output there too.
        run_event_loop(hook, tray, || true, || settings_request.poll())?;
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
