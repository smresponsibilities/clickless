//! GUI error surface for the console-free Windows target.
//!
//! Normal `clickless.exe` runs under the GUI subsystem, so `stderr` goes
//! nowhere. Failures here get one concise dialog plus one appended log line.
//! The console diagnostic target keeps plain `stderr`.

#[cfg(windows)]
pub mod win {
    use std::fmt::Write as _;

    /// Hard cap for the local log file. Rotation keeps the tail: the newest
    /// lines survive no matter the volume.
    pub const MAX_LOG_BYTES: usize = 64 * 1024;
    /// Newest log bytes carried inside copied diagnostics.
    pub const MAX_DIAG_TAIL: usize = 4096;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn log_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(
            std::path::Path::new(&base)
                .join("clickless")
                .join("clickless.log"),
        )
    }

    fn append_bounded(existing: Vec<u8>, line: &str) -> Vec<u8> {
        let mut text = String::from_utf8_lossy(&existing).into_owned();
        let _ = writeln!(text, "{line}");
        if text.len() > MAX_LOG_BYTES {
            let cut = text.len() - MAX_LOG_BYTES;
            text = text[text.floor_char_boundary(cut)..].to_string();
        }
        text.into_bytes()
    }

    fn append_log(line: &str) {
        if let Some(path) = log_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let existing = std::fs::read(&path).unwrap_or_default();
            let _ = std::fs::write(&path, append_bounded(existing, line));
        }
    }

    /// Runtime log line for the console-free target. Bounded, never panics,
    /// never writes outside the log file. Callers pass complete lines only:
    /// keys, typed text, pointer history and full configs must never reach
    /// this function; error strings name LogicalKeys from config, never
    /// keystrokes.
    pub fn log_event(line: &str) {
        append_log(line);
    }

    /// Shows one concise dialog and appends one log line. Never panics.
    pub fn gui_error(message: &str) {
        append_log(message);
        // Test/automation runs must not block on a modal native dialog.
        if std::env::var_os("CLICKLESS_NO_DIALOG").is_some() {
            return;
        }
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
            let text = wide(message);
            let caption = wide("Clickless");
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    /// Builds copyable diagnostics from parts. Takes a config *summary*,
    /// never a full config: the signature makes logging secrets impossible.
    /// The tail keeps its newest bytes under a fixed cap.
    pub fn diagnostics_text(
        version: &str,
        os: &str,
        log_tail: &str,
        config_summary: &str,
    ) -> String {
        let start = log_tail.len().saturating_sub(MAX_DIAG_TAIL);
        let tail = &log_tail[log_tail.floor_char_boundary(start)..];
        format!("Clickless {version} ({os})\nconfig: {config_summary}\n--- recent log ---\n{tail}")
    }

    /// Log path used above, exposed for tests.
    pub fn log_file_path() -> Option<std::path::PathBuf> {
        log_path()
    }
}

#[cfg(windows)]
pub use win::{diagnostics_text, gui_error, log_event, log_file_path};
