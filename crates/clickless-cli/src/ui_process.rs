//! Separate desktop process. Runtime owns capture and acknowledges changes.
use clickless_config::{Config, ui_protocol};
use std::{
    io::{BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
};

pub enum Request {
    Apply(Box<Config>),
    Focus(bool),
    Quit,
}
pub struct UiProcess {
    child: Child,
    input: ChildStdin,
    requests: mpsc::Receiver<Request>,
    pub focused: bool,
    failed: bool,
}
impl UiProcess {
    pub fn spawn(config: &Config, path: &Path, home: bool) -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("clickless-settings");
        let mut command = Command::new(executable);
        command
            .args(["--runtime-unix", "--config"])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        if home {
            command.arg("--home");
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot start desktop UI: {e}"))?;
        let mut input = child.stdin.take().ok_or("UI input pipe missing")?;
        if let Err(error) = ui_protocol::write_config(&mut input, config) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let output = child.stdout.take().ok_or("UI output pipe missing")?;
        let (tx, requests) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            while let Ok(command) = ui_protocol::read_line(&mut reader) {
                let request = match command.as_str() {
                    "apply" => match ui_protocol::read_config(&mut reader) {
                        Ok(config) => Request::Apply(Box::new(config)),
                        Err(_) => break,
                    },
                    "quit" => Request::Quit,
                    "focus 1" => Request::Focus(true),
                    "focus 0" => Request::Focus(false),
                    _ => break,
                };
                if tx.send(request).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            requests,
            focused: true,
            failed: false,
        })
    }
    pub fn request(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }
    pub fn reply(&mut self, result: Result<(), String>) -> Result<(), String> {
        let text = match result {
            Ok(()) => "OK".to_string(),
            Err(e) => format!("ERR {}", e.replace(['\r', '\n'], " ")),
        };
        let result = writeln!(self.input, "{text}")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string());
        self.failed |= result.is_err();
        result
    }
    pub fn closed(&mut self) -> Result<bool, String> {
        if self.failed {
            return Ok(true);
        }
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|e| e.to_string())
    }
    pub fn sync(&mut self, config: &Config) -> Result<(), String> {
        let result = writeln!(self.input, "state")
            .map_err(|e| e.to_string())
            .and_then(|_| ui_protocol::write_config(&mut self.input, config));
        self.failed |= result.is_err();
        result
    }
    pub fn show(&mut self, home: bool) -> Result<(), String> {
        writeln!(self.input, "{}", if home { "show home" } else { "show" })
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())
    }
}
impl Drop for UiProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run() -> Result<(), String> {
    let mut path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                path = Some(args.next().ok_or("--config requires a file path")?.into())
            }
            "--home" | "--gpui" | "--runtime-unix" => {}
            "--help" | "-h" => {
                println!("clickless-settings [--home] [--config FILE]");
                return Ok(());
            }
            _ => return Err(format!("Unknown Settings argument: {arg}")),
        }
    }
    let path = match path {
        Some(path) => path,
        None => clickless_config::default_config_path().map_err(|e| e.to_string())?,
    };
    let connected = std::env::args().any(|arg| arg == "--runtime-unix");
    let mut input = BufReader::new(std::io::stdin());
    let seed = if connected {
        ui_protocol::read_config(&mut input)?
    } else if path.exists() {
        Config::load_from_file(&path).map_err(|e| e.to_string())?
    } else {
        #[cfg(target_os = "macos")]
        {
            clickless_macos::default_config()
        }
        #[cfg(not(target_os = "macos"))]
        Config::default()
    };
    let replies = ui_protocol::read_replies(input);
    let responses = std::sync::Mutex::new(replies.responses);
    let mut usable = true;
    let apply: clickless_ui::ApplyCallback = Box::new(move |config| {
        if !usable {
            return Err(
                "Previous Apply was not acknowledged. Reopen Settings before retrying.".into(),
            );
        }
        let mut output = std::io::stdout().lock();
        writeln!(output, "apply").map_err(|e| e.to_string())?;
        ui_protocol::write_config(&mut output, config)?;
        drop(output);
        let reply = responses
            .lock()
            .map_err(|e| e.to_string())?
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| {
                usable = false;
                format!("Apply acknowledgement failed: {e}. Reopen Settings before retrying.")
            })?;
        if reply == "OK" { Ok(()) } else { Err(reply) }
    });
    let connection = connected.then_some(clickless_ui::RuntimeConnection {
        updates: replies.updates,
        alive: replies.alive,
        show: replies.show,
    });
    clickless_ui::run_connected(seed, path, connected.then_some(apply), connection, {
        #[cfg(target_os = "macos")]
        {
            clickless_ui::Defaults {
                config: clickless_macos::default_config(),
                activation_keys: vec![
                    clickless_core::LogicalKey::Space,
                    clickless_core::LogicalKey::ControlLeft,
                    clickless_core::LogicalKey::ShiftLeft,
                ],
                ..Default::default()
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            clickless_ui::Defaults::default()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exited_companion_transport_is_retired_without_runtime_error() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 0"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        child.wait().unwrap();
        let (_, requests) = mpsc::channel();
        let mut ui = UiProcess {
            child,
            input,
            requests,
            focused: true,
            failed: false,
        };
        assert!(ui.sync(&Config::default()).is_err());
        assert!(ui.closed().unwrap());
        assert!(ui.reply(Ok(())).is_err());
        assert!(ui.closed().unwrap());
    }
}
