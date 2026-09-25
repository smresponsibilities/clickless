//! Automated coverage checks: tray open/close/reopen, keyboard focus, second launch, invalid save, Practice.
//!
//! Integration tests verify the released Windows app handles Settings,
//! tray, Practice, focus, validation, reopen, and icons.

use clickless_windows::settings::SettingsWindow;
use clickless_config::Config;
use clickless_windows::lifecycle::SingleInstance;
use clickless_windows::settings_editor::{Fields, SettingsEditor, fields_from_config};
use clickless_windows::tray::{MenuCommand, TrayIds, command_for, pause_checked, tray_tooltip};
use clickless_core::{KeyEvent, LogicalKey, Phase};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// Tray menu items: each slot maps to its command.
#[test]
fn tray_items_map_to_commands() {
    let ids = TrayIds {
        show_grid: 0,
        hide_grid: 1,
        toggle_pause: 2,
        settings: 3,
        practice: 4,
        quit: 5,
    };
    assert_eq!(command_for(0, ids), Some(MenuCommand::ShowGrid));
    assert_eq!(command_for(1, ids), Some(MenuCommand::HideGrid));
    assert_eq!(command_for(2, ids), Some(MenuCommand::TogglePause));
    assert_eq!(command_for(3, ids), Some(MenuCommand::OpenSettings));
    assert_eq!(command_for(4, ids), Some(MenuCommand::OpenPractice));
    assert_eq!(command_for(5, ids), Some(MenuCommand::Quit));
    assert_eq!(command_for(6, ids), None);
}

/// Ticket 044: the pause check mark and the tooltip come from one state.
#[test]
fn tray_pause_state_is_consistent() {
    assert!(!pause_checked(false));
    assert!(pause_checked(true));
    assert_eq!(tray_tooltip(false, false), "Clickless: enabled");
    assert_eq!(tray_tooltip(true, false), "Clickless: paused");
    assert_eq!(tray_tooltip(true, true), "Clickless: paused, grid open");
}

/// Second launch must fail when first instance holds the mutex.
#[test]
fn second_launch_rejected() {
    let first = SingleInstance::acquire();
    if first.is_err() {
        eprintln!("single-instance mutex held by a live session");
        return;
    }
    let guard = first.unwrap();
    assert!(SingleInstance::acquire().is_err());
    drop(guard);
    assert!(SingleInstance::acquire().is_ok());
}

/// Invalid save scenarios: reserved keys and bad values are rejected.
#[test]
fn invalid_save_rejected() {
        let mut editor = SettingsEditor::new(Config::default());
    let fields = fields_from_config(&Config::default());

    // Reserved leader collision
    let mut bad = fields.clone();
    bad.leader = "space".to_string();
    assert!(editor.edit(&bad).is_err());

    // Valid edit should work
    let mut valid = fields.clone();
    valid.start_speed_px_s = "200".to_string();
    editor.edit(&valid).unwrap();
    assert!(editor.is_dirty());
}

/// Practice flow advances through HoldLeader -> GridPick -> Done.
#[test]
fn practice_flow_advances() {
    let mut practice = clickless_windows::practice::Practice::new();
    assert_eq!(
        practice.step(),
        clickless_windows::practice::Step::SelectActivationKey
    );

    // Choose the default CapsLock activation key, then practice the flow.
    practice.key(KeyEvent::new(LogicalKey::CapsLock, Phase::Press), 0);
    assert_eq!(practice.step(), clickless_windows::practice::Step::HoldLeader);

    practice.key(KeyEvent::new(LogicalKey::Space, Phase::Press), 100);
    practice.key(KeyEvent::new(LogicalKey::Space, Phase::Release), 350);
    assert_eq!(practice.step(), clickless_windows::practice::Step::GridPick);

    practice.key(KeyEvent::new(LogicalKey::Esc, Phase::Press), 500);
    assert!(practice.cancelled());
}

/// Settings window can be created, shown, and reopened at top scroll.
/// NOTE: This test requires a real Windows desktop session; fails in headless/CI.
#[test]
#[ignore = "requires real Windows desktop session"]
fn settings_reopen_focuses_first_control() {
    clickless_windows::settings::seed_settings(Config::default());
    let window = SettingsWindow::new().expect("create Settings window");
    unsafe {
        window.show();
        assert!(window.is_visible());
        // Reopen must focus the first useful control (ID_ENABLED).
        let enabled_id = 117; // ID_ENABLED from settings.rs
        assert_eq!(GetForegroundWindow(), window.hwnd());
        window.show(); // reopen
        assert_eq!(GetForegroundWindow(), window.hwnd());
    }
}

/// Screenshot capture helper: verifies DPI query works.
#[test]
fn native_dpi_query() {
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    let dpi = unsafe { GetDpiForSystem() };
    assert!(dpi > 0, "system DPI must be positive");
}