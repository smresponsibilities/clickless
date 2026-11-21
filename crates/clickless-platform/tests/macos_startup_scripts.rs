#[cfg(unix)]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
    };

    struct Fixture {
        directory: tempfile::TempDir,
    }
    impl Fixture {
        fn new() -> Self {
            let fixture = Self {
                directory: tempfile::tempdir().unwrap(),
            };
            fs::create_dir_all(fixture.home().join("Library/LaunchAgents")).unwrap();
            fs::create_dir(fixture.home().join("bin")).unwrap();
            fixture.command("uname", "echo Darwin");
            fixture.command("id", "echo 501");
            fixture.command(
                "plutil",
                "printf '%s\\n' \"$*\" >> \"$HOME/plutil.log\"\n[ ! -f \"$HOME/fail-lint\" ]",
            );
            fixture.command(
                "launchctl",
                r#"
printf '%s\n' "$*" >> "$HOME/launch.log"
case "$1" in
 print) test ! -f "$HOME/fail-query" || exit 1; test -f "$HOME/loaded" || exit 113 ;;
 bootout) test ! -f "$HOME/fail-bootout" || exit 1; rm -f "$HOME/loaded" ;;
 bootstrap)
   count=0; [ ! -f "$HOME/count" ] || count=$(cat "$HOME/count")
   count=$((count + 1)); echo "$count" > "$HOME/count"
   [ ! -f "$HOME/fail-all-bootstrap" ] || exit 1
   if [ -f "$HOME/fail-first-bootstrap" ] && [ "$count" = 1 ]; then exit 1; fi
   touch "$HOME/loaded" ;;
 *) exit 99 ;;
esac
"#,
            );
            fixture.command(
                "mv",
                "[ ! -f \"$HOME/fail-rename\" ] || exit 1\nexec /bin/mv \"$@\"",
            );
            fixture.command("rm", "if [ -f \"$HOME/fail-delete\" ]; then for arg in \"$@\"; do case \"$arg\" in *.plist) exit 1;; esac; done; fi\nexec /bin/rm \"$@\"");
            fixture.command("clickless path & <xml>", "exit 0");
            fixture
        }
        fn home(&self) -> &Path {
            self.directory.path()
        }
        fn plist(&self) -> PathBuf {
            self.home()
                .join("Library/LaunchAgents/org.smresponsibilities.clickless.plist")
        }
        fn command(&self, name: &str, body: &str) {
            let path = self.home().join("bin").join(name);
            fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fn flag(&self, name: &str) {
            fs::write(self.home().join(name), "").unwrap();
        }
        fn prior(&self, loaded: bool) {
            fs::write(self.plist(), b"exact prior plist bytes\n").unwrap();
            if loaded {
                self.flag("loaded");
            }
        }
        fn run(&self, enable: bool) -> Output {
            let script =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/startup-macos.sh");
            let mut command = Command::new("bash");
            command
                .arg(script)
                .arg(if enable { "--enable" } else { "--disable" });
            if enable {
                command.arg(self.home().join("bin/clickless path & <xml>"));
            }
            command
                .env("HOME", self.home())
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", self.home().join("bin").display()),
                )
                .output()
                .unwrap()
        }
        fn log(&self) -> String {
            fs::read_to_string(self.home().join("launch.log")).unwrap_or_default()
        }
        fn assert_prior(&self, loaded: bool) {
            assert_eq!(
                fs::read(self.plist()).unwrap(),
                b"exact prior plist bytes\n"
            );
            assert_eq!(self.home().join("loaded").exists(), loaded);
        }
    }

    #[test]
    fn rename_failure_restores_stopped_agent() {
        let fixture = Fixture::new();
        fixture.prior(true);
        fixture.flag("fail-rename");
        assert!(!fixture.run(true).status.success());
        fixture.assert_prior(true);
        assert_eq!(fixture.log().matches("bootstrap ").count(), 1);
    }

    #[test]
    fn disable_delete_failure_restores_stopped_agent() {
        let fixture = Fixture::new();
        fixture.prior(true);
        fixture.flag("fail-delete");
        assert!(!fixture.run(false).status.success());
        fixture.assert_prior(true);
        assert_eq!(fixture.log().matches("bootstrap ").count(), 1);
    }
    #[test]
    fn query_failure_prevents_mutation() {
        for enable in [true, false] {
            let fixture = Fixture::new();
            fixture.prior(true);
            fixture.flag("fail-query");
            assert!(!fixture.run(enable).status.success());
            fixture.assert_prior(true);
            assert!(!fixture.log().contains("bootout"));
        }
    }

    #[test]
    fn invalid_executable_cannot_modify_startup() {
        let fixture = Fixture::new();
        fixture.prior(true);
        fs::set_permissions(
            fixture.home().join("bin/clickless path & <xml>"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(!fixture.run(true).status.success());
        fixture.assert_prior(true);
        assert!(!fixture.log().contains("bootout"));
    }
    #[test]
    fn enable_disable_are_isolated_and_repeatable() {
        let fixture = Fixture::new();
        let unrelated = fixture.home().join("Library/LaunchAgents/unrelated.plist");
        fs::write(&unrelated, "untouched").unwrap();
        assert!(!fixture.plist().exists());
        for _ in 0..2 {
            assert!(fixture.run(true).status.success());
            assert!(fixture.home().join("loaded").exists());
        }
        let log = fs::read_to_string(fixture.home().join("plutil.log")).unwrap();
        assert!(log.contains("clickless path & <xml>"));
        for _ in 0..2 {
            assert!(fixture.run(false).status.success());
            assert!(!fixture.plist().exists());
            assert!(!fixture.home().join("loaded").exists());
        }
        assert_eq!(fs::read_to_string(unrelated).unwrap(), "untouched");
    }

    #[test]
    fn bootout_failure_preserves_loaded_agent_and_bytes() {
        for enable in [true, false] {
            let fixture = Fixture::new();
            fixture.prior(true);
            fixture.flag("fail-bootout");
            assert!(!fixture.run(enable).status.success());
            fixture.assert_prior(true);
            assert!(!fixture.log().contains("bootstrap"));
        }
    }

    #[test]
    fn bootstrap_failure_restores_only_previously_loaded_agent() {
        for loaded in [true, false] {
            let fixture = Fixture::new();
            fixture.prior(loaded);
            fixture.flag("fail-first-bootstrap");
            assert!(!fixture.run(true).status.success());
            fixture.assert_prior(loaded);
            let log = fixture.log();
            let operations: Vec<_> = log
                .lines()
                .map(|line| line.split_whitespace().next().unwrap())
                .collect();
            assert_eq!(
                operations,
                if loaded {
                    vec!["print", "bootout", "bootstrap", "bootstrap"]
                } else {
                    vec!["print", "bootstrap"]
                }
            );
        }
    }

    #[test]
    fn failed_recovery_preserves_backup() {
        let fixture = Fixture::new();
        fixture.prior(true);
        fixture.flag("fail-all-bootstrap");
        let result = fixture.run(true);
        assert!(!result.status.success());
        fixture.assert_prior(false);
        assert!(String::from_utf8_lossy(&result.stderr).contains("Recovery failure"));
        let backups: Vec<_> = fs::read_dir(fixture.plist().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.to_string_lossy().contains(".prior."))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0]).unwrap(), b"exact prior plist bytes\n");
    }

    #[test]
    fn loaded_without_prior_file_is_rejected_before_bootout() {
        let fixture = Fixture::new();
        fixture.flag("loaded");
        assert!(!fixture.run(true).status.success());
        assert!(fixture.home().join("loaded").exists());
        assert!(!fixture.plist().exists());
        assert!(!fixture.log().contains("bootout"));
    }

    #[test]
    fn validation_failure_does_not_stop_prior_agent() {
        let fixture = Fixture::new();
        fixture.prior(true);
        fixture.flag("fail-lint");
        assert!(!fixture.run(true).status.success());
        fixture.assert_prior(true);
        assert!(!fixture.log().contains("bootout"));
    }

    #[test]
    fn failed_first_install_leaves_startup_disabled() {
        let fixture = Fixture::new();
        fixture.flag("fail-first-bootstrap");
        assert!(!fixture.run(true).status.success());
        assert!(!fixture.plist().exists());
        assert!(!fixture.home().join("loaded").exists());
        assert_eq!(fixture.log().matches("bootstrap ").count(), 1);
    }
}
