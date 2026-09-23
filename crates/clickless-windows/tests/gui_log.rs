//! Ticket 029: GUI log rotation, unwritable location, no stray writes.
//! LOCALAPPDATA is redirected per test under one process-wide lock because
//! the variable is global to the test process.
//!
//! Windows-only: the GUI log, dialog and clipboard path do not exist on
//! other targets.

#![cfg(windows)]

use clickless_windows::gui_error::{diagnostics_text, log_event, log_file_path};
use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn temp_base(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("clickless-gui-log-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn log_file_never_exceeds_64kib_and_keeps_the_tail() {
    let _guard = env_lock();
    let base = temp_base("rotate");
    unsafe { std::env::set_var("LOCALAPPDATA", &base) };

    for i in 0..2000 {
        log_event(&format!("line {i:04} {:0>90}", ""));
    }
    let path = log_file_path().expect("log path with LOCALAPPDATA set");
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        bytes.len() <= 64 * 1024,
        "log grew past the bound: {} bytes",
        bytes.len()
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("line 1999"),
        "rotation must keep the newest line"
    );
}

#[test]
fn unwritable_log_location_never_panics_and_writes_nowhere() {
    let _guard = env_lock();
    let dir = std::env::temp_dir().join("clickless-gui-log-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A file where the directory should be: every create/read/write fails.
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    unsafe { std::env::set_var("LOCALAPPDATA", &blocker) };

    log_event("must not panic");
    assert!(log_file_path().is_some());
    assert!(
        !blocker.join("clickless").exists(),
        "nothing may be written beside a broken location"
    );
}

#[test]
fn diagnostics_text_bounds_the_tail_and_states_config_summary() {
    let tail = "x".repeat(8192);
    let text = diagnostics_text("0.1.0", "windows-x86_64", &tail, "valid");
    assert!(text.contains("0.1.0"), "version_first: {text}");
    assert!(text.contains("config: valid"), "summary present: {text}");
    assert!(
        text.len() <= 4096 + 512,
        "diagnostics stays small: {} bytes",
        text.len()
    );
    assert!(
        text.ends_with(&"x".repeat(64)),
        "newest tail bytes kept, oldest cut"
    );
}
