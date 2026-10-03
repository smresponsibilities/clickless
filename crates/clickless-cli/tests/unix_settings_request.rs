#![cfg(any(target_os = "linux", target_os = "macos"))]

use clickless_cli::unix_lifecycle::{SettingsRequest, SingleInstance, notify_open_settings};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn sigterm_allows_graceful_shutdown() {
    const NAME: &str = "sigterm_allows_graceful_shutdown";
    if !isolated_child(NAME) {
        return;
    }
    if std::env::var_os("CLICKLESS_SIGNAL_CHILD").is_some() {
        let _owner = SingleInstance::acquire().unwrap();
        let (send, receive) = mpsc::channel();
        ctrlc::set_handler(move || {
            let _ = send.send(());
        })
        .unwrap();
        println!("CLICKLESS_SIGNAL_READY");
        std::io::stdout().flush().unwrap();
        receive.recv_timeout(Duration::from_secs(10)).unwrap();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env("CLICKLESS_SIGNAL_CHILD", "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (send, receive) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        let ready = BufReader::new(stdout)
            .lines()
            .any(|line| line.unwrap().contains("CLICKLESS_SIGNAL_READY"));
        let _ = send.send(ready);
    });
    let ready = receive.recv_timeout(Duration::from_secs(10));
    if !matches!(ready, Ok(true)) {
        let _ = child.kill();
        let _ = child.wait();
        panic!("signal child failed to become ready: {ready:?}");
    }
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "SIGTERM must return through normal cleanup: {status}"
    );
    assert!(SingleInstance::acquire().is_ok());
}

fn isolated_child(test: &str) -> bool {
    // Change fixture environment only in a child. Tests acquire ownership
    // before sending socket traffic, so they never signal a live instance.
    if std::env::var_os("CLICKLESS_SETTINGS_TEST_CHILD").is_none() {
        let _test_lock = TEST_LOCK.lock().unwrap();
        // Keep below macOS's Unix socket path limit even when its default
        // temporary directory has a long /var/folders path.
        let directory = std::path::Path::new("/tmp").join(format!(
            "cl-settings-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env("CLICKLESS_SETTINGS_TEST_CHILD", "1")
            .env("TMPDIR", &directory)
            .env("USER", "test")
            .status();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(result.unwrap().success());
        return false;
    }
    true
}

#[test]
fn single_instance_ignores_user_environment() {
    const NAME: &str = "single_instance_ignores_user_environment";
    if !isolated_child(NAME) {
        return;
    }
    if std::env::var_os("CLICKLESS_OWNER_CONTENDER").is_some() {
        assert!(
            SingleInstance::acquire().is_err(),
            "same user must not acquire a second owner"
        );
        return;
    }
    let owner = SingleInstance::acquire().unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env("CLICKLESS_OWNER_CONTENDER", "1")
        .env("USER", "different-user-text")
        .env("TMPDIR", "/tmp")
        .status()
        .unwrap();
    assert!(result.success(), "USER text must not bypass ownership");
    drop(owner);
    assert!(
        SingleInstance::acquire().is_ok(),
        "normal exit releases ownership"
    );
}

#[test]
fn crashed_owner_releases_lock_and_stale_settings_endpoint() {
    const NAME: &str = "crashed_owner_releases_lock_and_stale_settings_endpoint";
    if !isolated_child(NAME) {
        return;
    }
    if std::env::var_os("CLICKLESS_OWNER_HOLD").is_some() {
        let mut owner = SingleInstance::acquire().unwrap();
        let _request = SettingsRequest::create(&mut owner).unwrap();
        println!("CLICKLESS_OWNER_READY");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_exact(&mut [0]).unwrap();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env("CLICKLESS_OWNER_HOLD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .any(|line| line.unwrap().contains("CLICKLESS_OWNER_READY"));
    assert!(
        ready,
        "child must acquire ownership before contention check"
    );
    let contention = SingleInstance::acquire().err().unwrap().kind();
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(contention, std::io::ErrorKind::AlreadyExists);
    let mut owner = SingleInstance::acquire().unwrap();
    let request = SettingsRequest::create(&mut owner).unwrap();
    request.signal().unwrap();
    assert!(request.poll(), "new owner recovers crash-left socket");
}

#[test]
fn settings_creation_preserves_non_socket_files() {
    const NAME: &str = "settings_creation_preserves_non_socket_files";
    if !isolated_child(NAME) {
        return;
    }
    let mut owner = SingleInstance::acquire().unwrap();
    let uid = std::fs::metadata(std::env::temp_dir()).unwrap().uid();
    let path = std::path::PathBuf::from(format!("/tmp/clickless-{uid}/settings.sock"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(b"preserve these bytes").unwrap();
    drop(file);
    let rejected = SettingsRequest::create(&mut owner).is_err();
    let contents = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        rejected,
        "unexpected files must not be replaced with sockets"
    );
    assert_eq!(contents, b"preserve these bytes");
}

fn send_raw_request(payload: &[u8]) -> Option<UnixStream> {
    let uid = std::fs::metadata(std::env::temp_dir()).unwrap().uid();
    let path = std::path::PathBuf::from(format!("/tmp/clickless-{uid}/settings.sock"));
    // Support the previous stream transport so the tests reproduce its bugs
    // as well as exercising the atomic replacement.
    match UnixStream::connect(&path) {
        Ok(mut client) => {
            client.write_all(payload).unwrap();
            Some(client)
        }
        Err(_) => {
            UnixDatagram::unbound()
                .unwrap()
                .send_to(payload, &path)
                .unwrap();
            None
        }
    }
}

#[test]
fn settings_requests_do_not_wait_for_partial_clients() {
    if !isolated_child("settings_requests_do_not_wait_for_partial_clients") {
        return;
    }

    let mut owner = SingleInstance::acquire().unwrap();
    let request = SettingsRequest::create(&mut owner).unwrap();
    assert!(!request.poll());
    request.signal().unwrap();
    assert!(request.poll());
    assert!(!request.poll());

    let partial_client = send_raw_request(b"op");
    let (sender, receiver) = mpsc::channel();
    let (request, result) = std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            sender.send(request.poll()).unwrap();
            request
        });
        let result = receiver.recv_timeout(Duration::from_secs(1));
        drop(partial_client);
        (worker.join().unwrap(), result)
    });
    assert_eq!(result, Ok(false), "partial client must not stall polling");
    request.signal().unwrap();
    assert!(
        request.poll(),
        "valid requests still work after malformed input"
    );
    assert!(!request.poll());
}

#[test]
fn settings_requests_reject_oversized_messages() {
    if !isolated_child("settings_requests_reject_oversized_messages") {
        return;
    }
    let mut owner = SingleInstance::acquire().unwrap();
    let request = SettingsRequest::create(&mut owner).unwrap();
    drop(send_raw_request(b"open extra"));
    assert!(!request.poll(), "only an exact open message is valid");
    drop(send_raw_request(b"nope"));
    request.signal().unwrap();
    assert!(!request.poll(), "poll handles at most one request");
    assert!(
        request.poll(),
        "malformed input does not discard the next request"
    );
    assert!(!request.poll());
    drop(request);
    assert!(
        notify_open_settings().is_err(),
        "missing endpoint must report failure"
    );
}
