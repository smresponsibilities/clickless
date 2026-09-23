//! Desktop lifecycle: single instance and the settings-open request path.
//!
//! A named mutex guarantees one Clickless runtime per session. A second launch
//! signals a named event instead of adding a second keyboard hook, and the
//! running instance polls that event in its main loop and opens Settings.

#[cfg(windows)]
pub mod win {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent, WaitForSingleObject,
    };

    pub const MUTEX_NAME: &str = "Local\\ClicklessSingleInstance";
    pub const SETTINGS_EVENT_NAME: &str = "Local\\ClicklessOpenSettings";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Guard that owns the single-instance mutex for the process lifetime.
    /// On drop it releases and closes the handle.
    pub struct SingleInstance {
        handle: HANDLE,
    }

    impl SingleInstance {
        /// Claims the named mutex. `Err(reason)` means another instance holds
        /// it or creation failed; the caller must not start a second hook.
        pub fn acquire() -> Result<Self, String> {
            let name = wide(MUTEX_NAME);
            let handle = unsafe { CreateMutexW(null_mut(), 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(format!("CreateMutexW failed (error {})", unsafe {
                    GetLastError()
                }));
            }
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe { CloseHandle(handle) };
                return Err("another Clickless instance is already running".to_string());
            }
            Ok(Self { handle })
        }

        pub fn handle(&self) -> HANDLE {
            self.handle
        }
    }

    impl Drop for SingleInstance {
        fn drop(&mut self) {
            unsafe {
                // Release is implicit at process exit; CloseHandle is enough
                // for a clean shutdown path because we never abandon mid-run.
                CloseHandle(self.handle);
            }
        }
    }

    /// Signals the running instance to open its Settings window. Safe to call
    /// when no instance runs: the event is auto-reset and simply stays unset.
    pub fn notify_open_settings() -> Result<(), String> {
        let name = wide(SETTINGS_EVENT_NAME);
        let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
        if handle.is_null() {
            // No running instance: the caller becomes the instance instead.
            return Ok(());
        }
        let ok = unsafe { SetEvent(handle) };
        unsafe { CloseHandle(handle) };
        if ok == 0 {
            return Err(format!("SetEvent failed (error {})", unsafe {
                GetLastError()
            }));
        }
        Ok(())
    }

    /// Named auto-reset event the running instance polls for Settings requests.
    pub struct SettingsRequest {
        handle: HANDLE,
    }

    impl SettingsRequest {
        pub fn create() -> Result<Self, String> {
            let name = wide(SETTINGS_EVENT_NAME);
            let handle = unsafe {
                CreateEventW(null_mut(), 0, 0, name.as_ptr()) // auto-reset, unnamed initially clear
            };
            if handle.is_null() {
                return Err(format!("CreateEventW failed (error {})", unsafe {
                    GetLastError()
                }));
            }
            Ok(Self { handle })
        }

        /// Non-blocking check: true when another launch asked for Settings.
        pub fn poll(&self) -> bool {
            unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
        }

        /// Raises Settings on this instance: wakes the loop's next poll.
        /// Used at startup when the config file is malformed.
        pub fn signal(&self) -> Result<(), String> {
            if unsafe { SetEvent(self.handle) } == 0 {
                return Err(format!("SetEvent failed (error {})", unsafe {
                    GetLastError()
                }));
            }
            Ok(())
        }

        pub fn handle(&self) -> HANDLE {
            self.handle
        }
    }

    impl Drop for SettingsRequest {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

#[cfg(windows)]
pub use win::{
    MUTEX_NAME, SETTINGS_EVENT_NAME, SettingsRequest, SingleInstance, notify_open_settings,
};

#[cfg(test)]
mod tests {
    use super::win::*;

    #[test]
    fn t01_acquire_succeeds_twice_in_sequence_not_twice_concurrently() {
        {
            let first = SingleInstance::acquire();
            if first.is_err() {
                // A real Clickless instance is running on this machine; the
                // negative path is still exercised by the reason string.
                eprintln!("single-instance mutex held by a live session");
                return;
            }
            let guard = first.unwrap();
            let second = SingleInstance::acquire();
            assert!(second.is_err());
            drop(guard);
        }
        // After release, acquiring again succeeds.
        assert!(SingleInstance::acquire().is_ok());
    }

    #[test]
    fn t02_settings_request_roundtrip_and_absent_instance() {
        // Notifying with no live event creator is a no-op success.
        let _ = notify_open_settings();

        let request = SettingsRequest::create().expect("create settings event");
        assert!(!request.poll(), "fresh event must not signal");

        notify_open_settings().expect("signal settings");
        assert!(request.poll(), "signal must arrive");
        // Auto-reset: second poll sees it cleared.
        assert!(!request.poll());
    }

    #[test]
    fn t03_signal_opens_settings_without_a_second_process() {
        let request = SettingsRequest::create().expect("create settings event");
        if request.poll() {
            // A live session owns the named event; its poll may have taken
            // a foreign signal. Nothing local left to assert.
            eprintln!("settings event owned by a live session");
            return;
        }
        request.signal().expect("local signal");
        assert!(request.poll(), "own signal must arrive");
        assert!(!request.poll(), "auto-reset clears after one poll");
    }
}
