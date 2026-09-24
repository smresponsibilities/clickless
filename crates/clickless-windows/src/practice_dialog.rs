//! First-run practice dialog (Ticket 030): native window over the pure
//! `crate::practice` flow. The dialog feeds dialog-local keys to a real
//! StateMachine while global capture is suspended; it owns no output
//! backend, so practice can never click. Esc always exits, focus loss
//! cancels, Finish persists only the completion version.

use crate::overlay::WindowsOverlay;
use crate::practice::{PRACTICE_VERSION, Practice, Step};
use crate::scancode::vk_to_logical;
use clickless_backend_api::OverlayBackend;
use clickless_core::{KeyEvent, Phase};
use std::cell::RefCell;
use std::ptr::null_mut;
use std::time::Instant;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardState, SetKeyboardState,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, GetDlgItem,
    GetForegroundWindow, IsDialogMessageW, MSG, RegisterClassW, SW_RESTORE, SetForegroundWindow,
    SetTimer, SetWindowTextW, ShowWindow, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_KEYUP,
    WM_KILLFOCUS, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

const CLASS_NAME: &str = "ClicklessPracticeWindow";

const ID_STEP: i32 = 301;
const ID_STATUS: i32 = 302;
const ID_START: i32 = 303;
const ID_SKIP: i32 = 304;
const ID_FINISH: i32 = 305;

/// Set by the Settings window; the event loop opens practice on next drain.
pub(crate) static PRACTICE_OPEN_REQUEST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static PRACTICE: RefCell<Option<Practice>> = const { RefCell::new(None) };
    static GRID_OVERLAY: RefCell<Option<WindowsOverlay>> = const { RefCell::new(None) };
    static START: RefCell<Instant> = RefCell::new(Instant::now());
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn sync_grid_overlay() {
    GRID_OVERLAY.with(|cell| {
        let mut overlay = cell.borrow_mut();
        let Some(overlay) = overlay.as_mut() else {
            return;
        };
        PRACTICE.with(|practice| {
            let frame = practice.borrow().as_ref().and_then(Practice::grid_overlay);
            match frame {
                Some(frame) => {
                    let _ = overlay.show(&frame);
                }
                None => {
                    let _ = overlay.hide();
                }
            }
        });
    });
}

fn clear_capslock_toggle() {
    unsafe {
        let mut state = [0u8; 256];
        if GetKeyboardState(state.as_mut_ptr()) != 0 {
            state[0x14] &= 0x7f;
            let _ = SetKeyboardState(state.as_ptr());
        }
    }
}

fn step_text(step: Step) -> &'static str {
    match step {
        Step::HoldLeader => "Step 1 of 2: hold CapsLock to enter pointer mode.",
        Step::GridPick => "Step 2 of 2: choose one outer cell, then one inner cell to click.",
        Step::Done => "Done. Finish saves completion and closes.",
    }
}

unsafe fn set_text(hwnd: HWND, id: i32, text: &str) {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem;
        let control = GetDlgItem(hwnd, id);
        if !control.is_null() {
            SetWindowTextW(control, wide(text).as_ptr());
        }
    }
}

unsafe fn refresh(hwnd: HWND) {
    unsafe {
        PRACTICE.with(|cell| {
            let practice = cell.borrow();
            match practice.as_ref() {
                None => {
                    set_text(hwnd, ID_STEP, "Press Start, or Skip to leave.");
                    set_text(hwnd, ID_STATUS, "");
                }
                Some(practice) if practice.cancelled() => {
                    set_text(hwnd, ID_STEP, "Practice cancelled.");
                    set_text(hwnd, ID_STATUS, "Start begins again; Esc closes.");
                }
                Some(practice) => {
                    set_text(hwnd, ID_STEP, step_text(practice.step()));
                    let status = practice
                        .grid_overlay()
                        .map(|frame| {
                            let labels = frame
                                .cells
                                .iter()
                                .map(|cell| cell.label.as_str())
                                .take(30)
                                .collect::<Vec<_>>();
                            format!("Grid open. Choose a grid label:\n{}", labels.join("  "))
                        })
                        .unwrap_or_else(|| {
                            format!("Nested targets so far: {}", practice.nested_count())
                        });
                    set_text(hwnd, ID_STATUS, &status);
                    let done = practice.step() == Step::Done;
                    let finish = GetDlgItem(hwnd, ID_FINISH);
                    if !finish.is_null() {
                        EnableWindow(finish, done as i32);
                    }
                }
            }
        });
    }
}

unsafe fn feed_key(hwnd: HWND, vk: u32, press: bool) {
    unsafe {
        let Some(key) = vk_to_logical(vk) else {
            return;
        };
        let now = START.with(|start| start.borrow().elapsed().as_millis() as u64);
        PRACTICE.with(|cell| {
            if cell.borrow().is_none() {
                *cell.borrow_mut() = Some(Practice::new());
                START.with(|start| *start.borrow_mut() = Instant::now());
            }
            if let Some(practice) = cell.borrow_mut().as_mut() {
                practice.key(
                    KeyEvent::new(key, if press { Phase::Press } else { Phase::Release }),
                    now,
                );
            }
        });
        refresh(hwnd);
        sync_grid_overlay();
        if vk == 0x1B && press {
            // Esc always exits, after cancelling through the flow above.
            ShowWindow(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE);
        }
    }
}

unsafe fn persist_completion(hwnd: HWND) {
    unsafe {
        let message = match clickless_config::default_config_path() {
            Err(e) => format!("config path unavailable: {e}"),
            Ok(path) => {
                let mut config =
                    clickless_config::Config::load_from_file(&path).unwrap_or_default();
                config.practice_completed_version = PRACTICE_VERSION;
                match config.save_to_file(&path) {
                    Ok(()) => "Practice complete.".to_string(),
                    Err(e) => format!("could not save completion: {e}"),
                }
            }
        };
        set_text(hwnd, ID_STATUS, &message);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                clear_capslock_toggle();
                if (GetAsyncKeyState(0x14) & (0x8000u16 as i16)) == 0 {
                    sync_grid_overlay();
                }
                0
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                feed_key(hwnd, wparam as u32, true);
                0
            }
            WM_KEYUP | WM_SYSKEYUP => {
                feed_key(hwnd, wparam as u32, false);
                0
            }
            WM_KILLFOCUS => {
                PRACTICE.with(|cell| {
                    if let Some(practice) = cell.borrow_mut().as_mut()
                        && !practice.cancelled()
                        && practice.step() != Step::Done
                    {
                        practice.cancel();
                    }
                });
                refresh(hwnd);
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as i32;
                match id {
                    ID_START => {
                        PRACTICE.with(|cell| *cell.borrow_mut() = Some(Practice::new()));
                        START.with(|start| *start.borrow_mut() = Instant::now());
                        SetFocus(hwnd);
                        refresh(hwnd);
                    }
                    ID_SKIP => {
                        PRACTICE.with(|cell| {
                            if let Some(practice) = cell.borrow_mut().as_mut() {
                                practice.cancel();
                            }
                        });
                        use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
                        ShowWindow(hwnd, SW_HIDE);
                    }
                    ID_FINISH => {
                        persist_completion(hwnd);
                        use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
                        ShowWindow(hwnd, SW_HIDE);
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
                ShowWindow(hwnd, SW_HIDE);
                0
            }
            WM_DESTROY => 0,
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub struct PracticeWindow {
    hwnd: HWND,
}

impl PracticeWindow {
    pub fn new() -> Result<Self, String> {
        static REGISTER: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
        REGISTER
            .get_or_init(|| unsafe {
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
                    lpszMenuName: null_mut(),
                    lpszClassName: class_name.as_ptr(),
                };
                if RegisterClassW(&class) == 0 {
                    return Err("RegisterClassW failed for the practice window".to_string());
                }
                Ok(())
            })
            .clone()?;
        let class_name = wide(CLASS_NAME);
        let title = wide("Clickless Practice");
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                140,
                140,
                620,
                360,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err("CreateWindowExW failed for the practice window".to_string());
        }
        GRID_OVERLAY.with(|cell| {
            *cell.borrow_mut() = WindowsOverlay::new().ok();
        });
        unsafe { SetTimer(hwnd, 1, 50, None) };
        unsafe {
            let mut y = 24;
            let statics = [
                ("Practice", ID_STEP, 72),
                (
                    "Learn the activation key, then choose a grid label.",
                    ID_STATUS,
                    120,
                ),
            ];
            for (text, id, h) in statics {
                CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    y,
                    560,
                    h,
                    hwnd,
                    id as _,
                    null_mut(),
                    null_mut(),
                );
                y += h + 8;
            }
            let buttons = [
                ("Start", ID_START),
                ("Skip", ID_SKIP),
                ("Finish", ID_FINISH),
            ];
            let mut x = 12;
            for (text, id) in buttons {
                let button = CreateWindowExW(
                    0,
                    wide("BUTTON").as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
                    x,
                    y,
                    156,
                    36,
                    hwnd,
                    id as _,
                    null_mut(),
                    null_mut(),
                );
                if id == ID_FINISH && !button.is_null() {
                    EnableWindow(button, 0);
                }
                x += 170;
            }
        }
        Ok(Self { hwnd })
    }

    pub fn show(&self) {
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;
            ShowWindow(self.hwnd, SW_RESTORE);
            ShowWindow(self.hwnd, SW_SHOW);
            BringWindowToTop(self.hwnd);
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
        };
    }

    pub fn hide(&self) {
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
            ShowWindow(self.hwnd, SW_HIDE)
        };
    }

    pub fn has_focus(&self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd }
    }

    pub fn translate_message(&self, msg: &MSG) -> bool {
        // IsDialogMessageW routes key messages to the focused child control.
        // Practice owns the keyboard flow, so intercept keys before the
        // dialog manager can swallow J/H/K/L or CapsLock.
        if msg.message == WM_KEYDOWN
            || msg.message == WM_KEYUP
            || msg.message == WM_SYSKEYDOWN
            || msg.message == WM_SYSKEYUP
        {
            unsafe {
                feed_key(
                    self.hwnd,
                    msg.wParam as u32,
                    msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN,
                )
            };
            true
        } else {
            unsafe { IsDialogMessageW(self.hwnd, msg) != 0 }
        }
    }

    pub fn is_created(&self) -> bool {
        !self.hwnd.is_null()
    }

    /// Reset the practice state so "Practice again" starts fresh.
    pub fn reset_state(&self) {
        use crate::practice_dialog::PRACTICE;
        PRACTICE.with(|cell| *cell.borrow_mut() = None);
    }
}

impl Drop for PracticeWindow {
    fn drop(&mut self) {
        GRID_OVERLAY.with(|cell| {
            if let Some(overlay) = cell.borrow_mut().as_mut() {
                let _ = overlay.hide();
            }
        });
        unsafe { DestroyWindow(self.hwnd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem;

    #[test]
    fn practice_window_creates_with_start_skip_finish() {
        let window = PracticeWindow::new().expect("create practice window");
        assert!(window.is_created());
        unsafe {
            for id in [ID_START, ID_SKIP, ID_FINISH] {
                assert!(
                    !GetDlgItem(window.hwnd, id).is_null(),
                    "practice button {id} exists"
                );
            }
        }
    }
}
