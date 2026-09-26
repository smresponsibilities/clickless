//! Display enumeration for Windows (Ticket 048).
//!
//! Returns every monitor as a rect in virtual-screen coordinates, including
//! negative origins for displays left of or above the primary one, and the
//! per-monitor DPI scale the overlay needs to stay legible on mixed-DPI
//! setups. The CLI previously asked only for the main display size, so the
//! grid never knew about a second monitor at all.

use clickless_core::grid::Rect;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows_sys::core::BOOL;

/// One monitor's geometry plus its scale relative to 96 dpi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    pub rect: Rect,
    /// 1.0 at 96 dpi, 1.5 at 144 dpi, 2.0 at 192 dpi.
    pub scale_percent: u32,
}

impl Display {
    /// Effective DPI for this monitor, derived from the reported scale.
    pub fn dpi(&self) -> u32 {
        96 * self.scale_percent / 100
    }
}

// EnumDisplayMonitors passes the context pointer through to the callback.
struct EnumContext {
    displays: Vec<Display>,
}

unsafe extern "system" fn collect(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    unsafe {
        let context = &mut *(data as *mut EnumContext);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut MONITORINFO) == 0 {
            return 1;
        }
        let rc = info.monitorInfo.rcMonitor;
        let width = (rc.right - rc.left) as i64;
        let height = (rc.bottom - rc.top) as i64;
        if width <= 0 || height <= 0 {
            // A monitor reporting no area cannot host a grid.
            return 1;
        }
        // GetDpiForMonitor is the documented per-monitor scale. A failure
        // leaves dpi at 96, which is the safe default for legibility.
        let mut dpi_x: u32 = 96;
        let mut dpi_y: u32 = 96;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let dpi = if dpi_x == 0 { 96 } else { dpi_x };
        context.displays.push(Display {
            rect: Rect::new(rc.left as i64, rc.top as i64, width, height),
            scale_percent: (dpi * 100 / 96).max(100),
        });
        1
    }
}

/// Every attached monitor. Empty when enumeration fails, which the caller
/// treats as "no usable display".
pub fn displays() -> Vec<Display> {
    let mut context = EnumContext {
        displays: Vec::new(),
    };
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            Some(collect),
            &mut context as *mut EnumContext as LPARAM,
        );
    }
    context.displays
}

/// Monitors as grid rects, for the core navigator.
pub fn monitor_rects() -> Vec<Rect> {
    displays().into_iter().map(|display| display.rect).collect()
}

/// DPI of the monitor containing `(x, y)`, falling back to the desktop window
/// DPI and then to 96.
pub fn dpi_at(x: i64, y: i64) -> u32 {
    for display in displays() {
        if display.rect.contains(x, y) {
            return display.dpi();
        }
    }
    unsafe {
        let hwnd: HWND = std::ptr::null_mut();
        let dpi = GetDpiForWindow(hwnd);
        if dpi == 0 { 96 } else { dpi }
    }
}

/// Virtual-screen bounds, or a zero rect when the metrics are unavailable.
pub fn virtual_screen() -> Rect {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN) as i64;
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN) as i64;
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN) as i64;
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN) as i64;
        Rect::new(x, y, width.max(0), height.max(0))
    }
}

/// True when `monitors` differ from `previous` in a way that changes the grid
/// geometry: a different set of rects, or a different scale on any of them.
pub fn layout_changed(previous: &[Display], monitors: &[Display]) -> bool {
    previous.len() != monitors.len() || previous.iter().zip(monitors).any(|(a, b)| a != b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(x: i64, y: i64, w: i64, h: i64, scale: u32) -> Display {
        Display {
            rect: Rect::new(x, y, w, h),
            scale_percent: scale,
        }
    }

    #[test]
    fn t01_displays_are_non_empty_and_usable() {
        let found = displays();
        assert!(
            !found.is_empty(),
            "a Windows session has at least one display"
        );
        for found_one in &found {
            assert!(found_one.rect.width > 0 && found_one.rect.height > 0);
            assert!(
                found_one.scale_percent >= 100,
                "scale below 100% is not real"
            );
            assert!(found_one.dpi() >= 96);
        }
    }

    #[test]
    fn t02_monitor_rects_match_the_displays() {
        assert_eq!(monitor_rects().len(), displays().len());
    }

    #[test]
    fn t03_virtual_screen_covers_every_monitor() {
        let screen = virtual_screen();
        assert!(screen.width > 0 && screen.height > 0);
        for found_one in displays() {
            assert!(
                found_one.rect.x >= screen.x
                    && found_one.rect.y >= screen.y
                    && found_one.rect.x + found_one.rect.width <= screen.x + screen.width
                    && found_one.rect.y + found_one.rect.height <= screen.y + screen.height,
                "monitor {:?} escapes the virtual screen {:?}",
                found_one.rect,
                screen
            );
        }
    }

    #[test]
    fn t04_dpi_at_a_real_monitor_is_positive() {
        let first = displays()[0];
        assert!(dpi_at(first.rect.x + 1, first.rect.y + 1) > 0);
    }

    #[test]
    fn t05_layout_change_detects_added_removed_resized_and_rescaled() {
        let a = display(0, 0, 1920, 1080, 100);
        let b = display(1920, 0, 1280, 720, 100);
        assert!(!layout_changed(&[a], &[a]), "unchanged layout");
        assert!(layout_changed(&[a], &[a, b]), "added monitor");
        assert!(layout_changed(&[a, b], &[a]), "removed monitor");
        assert!(
            layout_changed(&[a], &[display(0, 0, 2560, 1440, 100)]),
            "resized monitor"
        );
        assert!(
            layout_changed(&[a], &[display(0, 0, 1920, 1080, 150)]),
            "changed scale on a mixed-DPI change"
        );
        // A monitor to the left of the primary has a negative origin and must
        // still register as a layout change when it appears.
        assert!(layout_changed(
            &[a],
            &[a, display(-1280, 0, 1280, 720, 100)]
        ));
    }
}
