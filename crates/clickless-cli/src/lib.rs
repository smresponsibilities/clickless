use clickless_config::Config;
use std::env;

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn enable_grid_runtime(
    sm: &mut clickless_core::StateMachine,
    display: Option<(i32, i32)>,
    cursor: Option<(i32, i32)>,
    grid: clickless_core::grid::GridConfig,
    gui: bool,
) {
    if let (Some((w, h)), Some((x, y))) = (display, cursor) {
        sm.enable_grid_with_monitors(
            vec![clickless_core::grid::Rect::new(0, 0, w as i64, h as i64)],
            (x as i64, y as i64),
            grid,
        );
    } else {
        note(
            gui,
            "Grid mode disabled: display size or pointer position unavailable.",
        );
    }
}

/// Distance in pixels for each leg of the bounded output smoke path. Any
/// cursor movement stays within this offset of the starting position.
const SMOKE_OFFSET_PX: i32 = 40;

/// How the runtime must boot: which config, paused or live, and what to
/// tell the owner. A malformed file boots defaults paused with an
/// explanation; the file itself is never touched.
pub struct StartupPlan {
    pub config: Config,
    pub start_paused: bool,
    pub notice: Option<String>,
}

/// Resolves the boot config without side effects. Missing file means
/// defaults; malformed file means paused defaults plus a notice.
pub fn resolve_startup(config_path: Option<&std::path::Path>) -> StartupPlan {
    let Some(path) = config_path else {
        return StartupPlan {
            config: Config::default(),
            start_paused: false,
            notice: None,
        };
    };
    match Config::load_from_file(path) {
        Ok(config) => StartupPlan {
            config,
            start_paused: false,
            notice: None,
        },
        Err(err) => StartupPlan {
            config: Config::default(),
            start_paused: true,
            notice: Some(format!(
                "Config {} ignored: {err}. Running paused on defaults; the file was left untouched.",
                path.display()
            )),
        },
    }
}

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

/// Shared runtime entry for both the GUI target (`clickless`) and the
/// console diagnostic target (`clicklessctl`). Parses args, runs config
/// checks or the platform hook loop, and returns a plain error string so
/// each binary can choose its own error surface (dialog vs stderr).
pub fn run() -> Result<(), String> {
    run_with_mode(false)
}

/// Console-free entry for the Windows GUI target: runtime-path notes go to
/// the local log instead of stderr. Explicit commands (--help, --version,
/// --check-config, --smoke-output) keep console output on both targets.
#[cfg(windows)]
pub fn run_gui() -> Result<(), String> {
    run_with_mode(true)
}

/// Runtime-path note: log line under the GUI target, stderr otherwise.
fn note(gui: bool, message: &str) {
    #[cfg(windows)]
    if gui {
        clickless_windows::gui_error::log_event(message);
        return;
    }
    let _ = gui;
    eprintln!("{message}");
}

/// Runtime-path announcement: log line under the GUI target, stdout otherwise.
fn announce(gui: bool, message: String) {
    #[cfg(windows)]
    if gui {
        clickless_windows::gui_error::log_event(&message);
        return;
    }
    let _ = gui;
    println!("{message}");
}

fn run_with_mode(gui: bool) -> Result<(), String> {
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

    // The tray runtime is Windows-only today; elsewhere the flag is accepted
    // and ignored so scripts stay portable.
    #[cfg(not(windows))]
    {
        let _ = no_tray;
    }

    let StartupPlan {
        config,
        start_paused,
        notice,
    } = if let Some(path) = config_path {
        let path = std::path::PathBuf::from(path);
        resolve_startup(Some(&path))
    } else {
        // Saved settings take effect without a flag; a missing file means
        // defaults, a malformed one boots paused with a notice.
        match clickless_config::default_config_path() {
            Ok(def) if def.exists() => resolve_startup(Some(&def)),
            _ => resolve_startup(None),
        }
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
        // Validation mode reports the file as-is: a malformed config fails
        // here instead of booting paused defaults.
        if let Some(notice) = notice {
            return Err(notice);
        }
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
        let _single_instance = match lifecycle::SingleInstance::acquire() {
            Ok(instance) => instance,
            Err(reason) => {
                note(
                    gui,
                    &format!("Clickless is already running ({reason}). Opening Settings there."),
                );
                lifecycle::notify_open_settings()?;
                return Ok(());
            }
        };
        let settings_request = SettingsRequest::create()?;

        let output = EnigoAdapter::new().map_err(|e| format!("Output adapter error: {e}"))?;
        let display = output.main_display().ok();
        let cursor = output.cursor_location().ok();
        // Clone once up front: the hook constructor moves fields out of
        // `config`, and the hook still needs the whole config for the editor
        // seed.
        let boot_config = config.clone();
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
        hook.sm_mut().set_hold_ms(config.settings.hold_ms);
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone(), gui);
        match clickless_windows::overlay::WindowsOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(
                overlay.with_theme(config.theme.to_overlay_theme()),
            )),
            Err(reason) => note(gui, &format!("Grid overlay disabled: {reason}")),
        }
        let mut hook = hook.with_boot_config(boot_config);
        if start_paused {
            hook.set_paused(true)?;
        }
        if let Some(notice) = notice {
            note(gui, &notice);
            clickless_windows::settings::seed_notice(notice);
            settings_request.signal()?;
        }

        // Tray: optional so CI and headless runs keep working.
        let tray = if no_tray {
            None
        } else {
            match TrayMenu::new() {
                Ok(tray) => Some(tray),
                Err(reason) => {
                    note(
                        gui,
                        &format!("Tray unavailable, continuing without it: {reason}"),
                    );
                    None
                }
            }
        };

        announce(
            gui,
            "Clickless running on Windows. Hold leader key (default CapsLock) to move pointer."
                .to_string(),
        );

        // Cleanup on any exit path: unhook and overlay hide happen inside
        // run_event_loop; Quit releases app-held output there too.
        // First run opens practice; completion is stored in the config.
        // Read before the move into with_boot_config above borrows it.
        let first_run = hook.current_config().practice_completed_version
            < clickless_windows::practice::PRACTICE_VERSION;
        let practice = if first_run {
            match clickless_windows::practice_dialog::PracticeWindow::new() {
                Ok(window) => {
                    window.show();
                    Some(window)
                }
                Err(reason) => {
                    note(gui, &format!("Practice window unavailable: {reason}"));
                    None
                }
            }
        } else {
            None
        };
        run_event_loop(hook, tray, || true, || settings_request.poll(), practice)?;
        return Ok(());
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
        hook.sm_mut().set_hold_ms(config.settings.hold_ms);
        hook.set_scroll_step(config.settings.scroll_step);
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone(), gui);
        match clickless_linux::overlay::LinuxOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(
                overlay.with_theme(config.theme.to_overlay_theme()),
            )),
            Err(reason) => note(gui, &format!("Grid overlay disabled: {reason}")),
        }

        announce(
            gui,
            "Clickless running on Linux. Hold leader key (default CapsLock) to move pointer."
                .to_string(),
        );
        if start_paused {
            hook.sm_mut().set_paused(true);
        }
        if let Some(notice) = notice {
            note(gui, &notice);
        }
        run_event_loop(hook, || true)?;
        return Ok(());
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
        hook.sm_mut().set_hold_ms(config.settings.hold_ms);
        hook.set_scroll_step(config.settings.scroll_step);
        enable_grid_runtime(hook.sm_mut(), display, cursor, config.grid.clone(), gui);
        match clickless_macos::overlay::MacosOverlay::new() {
            Ok(overlay) => hook.set_overlay(Box::new(overlay)),
            Err(reason) => note(gui, &format!("Grid overlay disabled: {reason}")),
        }

        announce(
            gui,
            "Clickless running on macOS. Hold leader key (default CapsLock) to move pointer."
                .to_string(),
        );
        if start_paused {
            hook.sm_mut().set_paused(true);
        }
        if let Some(notice) = notice {
            note(gui, &notice);
        }
        run_event_loop(hook, || true)?;
        return Ok(());
    }

    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        let _ = config;
        Err("Pointer runtime is not supported on this platform.".to_string())
    }

    // Every real OS arm above ends in `return`, so this line only exists to
    // give the function a tail value on platforms with a runtime. It is
    // unreachable, which the compiler cannot prove; the allow keeps clippy
    // honest without dead cfg gymnastics.
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    #[allow(unreachable_code)]
    {
        unreachable!("the per-OS arm above returned")
    }
}
