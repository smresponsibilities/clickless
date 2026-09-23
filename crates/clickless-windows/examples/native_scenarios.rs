//! App-rebuild 19: native Windows scenario runner.
//!
//! What the pure model cannot prove. Safe checks run by default; anything
//! that touches the real pointer, hook, or tray needs `--live` and owner
//! eyeballs. Every line prints `PASS:`, `FAIL:` or `NEEDS-OWNER:`.
//! A guard restores overlay state on interruption.

use std::process::ExitCode;

#[cfg(windows)]
fn report(pass: bool, name: &str) -> bool {
    if pass {
        println!("PASS: {name}");
    } else {
        println!("FAIL: {name}");
    }
    pass
}

/// PE subsystem of a binary: 2 = GUI, 3 = console.
fn pe_subsystem(path: &std::path::Path) -> Option<u16> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return None;
    }
    let e = u32::from_le_bytes(bytes[0x3C..0x40].try_into().ok()?) as usize;
    let o = e.checked_add(24 + 68)?;
    let arr: [u8; 2] = bytes.get(o..o + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(arr))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let live = args.iter().any(|a| a == "--live");

    #[cfg(windows)]
    let ok = windows_checks(live);
    #[cfg(not(windows))]
    let ok = {
        let _ = live;
        println!("NEEDS-OWNER: Windows-only scenarios need a Windows run");
        true
    };

    match std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(pe_subsystem)
    {
        Some(2) => {
            println!("PASS: runner binary is GUI subsystem (2)");
        }
        other => {
            println!(
                "NEEDS-OWNER: runner subsystem is {other:?}, want GUI (2) for the real target"
            );
        }
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(windows)]
fn windows_checks(live: bool) -> bool {
    use clickless_backend_api::OverlayBackend;
    use clickless_config::Config;
    use clickless_windows::lifecycle::{SettingsRequest, SingleInstance, notify_open_settings};

    let mut ok = true;

    // Single instance: exactly one owner at a time.
    match SingleInstance::acquire() {
        Ok(guard) => {
            ok &= report(SingleInstance::acquire().is_err(), "second acquire fails");
            drop(guard);
            ok &= report(SingleInstance::acquire().is_ok(), "reacquire after drop");
            // Release immediately so a live owner session is unaffected.
            drop(SingleInstance::acquire().unwrap());
        }
        Err(reason) => {
            println!("NEEDS-OWNER: live Clickless holds the mutex ({reason}); stop it and rerun");
        }
    }

    // Settings-request event round trip.
    match SettingsRequest::create() {
        Ok(request) => {
            let quiet = !request.poll();
            let signaled = notify_open_settings().is_ok() && request.poll();
            ok &= report(quiet && signaled, "settings event round trip");
        }
        Err(err) => {
            ok &= report(false, &format!("settings event create ({err})"));
        }
    }

    // Config round trip through the real parser/validator. Byte order is
    // intentionally not asserted: HashMap serialization order varies.
    let config = Config::default();
    ok &= report(config.validate().is_ok(), "default config validates");
    ok &= report(
        Config::parse(&config.to_toml()).is_ok(),
        "config serializes to parseable TOML",
    );

    if !live {
        println!("NEEDS-OWNER: rerun with --live for overlay flash, focus, tray and chord checks");
        return ok;
    }

    // Live: present one real frame on the real overlay, then hide it.
    // The guard hides again on early return or panic.
    struct HideGuard {
        overlay: Option<clickless_windows::overlay::WindowsOverlay>,
    }
    impl Drop for HideGuard {
        fn drop(&mut self) {
            if let Some(overlay) = self.overlay.as_mut() {
                let _ = overlay.hide();
            }
        }
    }
    match clickless_windows::overlay::WindowsOverlay::new() {
        Ok(overlay) => {
            use clickless_core::grid::{OverlayCell, OverlayFrame, Rect};
            let frame = OverlayFrame {
                level: 1,
                cells: vec![OverlayCell {
                    rect: Rect::new(100, 100, 640, 360),
                    label: "a".to_string(),
                }],
                highlight: None,
                pointer: None,
            };
            let mut guard = HideGuard {
                overlay: Some(overlay),
            };
            let shown = guard
                .overlay
                .as_mut()
                .map(|o| o.show(&frame).is_ok())
                .unwrap_or(false);
            ok &= report(shown, "live overlay presents one frame");
            println!("NEEDS-OWNER: confirm a labeled cell flashed at (100,100), then hid");
            std::thread::sleep(std::time::Duration::from_secs(2));
            drop(guard);
            println!("NEEDS-OWNER: confirm no grid pixels remain on screen");
        }
        Err(err) => {
            println!("NEEDS-OWNER: overlay unavailable headless ({err})");
        }
    }

    println!("NEEDS-OWNER: Notepad typing before/during/after capture; unbound keys type");
    println!("NEEDS-OWNER: PrintScreen and Win+Shift+S during Grid reach the screenshot tools");
    println!("NEEDS-OWNER: tray pause/resume, second launch opens Settings, Quit removes the icon");
    println!("NEEDS-OWNER: 100/150/200% DPI, mixed-DPI monitors, monitor removal while grid shows");
    println!("NEEDS-OWNER: Narrator names every Settings control; high-contrast focus visible");
    ok
}
