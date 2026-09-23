//! Ticket 028: malformed startup config recovers paused on defaults.
//! The file is never touched; Settings opens with the error.

use clickless_cli::resolve_startup;
use clickless_config::Config;
use std::fs;

fn write_temp(name: &str, body: &str) -> std::path::PathBuf {
    // Unique dir per call: the tests in this file run in parallel and a
    // shared directory raced on remove/create.
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "clickless-startup-test-{}-{}",
        std::process::id(),
        id
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn startup_without_file_boots_defaults_unpaused() {
    let plan = resolve_startup(None);
    assert_eq!(plan.config, Config::default());
    assert!(!plan.start_paused);
    assert!(plan.notice.is_none());
}

#[test]
fn startup_with_valid_file_boots_it_unpaused() {
    let path = write_temp("good.toml", "[settings]\nhold_ms = 350\n");
    let plan = resolve_startup(Some(&path));
    assert_eq!(plan.config.settings.hold_ms, 350);
    assert!(!plan.start_paused);
    assert!(plan.notice.is_none());
}

#[test]
fn startup_with_malformed_file_boots_paused_on_defaults_and_preserves_bytes() {
    let body = "[settings]\nhold_ms = 0\nnote = \"hunter2-secret\"\n";
    let path = write_temp("bad.toml", body);
    let plan = resolve_startup(Some(&path));
    assert_eq!(plan.config, Config::default());
    assert!(plan.start_paused, "malformed config must start paused");
    let notice = plan.notice.expect("malformed config must explain itself");
    assert!(
        notice.contains("hold_ms"),
        "notice names the fault: {notice}"
    );
    assert!(
        !notice.contains("hunter2"),
        "notice must not echo file content: {notice}"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        body,
        "malformed file preserved byte-for-byte"
    );
}
