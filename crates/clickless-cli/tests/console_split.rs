use std::path::PathBuf;

fn cli_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn gui_binary_uses_windows_subsystem_and_console_target_exists() {
    let main_rs = std::fs::read_to_string(cli_dir().join("src/main.rs")).unwrap();
    assert!(
        main_rs.contains("windows_subsystem"),
        "clickless.exe must select GUI subsystem before process creation"
    );

    let ctl_path = cli_dir().join("src/bin/clicklessctl.rs");
    assert!(
        ctl_path.is_file(),
        "console diagnostic target src/bin/clicklessctl.rs must exist"
    );
    let ctl_rs = std::fs::read_to_string(&ctl_path).unwrap();
    assert!(
        !ctl_rs.contains("windows_subsystem"),
        "clicklessctl must stay console so --help output is readable"
    );
}

#[test]
fn both_targets_share_one_runtime_function() {
    let main_rs = std::fs::read_to_string(cli_dir().join("src/main.rs")).unwrap();
    let ctl_rs = std::fs::read_to_string(cli_dir().join("src/bin/clicklessctl.rs")).unwrap();
    assert!(
        main_rs.contains("clickless_cli::run") || main_rs.contains("clickless-cli::run"),
        "GUI target must call the shared run function"
    );
    assert!(
        ctl_rs.contains("clickless_cli::run") || ctl_rs.contains("clickless-cli::run"),
        "console target must call the shared run function"
    );
}
