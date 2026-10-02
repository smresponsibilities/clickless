#[cfg(not(feature = "winui3"))]
mod legacy {
    // First-run practice dialog (Ticket 030): native window over the pure
    // `crate::practice` flow. The dialog feeds dialog-local keys to a real
    // StateMachine while global capture is suspended; it owns no output
    // backend, so practice can never click. Esc always exits, focus loss
    // cancels, Finish persists only the completion version.

    use crate::overlay::WindowsOverlay;
    use crate::practice::{PRACTICE_VERSION, Practice, Step};
    use crate::scancode::vk_to_logical;
    use crate::window_style::{font, scale};
    use clickless_backend_api::OverlayBackend;
    use clickless_core::{KeyEvent, Phase};
    use std::cell::RefCell;
    use std::ptr::null_mut;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

    use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, GetDlgItem,
        GetForegroundWindow, IMAGE_ICON, IsChild, IsDialogMessageW, LR_DEFAULTSIZE, LR_SHARED,
        LoadImageW, MSG, RegisterClassW, SW_RESTORE, SetForegroundWindow, SetTimer, SetWindowTextW,
        ShowWindow, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
        WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_SYSMENU,
        WS_TABSTOP, WS_VISIBLE,
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
        static COMPLETED_LEADER: std::cell::Cell<Option<clickless_core::LogicalKey>> = const { std::cell::Cell::new(None) };
        static PRACTICE: RefCell<Option<Practice>> = const { RefCell::new(None) };
        static GRID_OVERLAY: RefCell<Option<WindowsOverlay>> = const { RefCell::new(None) };
        static START: RefCell<Instant> = RefCell::new(Instant::now());
        static PREVIOUS_FOCUS: RefCell<Option<HWND>> = const { RefCell::new(None) };
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn hide_dialog(hwnd: HWND) {
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
            ShowWindow(hwnd, SW_HIDE);
            PREVIOUS_FOCUS.with(|cell| {
                if let Some(hwnd) = *cell.borrow_mut()
                    && !hwnd.is_null()
                {
                    SetForegroundWindow(hwnd);
                }
            });
        }
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

    fn key_name(key: clickless_core::LogicalKey) -> &'static str {
        use clickless_core::LogicalKey;
        match key {
            LogicalKey::CapsLock => "CapsLock",
            LogicalKey::Space => "Space",
            LogicalKey::ControlLeft => "Left Ctrl",
            LogicalKey::ShiftLeft => "Left Shift",
            _ => "Unknown",
        }
    }

    /// Builds the flow from the saved setup. First run (never completed) starts
    /// with the activation-key chooser; later runs teach the saved leader and
    /// grid style instead of a hard-coded CapsLock lesson.
    fn new_practice() -> Practice {
        let saved = crate::settings_process::config_path()
            .ok()
            .and_then(|path| clickless_config::Config::load_from_file(&path).ok());
        match saved {
            Some(config) if config.practice_completed_version > 0 => {
                Practice::with_setup(config.settings.leader, config.grid.dense)
            }
            _ => Practice::new(),
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
                        // The instruction and status come from the pure lesson
                        // module, which derives the key list from the live
                        // overlay, so the copy cannot drift from what is shown.
                        set_text(hwnd, ID_STEP, &practice.instruction());
                        set_text(hwnd, ID_STATUS, &practice.status());
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
                    *cell.borrow_mut() = Some(new_practice());
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

    pub(crate) fn take_completed_leader() -> Option<clickless_core::LogicalKey> {
        COMPLETED_LEADER.with(|leader| leader.take())
    }

    unsafe fn persist_completion(hwnd: HWND) -> bool {
        unsafe {
            let message = match crate::settings_process::config_path() {
                Err(e) => format!("config path unavailable: {e}"),
                Ok(path) => {
                    let chosen =
                        PRACTICE.with(|cell| cell.borrow().as_ref().map(Practice::chosen_leader));
                    let mut config = if path.exists() {
                        match clickless_config::Config::load_from_file(&path) {
                            Ok(config) => config,
                            Err(error) => {
                                set_text(
                                    hwnd,
                                    ID_STATUS,
                                    &format!("Could not save practice: {error}"),
                                );
                                return false;
                            }
                        }
                    } else {
                        clickless_config::Config::default()
                    };
                    if let Some(leader) = chosen {
                        config.settings.leader = leader;
                    }
                    config.practice_completed_version = PRACTICE_VERSION;
                    match config.validate() {
                        Err(e) => format!("could not save activation key: {e}"),
                        Ok(()) => match config.save_to_file(&path) {
                            Ok(()) => {
                                COMPLETED_LEADER
                                    .with(|leader| leader.set(Some(config.settings.leader)));
                                format!(
                                    "Practice complete. Activation key: {}.",
                                    key_name(config.settings.leader)
                                )
                            }
                            Err(e) => format!("could not save completion: {e}"),
                        },
                    }
                }
            };
            set_text(hwnd, ID_STATUS, &message);
            COMPLETED_LEADER.with(|leader| leader.get().is_some())
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
                windows_sys::Win32::UI::WindowsAndMessaging::WM_DRAWITEM => {
                    crate::window_style::draw_button(lparam)
                }
                windows_sys::Win32::UI::WindowsAndMessaging::WM_CTLCOLORSTATIC => {
                    crate::window_style::label_background(wparam)
                }
                WM_TIMER => {
                    sync_grid_overlay();
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
                    if IsChild(hwnd, wparam as HWND) != 0 {
                        return 0;
                    }
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
                            PRACTICE.with(|cell| *cell.borrow_mut() = Some(new_practice()));
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
                            hide_dialog(hwnd);
                        }
                        ID_FINISH if persist_completion(hwnd) => hide_dialog(hwnd),
                        _ => {}
                    }
                    0
                }
                WM_CLOSE => {
                    hide_dialog(hwnd);
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
            let class_name = wide(CLASS_NAME);
            let title = wide("Clickless Practice");
            static REGISTER: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
            REGISTER
                .get_or_init(|| unsafe {
                    let icon = LoadImageW(
                        windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                            std::ptr::null(),
                        ),
                        101 as *const _, // Use resource ID 101 directly
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
                        hbrBackground: crate::window_style::background(),
                        lpszMenuName: null_mut(),
                        lpszClassName: class_name.as_ptr(),
                    };
                    if RegisterClassW(&class) == 0 {
                        return Err("RegisterClassW failed for the practice window".to_string());
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
                    140,
                    140,
                    scale(760),
                    scale(540),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                return Err("CreateWindowExW failed for the practice window".to_string());
            }
            crate::window_style::dark_caption(hwnd);
            GRID_OVERLAY.with(|cell| {
                *cell.borrow_mut() = WindowsOverlay::new().ok();
            });
            unsafe { SetTimer(hwnd, 1, 50, None) };
            unsafe {
                let mut y = 28;
                let statics = [
                    ("Practice", ID_STEP, 220),
                    (
                        "Learn your activation key and grid targeting. Practice never moves or clicks the real pointer. Select Start to begin; Esc exits.",
                        ID_STATUS,
                        120,
                    ),
                ];
                for (text, id, h) in statics {
                    let label = CreateWindowExW(
                        0,
                        wide("STATIC").as_ptr(),
                        wide(text).as_ptr(),
                        WS_CHILD | WS_VISIBLE,
                        scale(28),
                        scale(y),
                        scale(688),
                        scale(h),
                        hwnd,
                        id as _,
                        null_mut(),
                        null_mut(),
                    );
                    font(label, false);
                    y += h + 16;
                }
                let buttons = [
                    ("Start", ID_START),
                    ("Skip", ID_SKIP),
                    ("Finish", ID_FINISH),
                ];
                let mut x = 28;
                for (text, id) in buttons {
                    let button = CreateWindowExW(
                        0,
                        wide("BUTTON").as_ptr(),
                        wide(text).as_ptr(),
                        WS_CHILD
                            | WS_VISIBLE
                            | WS_TABSTOP
                            | windows_sys::Win32::UI::WindowsAndMessaging::BS_OWNERDRAW as u32,
                        scale(x),
                        scale(y),
                        scale(216),
                        scale(40),
                        hwnd,
                        id as _,
                        null_mut(),
                        null_mut(),
                    );
                    font(button, false);
                    if id == ID_FINISH && !button.is_null() {
                        EnableWindow(button, 0);
                    }
                    x += 236;
                }
            }
            Ok(Self { hwnd })
        }

        pub fn show(&self) {
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;
                PREVIOUS_FOCUS.with(|cell| {
                    *cell.borrow_mut() = Some(GetForegroundWindow());
                });
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
                ShowWindow(self.hwnd, SW_HIDE);
                // Return focus to the previous window
                PREVIOUS_FOCUS.with(|cell| {
                    if let Some(hwnd) = *cell.borrow_mut()
                        && !hwnd.is_null()
                    {
                        SetForegroundWindow(hwnd);
                    }
                });
            };
        }

        pub fn has_focus(&self) -> bool {
            unsafe { GetForegroundWindow() == self.hwnd }
        }

        pub fn translate_message(&self, msg: &MSG) -> bool {
            if msg.hwnd != self.hwnd && unsafe { IsChild(self.hwnd, msg.hwnd) } == 0 {
                return false;
            }
            // IsDialogMessageW routes key messages to the focused child control.
            // Practice owns the keyboard flow, so intercept keys before the
            // dialog manager can swallow J/H/K/L or CapsLock.
            let key_message = matches!(
                msg.message,
                WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
            );
            let lesson_active = PRACTICE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|practice| !practice.cancelled() && practice.step() != Step::Done)
            });
            if key_message
                && (msg.wParam == 0x1B
                    || (lesson_active && msg.wParam != 0x09 && msg.wParam != 0x0D))
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
            PRACTICE.with(|cell| *cell.borrow_mut() = None);
            unsafe {
                set_text(self.hwnd, ID_STEP, "Practice");
                set_text(
                    self.hwnd,
                    ID_STATUS,
                    "Learn your activation key and grid targeting. Practice never moves or clicks the real pointer. Select Start to begin; Esc exits.",
                );
                EnableWindow(GetDlgItem(self.hwnd, ID_FINISH), 0);
            }
            sync_grid_overlay();
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
}
#[cfg(feature = "winui3")]
pub use crate::winui_windows::PracticeWindow;
#[cfg(feature = "winui3")]
pub(crate) use crate::winui_windows::{PRACTICE_OPEN_REQUEST, take_completed_leader};
#[cfg(not(feature = "winui3"))]
pub use legacy::*;
