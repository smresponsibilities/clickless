#[cfg(windows)]
pub mod lifecycle;
#[cfg(windows)]
pub mod overlay;
pub mod scancode;
#[cfg(windows)]
pub mod settings;
#[cfg(windows)]
pub mod tray;

use clickless_backend_api::{Button, Dir, NullOverlay, OutputBackend, OverlayBackend};
use clickless_core::grid::OverlayFrame;
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Phase, StateMachine};
use std::collections::HashMap;

pub struct WindowsHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
    overlay: Box<dyn OverlayBackend + Send>,
    shown_overlay: Option<OverlayFrame>,
}

impl<O: OutputBackend> WindowsHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
        }
    }

    pub fn with_config(
        out: O,
        leader: LogicalKey,
        bindings: HashMap<LogicalKey, Action>,
        motion: MotionConfig,
    ) -> Self {
        Self {
            sm: StateMachine::with_config(leader, bindings, motion),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
        }
    }

    /// Installs the grid overlay renderer. Defaults to a no-op renderer.
    pub fn set_overlay(&mut self, overlay: Box<dyn OverlayBackend + Send>) {
        self.overlay = overlay;
        self.shown_overlay = None;
    }

    pub fn process_key(
        &mut self,
        vk: u32,
        is_down: bool,
        now_ms: u64,
    ) -> Result<Option<Action>, String> {
        let key = match scancode::vk_to_logical(vk) {
            Some(k) => k,
            None => return Ok(None),
        };
        let phase = if is_down {
            Phase::Press
        } else {
            Phase::Release
        };
        let action = self.sm.on_event(KeyEvent::new(key, phase), now_ms);
        if let Some(a) = action {
            self.execute(a)?;
        }
        self.sync_overlay()?;
        Ok(action)
    }

    /// Shows the grid overlay for the current level and hides it otherwise.
    fn sync_overlay(&mut self) -> Result<(), String> {
        let frame = self.sm.grid_overlay();
        if frame == self.shown_overlay {
            return Ok(());
        }
        match frame {
            Some(frame) => {
                self.overlay.show(&frame)?;
                self.shown_overlay = Some(frame);
            }
            None => {
                self.overlay.hide()?;
                self.shown_overlay = None;
            }
        }
        Ok(())
    }

    pub fn hide_overlay(&mut self) -> Result<(), String> {
        self.shown_overlay = None;
        self.overlay.hide()
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<(), String> {
        for (dx, dy) in self.sm.tick(dt_ms) {
            self.out.move_rel(dx as i32, dy as i32)?;
        }
        Ok(())
    }

    pub(crate) fn execute(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::ClickLeft => self.out.click(Button::Left)?,
            Action::ClickRight => self.out.click(Button::Right)?,
            Action::ScrollUp => self.out.scroll(0, 1)?,
            Action::ScrollDown => self.out.scroll(0, -1)?,
            Action::MoveTo(x, y) => self.out.move_abs(x as i32, y as i32)?,
            Action::ClickAt(x, y) => {
                self.out.move_abs(x as i32, y as i32)?;
                self.out.click(Button::Left)?;
            }
            Action::DragTo(x, y) => {
                self.out.move_abs(x as i32, y as i32)?;
                self.out.button(Button::Left, Dir::Down)?;
            }
            Action::DragEnd => self.out.button(Button::Left, Dir::Up)?,
            _ => {}
        }
        Ok(())
    }

    pub fn sm(&self) -> &StateMachine {
        &self.sm
    }

    pub fn sm_mut(&mut self) -> &mut StateMachine {
        &mut self.sm
    }

    pub fn is_intercepting(&self) -> bool {
        self.sm.layer() == clickless_core::Layer::Mouse
            || self.sm.layer() == clickless_core::Layer::Grid
    }

    /// Tray Enable/pause. Pausing forces an exit, which ends any held drag
    /// before capture stops, and hides the overlay.
    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        if let Some(action) = self.sm.set_paused(paused) {
            self.execute(action)?;
        }
        self.sync_overlay()
    }

    /// Tray Show/hide grid. Show activates the grid overlay without a leader
    /// hold; hide forces an exit back to the initial layer.
    pub fn show_grid(&mut self, show: bool) -> Result<(), String> {
        if show {
            let _ = self.sm.show_grid();
        } else {
            let _ = self.sm.force_exit();
        }
        self.sync_overlay()
    }

    pub fn out(&self) -> &O {
        &self.out
    }
}

/// Desktop runtime loop: keyboard hook, tick pacing, tray command handling and
/// the settings-request poll. `tray` is `None` for headless/CI runs.
pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    hook: WindowsHook<O>,
    mut tray: Option<crate::tray::TrayMenu>,
    mut is_running: impl FnMut() -> bool,
    mut on_settings_request: impl FnMut() -> bool,
) -> Result<(), String> {
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, PeekMessageW,
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static HOOK_PTR: AtomicPtr<WindowsHookState> = AtomicPtr::new(null_mut());

    struct WindowsHookState {
        hook: WindowsHook<Box<dyn OutputBackend + Send>>,
        start_time: Instant,
        settings_window: Option<crate::settings::win::SettingsWindow>,
    }

    impl WindowsHookState {
        fn on_open_settings(&mut self) {
            if self.settings_window.is_none() {
                match crate::settings::win::SettingsWindow::new() {
                    Ok(window) => self.settings_window = Some(window),
                    Err(e) => {
                        eprintln!("settings window unavailable: {e}");
                        return; // retried on the next request
                    }
                }
            }
            if let Some(window) = self.settings_window.as_ref() {
                window.show();
            }
        }
    }

    unsafe extern "system" fn low_level_keyboard_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 {
            let state_ptr = HOOK_PTR.load(Ordering::SeqCst);
            if !state_ptr.is_null() {
                let state = unsafe { &mut *state_ptr };
                let kbd = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
                let is_down = w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN;
                let is_up = w_param as u32 == WM_KEYUP || w_param as u32 == WM_SYSKEYUP;
                if is_down || is_up {
                    let now_ms = state.start_time.elapsed().as_millis() as u64;
                    let was_in_mouse = state.hook.is_intercepting();
                    let _ = state.hook.process_key(kbd.vkCode, is_down, now_ms);
                    let is_in_mouse = state.hook.is_intercepting();
                    if was_in_mouse || is_in_mouse {
                        return 1; // Suppress input
                    }
                }
            }
        }
        unsafe { CallNextHookEx(null_mut(), n_code, w_param, l_param) }
    }

    let out_boxed: Box<dyn OutputBackend + Send> = Box::new(hook.out);
    let mut state = WindowsHookState {
        hook: WindowsHook {
            sm: hook.sm,
            out: out_boxed,
            overlay: hook.overlay,
            shown_overlay: None,
        },
        start_time: Instant::now(),
        settings_window: None,
    };
    HOOK_PTR.store(&mut state as *mut _, Ordering::SeqCst);

    let h_hook: HHOOK = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            null_mut() as HINSTANCE,
            0,
        )
    };

    if h_hook.is_null() {
        HOOK_PTR.store(null_mut(), Ordering::SeqCst);
        return Err("Failed to install WH_KEYBOARD_LL hook".to_string());
    }

    let mut last_tick = Instant::now();
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    let mut quit_requested = false;

    while is_running() && !quit_requested {
        unsafe {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
        }
        // A second launch signalled the named event: open Settings here, on
        // the thread that owns the windows.
        if on_settings_request() {
            state.on_open_settings();
        }

        // Tray menu commands from the muda event channel.
        if let Some(tray) = tray.as_mut() {
            while let Ok(event) = crate::tray::menu_events().try_recv() {
                let command = tray.resolve(&event.id);
                match command {
                    Some(crate::tray::MenuCommand::ShowGrid) => {
                        state.hook.show_grid(true)?;
                    }
                    Some(crate::tray::MenuCommand::HideGrid) => {
                        state.hook.show_grid(false)?;
                    }
                    Some(crate::tray::MenuCommand::TogglePause) => {
                        let paused = !state.hook.sm().is_paused();
                        state.hook.set_paused(paused)?;
                        tray.set_paused(paused);
                    }
                    Some(crate::tray::MenuCommand::OpenSettings) => {
                        state.on_open_settings();
                    }
                    Some(crate::tray::MenuCommand::Quit) => {
                        // Release app-held output before the loop unwinds.
                        if let Some(action) = state.hook.sm_mut().force_exit() {
                            let _ = state.hook.execute(action);
                        }
                        let _ = state.hook.hide_overlay();
                        quit_requested = true;
                    }
                    None => {}
                }
            }
        }

        if quit_requested {
            break;
        }
        let now = Instant::now();
        let dt_ms = now.duration_since(last_tick).as_millis() as u64;
        if dt_ms >= 10 {
            let _ = state.hook.tick(dt_ms);
            last_tick = now;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let _ = state.hook.hide_overlay();
    unsafe {
        UnhookWindowsHookEx(h_hook);
    }
    HOOK_PTR.store(null_mut(), Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_backend_api::{Button, Dir, OutputBackend};

    struct MockOut {
        moves: Vec<(i32, i32)>,
        abs: Vec<(i32, i32)>,
        buttons: Vec<(Button, Dir)>,
        scrolls: Vec<(i32, i32)>,
    }

    impl MockOut {
        fn new() -> Self {
            Self {
                moves: Vec::new(),
                abs: Vec::new(),
                buttons: Vec::new(),
                scrolls: Vec::new(),
            }
        }
    }

    impl OutputBackend for MockOut {
        fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.moves.push((dx, dy));
            Ok(())
        }

        fn move_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.abs.push((x, y));
            Ok(())
        }
        fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
            self.buttons.push((b, d));
            Ok(())
        }
        fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.scrolls.push((dx, dy));
            Ok(())
        }
    }

    fn enter_mouse(hook: &mut WindowsHook<MockOut>) {
        let _ = hook.process_key(0x14, true, 0); // CapsLock press
        let _ = hook.process_key(0x14, true, 200); // poll past threshold
    }

    // tray lifecycle commands

    #[test]
    fn t20_set_paused_ends_drag_and_hides_overlay() {
        use clickless_core::grid::GridConfig;
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                drag_after_select: true,
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // grid
        hook.process_key(0x4B, true, 400).unwrap();
        hook.process_key(0x4B, true, 500).unwrap();
        hook.process_key(0x4B, false, 600).unwrap(); // drag started

        hook.set_paused(true).unwrap();
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
        assert!(hook.sm().is_paused());
        assert!(hook.sm().grid_overlay().is_none());

        hook.set_paused(false).unwrap();
        assert!(!hook.sm().is_paused());
        assert_eq!(hook.process_key(0x14, true, 700).unwrap(), None);
        assert_eq!(hook.process_key(0x14, true, 900).unwrap(), None);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t21_show_grid_shows_overlay_without_leader() {
        use clickless_core::grid::GridConfig;
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());

        hook.show_grid(true).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Grid);

        hook.show_grid(false).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t01_unmapped_vk_yields_none() {
        let mut hook = WindowsHook::new(MockOut::new());
        assert_eq!(hook.process_key(0x0D, true, 0).unwrap(), None); // VK_RETURN
    }

    #[test]
    fn t02_capslock_hold_enters_mouse_layer() {
        let mut hook = WindowsHook::new(MockOut::new());
        hook.process_key(0x14, true, 0).unwrap();
        hook.process_key(0x14, true, 200).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t03_j_in_mouse_yields_move_right() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4A, true, 300).unwrap(),
            Some(Action::MoveRight)
        );
    }

    #[test]
    fn t04_h_in_mouse_yields_move_left() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x48, true, 300).unwrap(),
            Some(Action::MoveLeft)
        );
    }

    #[test]
    fn t05_k_in_mouse_yields_move_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4B, true, 300).unwrap(),
            Some(Action::MoveUp)
        );
    }

    #[test]
    fn t06_l_in_mouse_yields_move_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4C, true, 300).unwrap(),
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn t07_f_in_mouse_yields_click_left() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x46, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t08_d_in_mouse_yields_click_right() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x44, true, 300).unwrap(),
            Some(Action::ClickRight)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Right, Dir::Down), (Button::Right, Dir::Up)]
        );
    }

    #[test]
    fn t09_w_in_mouse_yields_scroll_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x57, true, 300).unwrap(),
            Some(Action::ScrollUp)
        );
        assert_eq!(hook.out().scrolls, vec![(0, 1)]);
    }

    #[test]
    fn t10_s_in_mouse_yields_scroll_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x53, true, 300).unwrap(),
            Some(Action::ScrollDown)
        );
        assert_eq!(hook.out().scrolls, vec![(0, -1)]);
    }

    #[test]
    fn t11_tick_moves_cursor() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x4A, true, 300).unwrap(); // J
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);
    }

    #[test]
    fn t12_esc_exits_mouse_layer() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x1B, true, 300).unwrap(); // Esc
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t13_capslock_release_exits_mouse() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x14, false, 400).unwrap(); // CapsLock release
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t14_u_in_mouse_yields_speed_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x55, true, 300).unwrap(),
            Some(Action::SpeedDown)
        );
    }

    #[test]
    fn t15_o_in_mouse_yields_speed_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4F, true, 300).unwrap(),
            Some(Action::SpeedUp)
        );
    }

    #[derive(Default)]
    struct RecorderOverlay {
        shows: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
        hides: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl OverlayBackend for RecorderOverlay {
        fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
            self.shows.lock().unwrap().push(frame.level);
            Ok(())
        }

        fn hide(&mut self) -> Result<(), String> {
            *self.hides.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn t19_grid_overlay_shows_each_level_then_hides() {
        use clickless_core::grid::GridConfig;

        let shows = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let hides = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());
        hook.set_overlay(Box::new(RecorderOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        enter_mouse(&mut hook);
        assert!(shows.lock().unwrap().is_empty());

        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid level 1
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        hook.process_key(0x1B, true, 500).unwrap(); // Esc leaves grid
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn t18_grid_drag_holds_button_until_leader_release() {
        use clickless_core::grid::GridConfig;

        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                drag_after_select: true,
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );

        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x4B, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x4B, false, 600).unwrap(); // release -> drag start

        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);

        hook.process_key(0x4A, true, 700).unwrap(); // J drags right
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);

        hook.process_key(0x14, false, 800).unwrap(); // leader release
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t17_grid_release_moves_absolutely_then_left_clicks() {
        use clickless_core::grid::GridConfig;

        let mut bindings = HashMap::new();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            bindings,
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );

        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x4B, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x4B, false, 600).unwrap(); // release -> click

        assert_eq!(hook.out().abs, vec![(959, 540), (959, 540)]);
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t16_with_config_applies_custom_leader_and_bindings() {
        let mut custom_bindings = HashMap::new();
        custom_bindings.insert(LogicalKey::H, Action::ClickLeft);
        let motion = MotionConfig {
            start_speed_px_s: 500,
            max_speed_px_s: 2000,
            ramp_ms: 250,
        };
        let mut hook =
            WindowsHook::with_config(MockOut::new(), LogicalKey::Space, custom_bindings, motion);

        let _ = hook.process_key(0x20, true, 0); // Space press
        let _ = hook.process_key(0x20, true, 200); // poll past threshold
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);

        assert_eq!(
            hook.process_key(0x48, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }
}
