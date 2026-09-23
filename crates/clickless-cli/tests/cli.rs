use std::process::Command;

// Console assertions run against clicklessctl: clickless.exe is the GUI
// subsystem target and has no console for readable output.
fn ctl() -> String {
    env!("CARGO_BIN_EXE_clicklessctl").to_string()
}

#[test]
fn unsupported_invocations_fail_instead_of_claiming_initialization() {
    for args in [
        vec!["clickless://show-overlay"],
        vec!["--config"],
        vec!["--unknown"],
    ] {
        let output = Command::new(ctl()).args(&args).output().unwrap();
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("initialized successfully"));
    }
}

#[test]
fn explicit_validation_and_information_commands_succeed() {
    for args in [vec!["--check-config"], vec!["--help"], vec!["--version"]] {
        let output = Command::new(ctl()).args(args).output().unwrap();
        assert!(output.status.success());
    }
    let output = Command::new(ctl())
        .args([
            "--check-config",
            "--config",
            "missing-config-for-cli-test.toml",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("IO error"));
}

#[test]
fn help_advertises_bounded_output_smoke_path() {
    let output = Command::new(ctl()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--smoke-output"));
    assert!(stdout.to_lowercase().contains("cursor"));
}

/// The GUI target reports explicit-command errors through the dialog plus
/// the local log instead of stderr: a bad config still fails loudly.
/// (MessageBoxW returns immediately in a headless session.)
#[test]
fn gui_target_logs_explicit_command_errors_instead_of_stderr() {
    let dir = std::env::temp_dir().join("clickless-gui-cli-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gui = env!("CARGO_BIN_EXE_clickless").to_string();
    let output = Command::new(gui)
        .env("LOCALAPPDATA", &dir)
        .env("CLICKLESS_NO_DIALOG", "1")
        .args([
            "--check-config",
            "--config",
            "missing-config-for-cli-test.toml",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let log = std::fs::read_to_string(dir.join("clickless").join("clickless.log")).unwrap();
    assert!(log.contains("IO error"), "log names the fault: {log}");
    let _ = std::fs::remove_dir_all(&dir);
}
