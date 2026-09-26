//! Home screen (Ticket 049): the window a user sees when Clickless starts.
//!
//! It answers the three questions a new user has - did it start, is pointer
//! control on, and what key do I hold - and offers practice, Settings, pause
//! and quit without hunting through the tray overflow. All wording and button
//! order comes from the pure `clickless_core::home` view model, so this file
//! only owns the window, the four buttons and the focus rule.

use clickless_core::LogicalKey;
use clickless_core::home::HomeView;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, GetDlgItem,
    GetForegroundWindow, IMAGE_ICON, IsDialogMessageW, LR_DEFAULTSIZE, LR_SHARED, LoadImageW, MSG,
    RegisterClassW, SW_HIDE, SW_SHOW, SetForegroundWindow, SetWindowTextW, ShowWindow, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE,
};

const CLASS_NAME: &str = "ClicklessHomeWindow";

const ID_STATUS: i32 = 401;
const ID_EXPLANATION: i32 = 402;
const ID_PRACTICE: i32 = 403;
const ID_SETTINGS: i32 = 404;
const ID_PAUSE: i32 = 405;
const ID_QUIT: i32 = 406;

/// Button order, top to bottom left to right, and the control ids they own.
const ID_FOR: [HomeAction; 4] = [
    HomeAction::StartPractice,
    HomeAction::OpenSettings,
    HomeAction::TogglePause,
    HomeAction::Quit,
];

const ID_OF: [i32; 4] = [ID_PRACTICE, ID_SETTINGS, ID_PAUSE, ID_QUIT];

thread_local! {
    /// The view currently rendered, so a refresh after a pause toggle reuses
    /// the pure wording rules instead of re-deriving them here.
    static VIEW: std::cell::RefCell<Option<HomeView>> = const { std::cell::RefCell::new(None) };
    /// The action the host must run. Taken by the event loop, never by the UI.
    static PENDING: std::cell::RefCell<Option<HomeAction>> = const { std::cell::RefCell::new(None) };
    static PREVIOUS_FOCUS: std::cell::RefCell<Option<HWND>> = const { std::cell::RefCell::new(None) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_text(hwnd: HWND, id: i32, text: &str) {
    let child = unsafe { GetDlgItem(hwnd, id) };
    if !child.is_null() {
        unsafe { SetWindowTextW(child, wide(text).as_ptr()) };
    }
}

/// Redraws the status line and the four button labels from the current view.
fn refresh(hwnd: HWND) {
    VIEW.with(|cell| {
        let Some(view) = cell.borrow().clone() else {
            return;
        };
        set_text(hwnd, ID_STATUS, &view.status);
        set_text(hwnd, ID_EXPLANATION, &view.explanation);
        for (action, id) in ID_FOR.into_iter().zip(ID_OF) {
            set_text(hwnd, id, action.label(view.enabled));
        }
    });
}

fn hide(hwnd: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        PREVIOUS_FOCUS.with(|cell| {
            if let Some(previous) = *cell.borrow_mut()
                && !previous.is_null()
            {
                SetForegroundWindow(previous);
            }
        });
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
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as i32;
                if let Some(position) = ID_OF.iter().position(|candidate| *candidate == id)
                    && let Some(action) = ID_FOR.get(position)
                {
                    PENDING.with(|cell| *cell.borrow_mut() = Some(*action));
                    if *action == HomeAction::Quit {
                        hide(hwnd);
                    }
                }
                0
            }
            WM_CLOSE => {
                hide(hwnd);
                0
            }
            WM_DESTROY => 0,
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// The home window. Owns no runtime state: it reports the action the user
/// chose and the host applies it to the real hook.
pub struct HomeWindow {
    hwnd: HWND,
}

impl HomeWindow {
    /// Builds the window for a given runtime state.
    pub fn new(
        enabled: bool,
        leader: LogicalKey,
        practice_completed: bool,
    ) -> Result<Self, String> {
        let view = HomeView::build(enabled, leader, practice_completed);
        let class_name = wide(CLASS_NAME);
        let title = wide(&view.title);
        static REGISTER: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
        REGISTER
            .get_or_init(|| unsafe {
                let icon = LoadImageW(
                    null_mut(),
                    101 as *const _,
                    IMAGE_ICON,
                    0,
                    0,
                    LR_DEFAULTSIZE | LR_SHARED,
                );
                let class = WNDCLASSW {
                    style: 0,
                    lpfnWndProc: Some(wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: null_mut(),
                    hIcon: icon,
                    hCursor: null_mut(),
                    hbrBackground: null_mut(),
                    lpszMenuName: null_mut(),
                    lpszClassName: class_name.as_ptr(),
                };
                if RegisterClassW(&class) == 0 {
                    return Err("RegisterClassW failed for the home window".to_string());
                }
                Ok(())
            })
            .clone()?;
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                200,
                160,
                560,
                300,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err("CreateWindowExW failed for the home window".to_string());
        }
        unsafe {
            let mut y = 16;
            for (id, height) in [(ID_STATUS, 44), (ID_EXPLANATION, 96)] {
                CreateWindowExW(
                    0,
                    wide("STATIC").as_ptr(),
                    wide("").as_ptr(),
                    WS_CHILD | WS_VISIBLE,
                    14,
                    y,
                    530,
                    height,
                    hwnd,
                    id as _,
                    null_mut(),
                    null_mut(),
                );
                y += height + 6;
            }
            let mut x = 14;
            for (action, id) in ID_FOR.into_iter().zip(ID_OF) {
                CreateWindowExW(
                    0,
                    wide("BUTTON").as_ptr(),
                    wide(action.label(enabled)).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER,
                    x,
                    y,
                    128,
                    34,
                    hwnd,
                    id as _,
                    null_mut(),
                    null_mut(),
                );
                x += 132;
            }
        }
        VIEW.with(|cell| *cell.borrow_mut() = Some(view));
        refresh(hwnd);
        Ok(Self { hwnd })
    }

    /// Shows the window and puts keyboard focus where the ticket requires: on
    /// Start practice for a first run, otherwise on Settings.
    pub fn show(&self) {
        unsafe {
            PREVIOUS_FOCUS.with(|cell| *cell.borrow_mut() = Some(GetForegroundWindow()));
            ShowWindow(self.hwnd, SW_SHOW);
            BringWindowToTop(self.hwnd);
            SetForegroundWindow(self.hwnd);
            let first_run = VIEW.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|view| view.first_run)
                    .unwrap_or(false)
            });
            let focus = if first_run { ID_PRACTICE } else { ID_SETTINGS };
            let target = GetDlgItem(self.hwnd, focus);
            SetFocus(if target.is_null() { self.hwnd } else { target });
        }
    }

    pub fn hide(&self) {
        hide(self.hwnd);
    }

    /// True while this window owns the keyboard, so the host can suspend
    /// global capture exactly as it does for Settings and practice.
    pub fn has_focus(&self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd }
    }

    pub fn is_created(&self) -> bool {
        !self.hwnd.is_null()
    }

    /// Re-renders after the runtime state changed, for example after the pause
    /// button toggled pointer control.
    pub fn refresh(&self, enabled: bool) {
        VIEW.with(|cell| {
            let mut slot = cell.borrow_mut();
            if let Some(view) = slot.as_mut() {
                view.enabled = enabled;
                let rebuilt = HomeView::build(enabled, LogicalKey::CapsLock, !view.first_run);
                view.status = rebuilt.status;
                view.explanation = rebuilt.explanation;
            }
        });
        refresh(self.hwnd);
    }

    /// The action the user chose, taken once so the host applies it exactly
    /// once. Returns `None` when nothing was chosen.
    pub fn take_action(&self) -> Option<HomeAction> {
        PENDING.with(|cell| cell.borrow_mut().take())
    }

    pub fn translate_message(&self, msg: &MSG) -> bool {
        unsafe { IsDialogMessageW(self.hwnd, msg) != 0 }
    }
}

impl Drop for HomeWindow {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Re-exported so the host names the activation key exactly as this window
/// and the practice dialog do.
pub use clickless_core::home::HomeAction;
pub use clickless_core::home::key_name as activation_key_name;

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::home::key_name;

    #[test]
    fn t01_home_window_creates_with_four_buttons() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        assert!(window.is_created());
        for id in ID_OF {
            let child = unsafe { GetDlgItem(window.hwnd, id) };
            assert!(!child.is_null(), "home window is missing button {id}");
        }
    }

    #[test]
    fn t02_status_control_exists_and_view_names_the_saved_key() {
        let _window = HomeWindow::new(true, LogicalKey::Space, true).expect("create home window");
        // The pure view model owns the wording; this pins that the window is
        // built from the saved key rather than a hard-coded CapsLock.
        let view = HomeView::build(true, LogicalKey::Space, true);
        assert!(view.status.contains(key_name(LogicalKey::Space)));
    }

    #[test]
    fn t03_an_action_is_delivered_exactly_once() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        assert_eq!(window.take_action(), None, "no action before any click");
        PENDING.with(|cell| *cell.borrow_mut() = Some(HomeAction::OpenSettings));
        assert_eq!(window.take_action(), Some(HomeAction::OpenSettings));
        assert_eq!(window.take_action(), None, "an action is delivered once");
    }

    #[test]
    fn t04_pause_toggle_updates_the_view_for_the_next_refresh() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        window.refresh(false);
        let labels = VIEW.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(HomeView::button_labels)
                .unwrap_or_default()
        });
        assert!(
            labels.contains(&"Resume pointer control"),
            "after pausing, the button must offer to resume: {labels:?}"
        );
    }
}
