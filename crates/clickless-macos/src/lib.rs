#[cfg(target_os = "macos")]
mod input;
pub mod key_code;
#[cfg(target_os = "macos")]
pub mod lifecycle;
#[cfg(target_os = "macos")]
pub mod overlay;
#[cfg(target_os = "macos")]
pub mod permissions;

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
    pending_click: Option<Button>,
}

impl<O: OutputBackend> MacosHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
            scroll_step: 1,
            pending_click: None,
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
            pending_click: None,
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
        let (outcome, result) = self.process_key_outcome(code, is_down, now_ms);
        result.map(|()| outcome.action)
    }

    fn process_key_outcome(
        &mut self,
        code: u16,
        is_down: bool,
        now_ms: u64,
    ) -> (clickless_core::Outcome, Result<(), String>) {
        let key = match key_code::cg_to_logical(code) {
            Some(k) => k,
            None => {
                if is_down {
                    self.sm.interrupt_pending_taps();
                }
                return (clickless_core::Outcome::PASS, Ok(()));
            }
        };
        let phase = if is_down {
            Phase::Press
        } else {
            Phase::Release
        };
        let outcome = self.sm.on_event_outcome(KeyEvent::new(key, phase), now_ms);
        let result = (|| {
            if let Some(a) = outcome.action {
                self.execute(a)?;
            }
            self.sync_overlay()
        })();
        (outcome, result)
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
            Action::ClickLeft => self.click(Button::Left)?,
            Action::ClickRight => self.click(Button::Right)?,
            Action::ScrollUp => self.out.scroll(0, step)?,
            Action::ScrollDown => self.out.scroll(0, -step)?,
            Action::MoveTo(x, y) => self.out.move_abs(x as i32, y as i32)?,
            Action::ClickAt(x, y) => {
                self.out.move_abs(x as i32, y as i32)?;
                self.click(Button::Left)?;
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

    fn click(&mut self, button: Button) -> Result<(), String> {
        self.pending_click = Some(button);
        self.out.click(button)?;
        self.pending_click = None;
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

    pub fn apply_config(
        &mut self,
        leader: LogicalKey,
        bindings: HashMap<LogicalKey, Action>,
        motion: MotionConfig,
        hold_ms: u64,
        scroll_step: i64,
    ) -> Result<(), String> {
        self.sm.reconfigure(leader, bindings, motion);
        self.sm.set_hold_ms(hold_ms);
        self.set_scroll_step(scroll_step);
        Ok(())
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        let cleanup = if paused {
            self.release_capture()
        } else {
            Ok(())
        };
        self.sm.set_paused(paused);
        cleanup
    }

    pub fn release_capture(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        if let Some(action) = self.sm.force_exit()
            && let Err(error) = self.execute(action)
        {
            errors.push(format!("Button release failed: {error}"));
        }
        if let Some(button) = self.pending_click {
            match self.out.button(button, Dir::Up) {
                Ok(()) => self.pending_click = None,
                Err(error) => errors.push(format!("Click release failed: {error}")),
            }
        }
        if let Err(error) = self.hide_overlay() {
            errors.push(format!("Overlay hide failed: {error}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub fn out(&self) -> &O {
        &self.out
    }
}

#[cfg(target_os = "macos")]
pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    hook: MacosHook<O>,
    is_running: impl FnMut() -> bool,
) -> Result<(), String> {
    input::run(hook, is_running)
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
        // Holding the leader with the grid enabled opens level 1 directly
        // (core t73); the overlay is never shown for the Mouse layer.
        assert_eq!(*shows.lock().unwrap(), vec![1]);

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
#[cfg(target_os = "macos")]
pub mod ui;
