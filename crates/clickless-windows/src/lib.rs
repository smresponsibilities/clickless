pub mod scancode;

use clickless_backend_api::{Button, OutputBackend};
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Phase, StateMachine};
use std::collections::HashMap;

pub struct WindowsHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
}

impl<O: OutputBackend> WindowsHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
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
        }
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
        Ok(action)
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<(), String> {
        for (dx, dy) in self.sm.tick(dt_ms) {
            self.out.move_rel(dx as i32, dy as i32)?;
        }
        Ok(())
    }

    fn execute(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::ClickLeft => self.out.click(Button::Left)?,
            Action::ClickRight => self.out.click(Button::Right)?,
            Action::ScrollUp => self.out.scroll(0, 1)?,
            Action::ScrollDown => self.out.scroll(0, -1)?,
            _ => {}
        }
        Ok(())
    }

    pub fn sm(&self) -> &StateMachine {
        &self.sm
    }

    pub fn is_intercepting(&self) -> bool {
        self.sm.layer() == clickless_core::Layer::Mouse
    }

    pub fn out(&self) -> &O {
        &self.out
    }
}

pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    hook: WindowsHook<O>,
    mut is_running: impl FnMut() -> bool,
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
        },
        start_time: Instant::now(),
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

    while is_running() {
        unsafe {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
        }
        let now = Instant::now();
        let dt_ms = now.duration_since(last_tick).as_millis() as u64;
        if dt_ms >= 10 {
            let _ = state.hook.tick(dt_ms);
            last_tick = now;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

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
        buttons: Vec<(Button, Dir)>,
        scrolls: Vec<(i32, i32)>,
    }

    impl MockOut {
        fn new() -> Self {
            Self {
                moves: Vec::new(),
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
