//! Settings runs outside the input-hook process. Private inherited pipes carry
//! validated configuration and acknowledgements; UI threads never touch hooks.

use clickless_core::config::Config;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, mpsc};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};

pub(crate) fn config_path() -> Result<std::path::PathBuf, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" || arg == "-c" {
            return args
                .next()
                .map(std::path::PathBuf::from)
                .ok_or("--config requires a file path".into());
        }
    }
    clickless_core::config::default_config_path().map_err(|e| e.to_string())
}

type ApplyCallback = Box<dyn FnMut(&Config) -> Result<(), String> + Send>;

pub(crate) enum Request {
    Apply(Box<Config>),
    Practice,
    Quit,
}

pub fn use_gpui() -> bool {
    cfg!(feature = "gpui-settings")
        && !std::env::var("CLICKLESS_SETTINGS_UI").is_ok_and(|host| host == "native")
}

fn read_config(reader: &mut impl BufRead) -> Result<Config, String> {
    clickless_core::config::ui_protocol::read_config(reader)
}
fn write_config(writer: &mut impl Write, config: &Config) -> Result<(), String> {
    clickless_core::config::ui_protocol::write_config(writer, config)
}

pub(crate) struct SettingsProcess {
    child: Mutex<Child>,
    input: clickless_core::config::ui_protocol::UiWriter,
    requests: mpsc::Receiver<Request>,
}

impl SettingsProcess {
    pub(crate) fn spawn(seed: Config) -> Result<Self, String> {
        Self::spawn_screen(seed, false)
    }

    pub(crate) fn spawn_home(seed: Config) -> Result<Self, String> {
        Self::spawn_screen(seed, true)
    }

    fn spawn_screen(seed: Config, home: bool) -> Result<Self, String> {
        use std::os::windows::process::CommandExt;
        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("clickless-settings.exe");
        let mut child = Command::new(exe)
            .arg("--runtime")
            .args(if home { vec!["--home"] } else { vec![] })
            .arg("--config")
            .arg(config_path()?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| format!("Cannot start Settings: {e}"))?;
        let pipe = child.stdin.take().ok_or("Settings input pipe missing")?;
        let input = clickless_core::config::ui_protocol::UiWriter::new(pipe);
        let input = match input {
            Ok(input) => input,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let mut seed_bytes = Vec::new();
        if let Err(error) =
            write_config(&mut seed_bytes, &seed).and_then(|()| input.send(seed_bytes))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let output = child.stdout.take().ok_or("Settings output pipe missing")?;
        let (tx, requests) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            while let Ok(command) = clickless_core::config::ui_protocol::read_line(&mut reader) {
                let request = match command.trim() {
                    "apply" => match read_config(&mut reader) {
                        Ok(config) => Request::Apply(Box::new(config)),
                        Err(_) => break,
                    },
                    "practice" => Request::Practice,
                    "quit" => Request::Quit,
                    _ => break,
                };
                if tx.send(request).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child: Mutex::new(child),
            input,
            requests,
        })
    }

    pub(crate) fn next_request(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }

    fn send(&self, bytes: Vec<u8>) {
        if self.input.send(bytes).is_err()
            && let Ok(mut child) = self.child.lock()
        {
            let _ = child.kill();
        }
    }

    pub(crate) fn reply(&self, result: Result<(), String>) {
        let message = match result {
            Ok(()) => "OK".to_string(),
            Err(error) => format!("ERR {}", error.replace(['\r', '\n'], " ")),
        };
        self.send(format!("{message}\n").into_bytes());
    }

    pub(crate) fn sync(&self, config: &Config) {
        if !use_gpui() {
            return;
        }
        let mut bytes = b"state\n".to_vec();
        if write_config(&mut bytes, config).is_ok() {
            self.send(bytes);
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        !self.input.is_closed()
            && self
                .child
                .lock()
                .is_ok_and(|mut child| child.try_wait().is_ok_and(|status| status.is_none()))
    }

    pub(crate) fn has_focus(&self) -> bool {
        let Ok(child) = self.child.lock() else {
            return false;
        };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
        }
        pid == child.id()
    }

    pub(crate) fn show(&self) {
        if use_gpui() {
            self.send(b"show\n".to_vec());
        }
        unsafe extern "system" fn raise(hwnd: HWND, pid: LPARAM) -> i32 {
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut owner);
                if owner == pid as u32 && IsWindowVisible(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                    return 0;
                }
            }
            1
        }
        if let Ok(child) = self.child.lock() {
            unsafe {
                EnumWindows(Some(raise), child.id() as LPARAM);
            }
        }
    }
}

impl Drop for SettingsProcess {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

pub fn run() -> Result<(), String> {
    let connected = std::env::args().any(|arg| arg == "--runtime");
    let input = std::sync::Arc::new(Mutex::new(BufReader::new(std::io::stdin())));
    let seed = if connected {
        read_config(&mut *input.lock().map_err(|e| e.to_string())?)?
    } else {
        let path = config_path()?;
        if path.exists() {
            Config::load_from_file(path).map_err(|e| e.to_string())?
        } else {
            Config::default()
        }
    };
    let callback: ApplyCallback = Box::new(move |config: &Config| {
        if !connected {
            return Err(
                "Open Settings from Clickless for live Apply. Use Save for the next launch.".into(),
            );
        }
        let mut output = std::io::stdout().lock();
        writeln!(output, "apply").map_err(|e| e.to_string())?;
        write_config(&mut output, config)?;
        drop(output);
        let mut reply = String::new();
        if input
            .lock()
            .map_err(|e| e.to_string())?
            .read_line(&mut reply)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("Clickless closed before applying settings.".into());
        }
        if reply.trim() == "OK" {
            Ok(())
        } else {
            Err(format!("Runtime Apply failed: {}", reply.trim()))
        }
    });
    #[cfg(feature = "winui3")]
    {
        let window = crate::winui_host::enabled::WinUiSettings::create(seed, callback)?;
        while window.is_visible() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    #[cfg(not(feature = "winui3"))]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
        };
        crate::settings::seed_settings(seed);
        let window = crate::settings::SettingsWindow::with_on_apply(callback)?;
        window.show();
        let mut message: MSG = unsafe { std::mem::zeroed() };
        while window.is_visible() {
            unsafe {
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if !window.translate_message(&message) {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    Ok(())
}

pub(crate) fn request_practice() {
    if std::env::args().any(|arg| arg == "--runtime") {
        let mut output = std::io::stdout().lock();
        let _ = writeln!(output, "practice");
        let _ = output.flush();
    }
}
