use std::process::Command;

#[test]
fn unsupported_invocations_fail_instead_of_claiming_initialization() {
    for args in [
        vec!["clickless://show-overlay"],
        vec!["--config"],
        vec!["--unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_clickless"))
            .args(&args)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("initialized successfully"));
    }
}

#[test]
fn explicit_validation_and_information_commands_succeed() {
    for args in [vec!["--check-config"], vec!["--help"], vec!["--version"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_clickless"))
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_clickless"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_clickless"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--smoke-output"));
    assert!(stdout.to_lowercase().contains("cursor"));
}
