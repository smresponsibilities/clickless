pub mod key_code;
#[cfg(target_os = "macos")]
pub mod overlay;

use clickless_backend_api::{Button, Dir, NullOverlay, OutputBackend, OverlayBackend};
use clickless_core::grid::OverlayFrame;
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Phase, StateMachine};
use std::collections::HashMap;

pub struct MacosHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
    overlay: Box<dyn OverlayBackend + Send>,
    shown_overlay: Option<OverlayFrame>,
    scroll_step: i64,
}

impl<O: OutputBackend> MacosHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
            scroll_step: 1,
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
            scroll_step: 1,
        }
    }

    /// Scroll notches per ScrollUp/Down action. Defaults to 1; the CLI sets
    /// it from config alongside the hold threshold on `sm`.
    pub fn set_scroll_step(&mut self, step: i64) {
        self.scroll_step = step.max(1);
    }

    /// Installs the grid overlay renderer. Defaults to a no-op renderer.
    pub fn set_overlay(&mut self, overlay: Box<dyn OverlayBackend + Send>) {
        self.overlay = overlay;
        self.shown_overlay = None;
    }

    pub fn process_key(
        &mut self,
        code: u16,
        is_down: bool,
        now_ms: u64,
    ) -> Result<Option<Action>, String> {
        let key = match key_code::cg_to_logical(code) {
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

    fn execute(&mut self, action: Action) -> Result<(), String> {
        let step = self.scroll_step.max(1) as i32;
        match action {
            Action::ClickLeft => self.out.click(Button::Left)?,
            Action::ClickRight => self.out.click(Button::Right)?,
            Action::ScrollUp => self.out.scroll(0, step)?,
            Action::ScrollDown => self.out.scroll(0, -step)?,
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

    pub fn out(&self) -> &O {
        &self.out
    }
}

#[cfg(target_os = "macos")]
pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    hook: MacosHook<O>,
    mut is_running: impl FnMut() -> bool,
) -> Result<(), String> {
    use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        EventField,
    };
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::time::Instant;

    static HOOK_PTR: AtomicPtr<MacosHookState> = AtomicPtr::new(null_mut());

    struct MacosHookState {
        hook: MacosHook<Box<dyn OutputBackend + Send>>,
        start_time: Instant,
    }

    let out_boxed: Box<dyn OutputBackend + Send> = Box::new(hook.out);
    let mut state = MacosHookState {
        hook: MacosHook {
            sm: hook.sm,
            out: out_boxed,
            overlay: hook.overlay,
            shown_overlay: None,
            scroll_step: hook.scroll_step,
        },
        start_time: Instant::now(),
    };
    HOOK_PTR.store(&mut state as *mut _, Ordering::SeqCst);

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        |_proxy, event_type, event| {
            let state_ptr = HOOK_PTR.load(Ordering::SeqCst);
            if !state_ptr.is_null() {
                let state = unsafe { &mut *state_ptr };
                let keycode =
                    event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                // CGEventType is not PartialEq in core-graphics 0.24, so match on it.
                let is_down = matches!(event_type, CGEventType::KeyDown)
                    || (matches!(event_type, CGEventType::FlagsChanged) && keycode == 0x39);
                let now_ms = state.start_time.elapsed().as_millis() as u64;
                let was_in_mouse = state.hook.is_intercepting();
                let _ = state.hook.process_key(keycode, is_down, now_ms);
                let is_in_mouse = state.hook.is_intercepting();
                if was_in_mouse || is_in_mouse {
                    return None; // Suppress event
                }
            }
            Some(event.to_owned())
        },
    )
    .map_err(|()| {
        "Failed to create CGEventTap. Ensure Accessibility permissions are granted.".to_string()
    })?;

    let loop_source = tap
        .mach_port
        .create_runloop_source(0)
        .map_err(|()| "Failed to create runloop source for CGEventTap".to_string())?;

    unsafe {
        CFRunLoop::get_current().add_source(&loop_source, kCFRunLoopCommonModes);
    }
    tap.enable();

    let mut last_tick = Instant::now();
    while is_running() {
        unsafe {
            CFRunLoop::run_in_mode(
                kCFRunLoopCommonModes,
                std::time::Duration::from_millis(5),
                true,
            );
        }
        let now = Instant::now();
        let dt_ms = now.duration_since(last_tick).as_millis() as u64;
        if dt_ms >= 10 {
            let _ = state.hook.tick(dt_ms);
            last_tick = now;
        }
    }

    let _ = state.hook.hide_overlay();
    // core-graphics 0.24 has no CGEventTap::disable; releasing the tap removes it.
    drop(tap);
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

    fn enter_mouse(hook: &mut MacosHook<MockOut>) {
        let _ = hook.process_key(0x39, true, 0); // CapsLock press
        let _ = hook.process_key(0x39, true, 200); // poll past threshold
    }

    #[test]
    fn t01_unmapped_code_yields_none() {
        let mut hook = MacosHook::new(MockOut::new());
        assert_eq!(hook.process_key(0x24, true, 0).unwrap(), None); // kVK_Return
    }

    #[test]
    fn t02_capslock_hold_enters_mouse_layer() {
        let mut hook = MacosHook::new(MockOut::new());
        hook.process_key(0x39, true, 0).unwrap();
        hook.process_key(0x39, true, 200).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t03_j_in_mouse_yields_move_right() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x26, true, 300).unwrap(),
            Some(Action::MoveRight)
        );
    }

    #[test]
    fn t04_h_in_mouse_yields_move_left() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x04, true, 300).unwrap(),
            Some(Action::MoveLeft)
        );
    }

    #[test]
    fn t05_k_in_mouse_yields_move_up() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x28, true, 300).unwrap(),
            Some(Action::MoveUp)
        );
    }

    #[test]
    fn t06_l_in_mouse_yields_move_down() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x25, true, 300).unwrap(),
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn t07_f_in_mouse_yields_click_left() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x03, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t08_d_in_mouse_yields_click_right() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x02, true, 300).unwrap(),
            Some(Action::ClickRight)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Right, Dir::Down), (Button::Right, Dir::Up)]
        );
    }

    #[test]
    fn t09_w_in_mouse_yields_scroll_up() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x0D, true, 300).unwrap(),
            Some(Action::ScrollUp)
        );
        assert_eq!(hook.out().scrolls, vec![(0, 1)]);
    }

    #[test]
    fn t20_set_scroll_step_scales_scroll_actions() {
        let mut hook = MacosHook::new(MockOut::new());
        hook.set_scroll_step(3);
        enter_mouse(&mut hook);
        hook.process_key(0x0D, true, 300).unwrap(); // W -> ScrollUp
        hook.process_key(0x01, true, 400).unwrap(); // S -> ScrollDown
        assert_eq!(hook.out().scrolls, vec![(0, 3), (0, -3)]);
    }

    #[test]
    fn t10_s_in_mouse_yields_scroll_down() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x01, true, 300).unwrap(),
            Some(Action::ScrollDown)
        );
        assert_eq!(hook.out().scrolls, vec![(0, -1)]);
    }

    #[test]
    fn t11_tick_moves_cursor() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x26, true, 300).unwrap(); // J
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);
    }

    #[test]
    fn t12_esc_exits_mouse_layer() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x35, true, 300).unwrap(); // Esc
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t13_capslock_release_exits_mouse() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x39, false, 400).unwrap(); // CapsLock release
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t14_u_in_mouse_yields_speed_down() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x20, true, 300).unwrap(),
            Some(Action::SpeedDown)
        );
    }

    #[test]
    fn t15_o_in_mouse_yields_speed_up() {
        let mut hook = MacosHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x1F, true, 300).unwrap(),
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
        let mut hook = MacosHook::with_config(
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

        hook.process_key(0x31, true, 300).unwrap(); // Space -> grid level 1
        hook.process_key(0x28, true, 400).unwrap(); // K -> level 2
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        hook.process_key(0x35, true, 500).unwrap(); // Esc leaves grid
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn t18_grid_drag_holds_button_until_leader_release() {
        use clickless_core::grid::GridConfig;

        let mut hook = MacosHook::with_config(
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
        hook.process_key(0x31, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x28, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x28, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x28, false, 600).unwrap(); // release -> drag start

        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);

        hook.process_key(0x26, true, 700).unwrap(); // J drags right
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);

        hook.process_key(0x39, false, 800).unwrap(); // leader release
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
        let mut hook = MacosHook::with_config(
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
        hook.process_key(0x31, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x28, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x28, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x28, false, 600).unwrap(); // release -> click

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
            MacosHook::with_config(MockOut::new(), LogicalKey::Space, custom_bindings, motion);

        let _ = hook.process_key(0x31, true, 0); // Space press
        let _ = hook.process_key(0x31, true, 200); // poll past threshold
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);

        assert_eq!(
            hook.process_key(0x04, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }
}
