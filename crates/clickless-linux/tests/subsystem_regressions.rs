#![cfg(target_os = "linux")]

#[test]
fn installer_preserves_opt_in_and_supports_disable_in_isolated_directories() {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!("clickless-install-test-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install-linux.sh");
    let run = |args: &[&str]| {
        let output = Command::new("bash")
            .arg(&script)
            .args(args)
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&[]);
    assert!(root.join("data/applications/clickless.desktop").is_file());
    let startup = root.join("config/autostart/clickless.desktop");
    assert!(!startup.exists());
    run(&["--enable-autostart"]);
    assert!(
        fs::read_to_string(&startup)
            .unwrap()
            .contains("X-GNOME-Autostart-enabled=true")
    );
    run(&[]);
    assert!(startup.exists());
    run(&["--disable-autostart"]);
    assert!(!startup.exists());
    fs::remove_dir_all(&root).unwrap();
}
