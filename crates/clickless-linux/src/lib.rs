pub mod key_code;

use clickless_backend_api::{Button, Dir, OutputBackend};
use clickless_core::{Action, KeyEvent, Phase, StateMachine};

pub struct LinuxHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
}

impl<O: OutputBackend> LinuxHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
        }
    }

    pub fn process_key(&mut self, code: u16, is_down: bool, now_ms: u64) -> Option<Action> {
        let key = key_code::evdev_to_logical(code)?;
        let phase = if is_down {
            Phase::Press
        } else {
            Phase::Release
        };
        let action = self.sm.on_event(KeyEvent::new(key, phase), now_ms);
        if let Some(a) = action {
            let _ = self.execute(a);
        }
        action
    }

    pub fn tick(&mut self, dt_ms: u64) {
        for (dx, dy) in self.sm.tick(dt_ms) {
            let _ = self.out.move_rel(dx as i32, dy as i32);
        }
    }

    fn execute(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::ClickLeft => self.out.button(Button::Left, Dir::Down)?,
            Action::ClickRight => self.out.button(Button::Right, Dir::Down)?,
            Action::ScrollUp => self.out.scroll(0, 1)?,
            Action::ScrollDown => self.out.scroll(0, -1)?,
            _ => {}
        }
        Ok(())
    }

    pub fn sm(&self) -> &StateMachine {
        &self.sm
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

    fn enter_mouse(hook: &mut LinuxHook<MockOut>) {
        hook.process_key(58, true, 0); // CapsLock press
        hook.process_key(58, true, 200); // poll past threshold
    }

    #[test]
    fn t01_unmapped_code_yields_none() {
        let mut hook = LinuxHook::new(MockOut::new());
        assert_eq!(hook.process_key(28, true, 0), None); // KEY_ENTER
    }

    #[test]
    fn t02_capslock_hold_enters_mouse_layer() {
        let mut hook = LinuxHook::new(MockOut::new());
        hook.process_key(58, true, 0);
        hook.process_key(58, true, 200);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t03_j_in_mouse_yields_move_right() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(36, true, 300), Some(Action::MoveRight));
    }

    #[test]
    fn t04_h_in_mouse_yields_move_left() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(35, true, 300), Some(Action::MoveLeft));
    }

    #[test]
    fn t05_k_in_mouse_yields_move_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(37, true, 300), Some(Action::MoveUp));
    }

    #[test]
    fn t06_l_in_mouse_yields_move_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(38, true, 300), Some(Action::MoveDown));
    }

    #[test]
    fn t07_f_in_mouse_yields_click_left() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(33, true, 300), Some(Action::ClickLeft));
        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);
    }

    #[test]
    fn t08_d_in_mouse_yields_click_right() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(32, true, 300), Some(Action::ClickRight));
        assert_eq!(hook.out().buttons, vec![(Button::Right, Dir::Down)]);
    }

    #[test]
    fn t09_w_in_mouse_yields_scroll_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(17, true, 300), Some(Action::ScrollUp));
        assert_eq!(hook.out().scrolls, vec![(0, 1)]);
    }

    #[test]
    fn t10_s_in_mouse_yields_scroll_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(31, true, 300), Some(Action::ScrollDown));
        assert_eq!(hook.out().scrolls, vec![(0, -1)]);
    }

    #[test]
    fn t11_tick_moves_cursor() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(36, true, 300); // J
        hook.tick(100);
        assert_eq!(hook.out().moves, vec![(30, 0)]);
    }

    #[test]
    fn t12_esc_exits_mouse_layer() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(1, true, 300); // Esc
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t13_capslock_release_exits_mouse() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(58, false, 400); // CapsLock release
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t14_u_in_mouse_yields_speed_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(22, true, 300), Some(Action::SpeedDown));
    }

    #[test]
    fn t15_o_in_mouse_yields_speed_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(hook.process_key(24, true, 300), Some(Action::SpeedUp));
    }
}
