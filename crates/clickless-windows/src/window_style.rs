//! Shared native typography and DPI scale for home and practice.
#[cfg(not(feature = "winui3"))]
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::HWND;
#[cfg(not(feature = "winui3"))]
use windows_sys::Win32::Graphics::Gdi::{CreateFontW, DEFAULT_CHARSET, DEFAULT_QUALITY};
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
#[cfg(not(feature = "winui3"))]
use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SETFONT};

pub(crate) fn scale(value: i32) -> i32 {
    value * unsafe { GetDpiForSystem() }.max(96) as i32 / 96
}

#[cfg(not(feature = "winui3"))]
pub(crate) fn font(hwnd: HWND, heading: bool) {
    static BODY: OnceLock<usize> = OnceLock::new();
    static HEADING: OnceLock<usize> = OnceLock::new();
    let cache = if heading { &HEADING } else { &BODY };
    let font = *cache.get_or_init(|| {
        let face: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
        unsafe {
            CreateFontW(
                -scale(if heading { 28 } else { 16 }),
                0,
                0,
                0,
                if heading { 600 } else { 400 },
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                0,
                0,
                DEFAULT_QUALITY as u32,
                0,
                face.as_ptr(),
            ) as usize
        }
    });
    if font != 0 {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, font, 1);
        }
    }
}

#[cfg(not(feature = "winui3"))]
pub(crate) fn background() -> windows_sys::Win32::Graphics::Gdi::HBRUSH {
    static BRUSH: OnceLock<usize> = OnceLock::new();
    *BRUSH.get_or_init(|| unsafe {
        windows_sys::Win32::Graphics::Gdi::CreateSolidBrush(0x00202020) as usize
    }) as _
}

pub(crate) fn dark_caption(hwnd: HWND) {
    let enabled = 1i32;
    unsafe {
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            20,
            &enabled as *const _ as _,
            4,
        );
    }
}

#[cfg(not(feature = "winui3"))]
pub(crate) unsafe fn label_background(wparam: usize) -> isize {
    unsafe {
        windows_sys::Win32::Graphics::Gdi::SetBkMode(
            wparam as _,
            windows_sys::Win32::Graphics::Gdi::TRANSPARENT as i32,
        );
        windows_sys::Win32::Graphics::Gdi::SetTextColor(wparam as _, 0x00F2F2F2);
        background() as isize
    }
}

/// Native buttons retain keyboard and accessibility semantics; only their surface is drawn.
#[cfg(not(feature = "winui3"))]
pub(crate) unsafe fn draw_button(lparam: isize) -> isize {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::Controls::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    unsafe {
        let draw = &*(lparam as *const DRAWITEMSTRUCT);
        let pressed = draw.itemState & ODS_SELECTED != 0;
        let disabled = draw.itemState & ODS_DISABLED != 0;
        let color = if pressed { 0x00505050 } else { 0x00363636 };
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(draw.hDC, brush as _);
        let old_pen = SelectObject(draw.hDC, GetStockObject(NULL_PEN) as _);
        FillRect(draw.hDC, &draw.rcItem, background());
        RoundRect(
            draw.hDC,
            draw.rcItem.left,
            draw.rcItem.top,
            draw.rcItem.right,
            draw.rcItem.bottom,
            scale(12),
            scale(12),
        );
        SelectObject(draw.hDC, old_brush);
        SelectObject(draw.hDC, old_pen);
        DeleteObject(brush as _);
        SetBkMode(draw.hDC, TRANSPARENT as i32);
        SetTextColor(draw.hDC, if disabled { 0x00888888 } else { 0x00F2F2F2 });
        let control_font = SendMessageW(
            draw.hwndItem,
            windows_sys::Win32::UI::WindowsAndMessaging::WM_GETFONT,
            0,
            0,
        );
        let old_font = SelectObject(draw.hDC, control_font as _);
        let mut text = [0u16; 256];
        let length = GetWindowTextW(draw.hwndItem, text.as_mut_ptr(), text.len() as i32);
        let mut rect = draw.rcItem;
        DrawTextW(
            draw.hDC,
            text.as_ptr(),
            length,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(draw.hDC, old_font);
        if draw.itemState & ODS_FOCUS != 0 {
            rect.left += 4;
            rect.top += 4;
            rect.right -= 4;
            rect.bottom -= 4;
            DrawFocusRect(draw.hDC, &rect);
        }
        1
    }
}
