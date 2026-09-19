//! Minimal native Settings window.
//!
//! One window per process. Closing it hides it; the process stays in the tray.
//! Slice 1 only needs lifecycle proof, so the window carries an honest
//! read-only status text and no pretend controls. Field editors come in the
//! next slice with the real config coverage table.

#[cfg(windows)]
pub mod win {
    use std::ptr::{null, null_mut};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, DT_LEFT, DT_NOPREFIX, DT_WORDBREAK, DrawTextW, EndPaint, PAINTSTRUCT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, RegisterClassW, SW_HIDE,
        SW_SHOW, ShowWindow, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WNDCLASSW, WS_CAPTION,
        WS_EX_TOOLWINDOW, WS_MINIMIZEBOX, WS_SYSMENU,
    };

    const CLASS_NAME: &str = "ClicklessSettingsWindow";

    /// Status lines shown in the client area. Honest about slice scope: no
    /// editors exist yet, so none are drawn.
    const STATUS_TEXT: &str = "Clickless is running.\r\n\
                               Capture: enabled (pause via the tray menu)\r\n\
                               Settings editing arrives with the config coverage slice.\r\n\
                               Close this window to return to the tray.";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn register_class() -> Result<(), String> {
        static STATE: OnceLock<Option<String>> = OnceLock::new();
        let failure = STATE.get_or_init(|| {
            let class_name = wide(CLASS_NAME);
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: null_mut(),
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
            };
            let atom = unsafe { RegisterClassW(&class) };
            (atom == 0).then(|| "RegisterClassW failed for the settings window".to_string())
        });
        failure.clone().map_or(Ok(()), Err)
    }

    /// Draws STATUS_TEXT into the client area. Returns false when painting is
    /// impossible so the caller can fall back to the default procedure.
    fn paint_status(hwnd: HWND) -> bool {
        unsafe {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if hdc.is_null() {
                return false;
            }
            let mut rect: RECT = std::mem::zeroed();
            if GetClientRect(hwnd, &mut rect) == 0 {
                EndPaint(hwnd, &ps);
                return false;
            }
            // Leading margin so text does not touch the window edge.
            rect.left += 12;
            rect.top += 12;
            let mut text = wide(STATUS_TEXT);
            DrawTextW(
                hdc,
                text.as_mut_ptr(),
                (text.len() - 1) as i32,
                &mut rect,
                DT_LEFT | DT_NOPREFIX | DT_WORDBREAK,
            );
            EndPaint(hwnd, &ps);
            true
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                if paint_status(hwnd) {
                    0
                } else {
                    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                }
            }
            WM_ERASEBKGND => {
                // Fill with the default button-face color so text stays
                // legible in light and dark themes.
                0
            }
            WM_CLOSE => {
                // Close means hide: the tray keeps running.
                unsafe { ShowWindow(hwnd, SW_HIDE) };
                0
            }
            WM_DESTROY => 0,
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    /// One settings window per process. Reopening brings the same window back.
    pub struct SettingsWindow {
        hwnd: HWND,
    }

    impl SettingsWindow {
        pub fn new() -> Result<Self, String> {
            register_class()?;
            let class_name = wide(CLASS_NAME);
            let title = wide("Clickless Settings");
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                    100,
                    100,
                    420,
                    220,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                return Err("CreateWindowExW failed for the settings window".to_string());
            }
            Ok(Self { hwnd })
        }

        pub fn show(&self) {
            unsafe { ShowWindow(self.hwnd, SW_SHOW) };
        }

        pub fn hide(&self) {
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }

        pub fn is_created(&self) -> bool {
            !self.hwnd.is_null()
        }
    }

    impl Drop for SettingsWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.hwnd) };
        }
    }
}

#[cfg(windows)]
pub use win::SettingsWindow;
