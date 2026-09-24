//! Grid overlay for Windows: a transparent, click-through, topmost layered window.
//!
//! Pixel content comes from `clickless_backend_api::overlay::render_frame`, so the
//! drawing itself is covered by tests that run everywhere. This file only owns the
//! window, the DIB surface and the blit.

use clickless_backend_api::OverlayBackend;
use clickless_backend_api::overlay::{OverlayTheme, render_frame_with_theme};
use clickless_core::grid::OverlayFrame;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_TOPMOST, RegisterClassW, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

const CLASS_NAME: &str = "ClicklessGridOverlay";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
        match atom == 0 {
            false => None,
            true if unsafe { windows_sys::Win32::Foundation::GetLastError() }
                == ERROR_CLASS_ALREADY_EXISTS =>
            {
                None
            }
            true => Some("RegisterClassW failed for the overlay window".to_string()),
        }
    });
    failure.clone().map_or(Ok(()), Err)
}

/// Transparent, click-through, always-on-top overlay window.
pub struct WindowsOverlay {
    hwnd: HWND,
    visible: bool,
    theme: OverlayTheme,
}

// The window is created on the thread that runs the hook loop and Win32 delivers
// its messages to that same thread, so the handle never actually crosses threads.
// The bound exists because the hook stores the renderer as a boxed trait object.
unsafe impl Send for WindowsOverlay {}

impl WindowsOverlay {
    pub fn new() -> Result<Self, String> {
        register_class()?;
        let class_name = wide(CLASS_NAME);
        let title = wide("Clickless grid overlay");
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err("CreateWindowExW failed for the overlay window".to_string());
        }
        Ok(Self {
            hwnd,
            visible: false,
            theme: OverlayTheme::default(),
        })
    }

    /// Replaces the theme used by subsequent renders.
    pub fn with_theme(mut self, theme: OverlayTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn render(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        let Some(target) = render_frame_with_theme(frame, &self.theme) else {
            return self.hide();
        };
        let width = target.width as i32;
        let height = target.height as i32;

        unsafe {
            let screen_dc = GetDC(null_mut());
            if screen_dc.is_null() {
                return Err("GetDC failed for the overlay window".to_string());
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return Err("CreateCompatibleDC failed for the overlay window".to_string());
            }

            let mut header: BITMAPINFOHEADER = std::mem::zeroed();
            header.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            header.biWidth = width;
            header.biHeight = -height; // top-down rows
            header.biPlanes = 1;
            header.biBitCount = 32;
            header.biCompression = BI_RGB;
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = header;

            let mut bits: *mut c_void = null_mut();
            let dib = CreateDIBSection(mem_dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if dib.is_null() || bits.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err("CreateDIBSection failed for the overlay window".to_string());
            }
            let previous = SelectObject(mem_dc, dib);
            let buffer = std::slice::from_raw_parts_mut(bits as *mut u8, target.pixels.len());
            buffer.copy_from_slice(&target.pixels);

            let destination = POINT {
                x: target.origin.0 as i32,
                y: target.origin.1 as i32,
            };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                destination.x,
                destination.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let updated = UpdateLayeredWindow(
                self.hwnd,
                screen_dc,
                &destination,
                &size,
                mem_dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );

            SelectObject(mem_dc, previous);
            DeleteObject(dib);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);

            if updated == 0 {
                return Err("UpdateLayeredWindow failed for the overlay window".to_string());
            }
        }

        self.visible = true;
        Ok(())
    }
}

impl OverlayBackend for WindowsOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.render(frame)
    }

    fn hide(&mut self) -> Result<(), String> {
        unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        self.visible = false;
        Ok(())
    }
}

impl Drop for WindowsOverlay {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::grid::{OverlayCell, Rect};

    fn frame(cells: Vec<(Rect, &str)>) -> OverlayFrame {
        OverlayFrame {
            level: 1,
            cells: cells
                .into_iter()
                .map(|(rect, label)| OverlayCell {
                    rect,
                    label: label.to_string(),
                })
                .collect(),
            highlight: None,
            pointer: None,
        }
    }

    #[test]
    fn t01_window_creation_reports_success_or_reason() {
        match WindowsOverlay::new() {
            Ok(overlay) => assert!(!overlay.is_visible()),
            Err(reason) => {
                eprintln!("overlay window unavailable here: {reason}");
                assert!(!reason.is_empty());
            }
        }
    }

    #[test]
    fn t02_overlay_implements_backend_trait() {
        fn assert_backend<T: OverlayBackend>() {}
        assert_backend::<WindowsOverlay>();
    }

    #[test]
    fn t03_overlay_renders_and_hides_a_frame() {
        let Ok(mut overlay) = WindowsOverlay::new() else {
            return;
        };
        let f = frame(vec![
            (Rect::new(0, 0, 640, 360), "u"),
            (Rect::new(640, 0, 640, 360), "i"),
        ]);
        match overlay.show(&f) {
            Ok(()) => {
                assert!(overlay.is_visible());
                overlay.hide().unwrap();
                assert!(!overlay.is_visible());
            }
            Err(reason) => eprintln!("overlay render unavailable here: {reason}"),
        }
    }
}
