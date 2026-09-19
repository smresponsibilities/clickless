pub mod key_code;

use clickless_backend_api::{Button, OutputBackend};
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Phase, StateMachine};
use std::collections::HashMap;

pub struct MacosHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
}

impl<O: OutputBackend> MacosHook<O> {
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
