pub const LEADER_HOLD_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Initial,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalKey {
    CapsLock,
    H,
    J,
    K,
    L,
    U,
    O,
    F,
    D,
    W,
    S,
    Esc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Press,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: LogicalKey,
    pub phase: Phase,
}

impl KeyEvent {
    pub fn new(key: LogicalKey, phase: Phase) -> Self {
        Self { key, phase }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    SpeedDown,
    SpeedUp,
    ClickLeft,
    ClickRight,
    ScrollUp,
    ScrollDown,
}

#[derive(Debug)]
pub struct StateMachine {
    layer: Layer,
    leader_pressed_at: Option<u64>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            layer: Layer::Initial,
            leader_pressed_at: None,
        }
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn poll(&mut self, now_ms: u64) {
        if self.layer == Layer::Initial {
            if let Some(start) = self.leader_pressed_at {
                if now_ms.saturating_sub(start) >= LEADER_HOLD_MS {
                    self.layer = Layer::Mouse;
                }
            }
        }
    }

    pub fn on_event(&mut self, event: KeyEvent, now_ms: u64) -> Option<Action> {
        self.poll(now_ms);
        match (self.layer, event.key, event.phase) {
            (Layer::Initial, LogicalKey::CapsLock, Phase::Press) => {
                self.leader_pressed_at = Some(now_ms);
                None
            }
            (Layer::Initial, LogicalKey::CapsLock, Phase::Release)
            | (Layer::Mouse, LogicalKey::CapsLock, Phase::Release) => {
                self.leader_pressed_at = None;
                if self.layer == Layer::Mouse {
                    self.layer = Layer::Initial;
                }
                None
            }
            (Layer::Mouse, LogicalKey::Esc, Phase::Press) => {
                self.leader_pressed_at = None;
                self.layer = Layer::Initial;
                None
            }
            (Layer::Mouse, key, Phase::Press) => binding(key),
            _ => None,
        }
    }
}

fn binding(key: LogicalKey) -> Option<Action> {
    match key {
        LogicalKey::H => Some(Action::MoveLeft),
        LogicalKey::J => Some(Action::MoveRight),
        LogicalKey::K => Some(Action::MoveUp),
        LogicalKey::L => Some(Action::MoveDown),
        LogicalKey::U => Some(Action::SpeedDown),
        LogicalKey::O => Some(Action::SpeedUp),
        LogicalKey::F => Some(Action::ClickLeft),
        LogicalKey::D => Some(Action::ClickRight),
        LogicalKey::W => Some(Action::ScrollUp),
        LogicalKey::S => Some(Action::ScrollDown),
        _ => None,
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LogicalKey::*;
    use Phase::*;

    fn press(key: LogicalKey) -> KeyEvent {
        KeyEvent::new(key, Press)
    }

    fn release(key: LogicalKey) -> KeyEvent {
        KeyEvent::new(key, Release)
    }

    fn enter_mouse(sm: &mut StateMachine) {
        sm.on_event(press(CapsLock), 0);
        sm.poll(LEADER_HOLD_MS);
    }

    // state

    #[test]
    fn t01_starts_in_initial_layer() {
        let sm = StateMachine::new();
        assert_eq!(sm.layer(), Layer::Initial);
    }

    // tap-hold leader timing

    #[test]
    fn t02_leader_press_stays_initial_before_threshold() {
        let mut sm = StateMachine::new();
        sm.on_event(press(CapsLock), 0);
        sm.poll(LEADER_HOLD_MS - 1);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t03_leader_hold_enters_mouse_at_threshold() {
        let mut sm = StateMachine::new();
        sm.on_event(press(CapsLock), 0);
        sm.poll(LEADER_HOLD_MS);
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t04_leader_tap_within_threshold_emits_nothing() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.on_event(press(CapsLock), 0), None);
        assert_eq!(sm.on_event(release(CapsLock), 100), None);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t05_leader_release_after_hold_exits_mouse() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(release(CapsLock), 400);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t06_key_before_leader_hold_passes_through() {
        let mut sm = StateMachine::new();
        sm.on_event(press(CapsLock), 0);
        assert_eq!(sm.on_event(press(J), LEADER_HOLD_MS - 50), None);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t07_leader_still_held_long_term_stays_in_mouse() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.poll(10_000);
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    // mouse-layer default bindings

    #[test]
    fn t08_j_yields_move_right() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(J), 300), Some(Action::MoveRight));
    }

    #[test]
    fn t09_h_yields_move_left() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(H), 300), Some(Action::MoveLeft));
    }

    #[test]
    fn t10_k_yields_move_up() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(K), 300), Some(Action::MoveUp));
    }

    #[test]
    fn t11_l_yields_move_down() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(L), 300), Some(Action::MoveDown));
    }

    #[test]
    fn t12_u_yields_speed_down() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(U), 300), Some(Action::SpeedDown));
    }

    #[test]
    fn t13_o_yields_speed_up() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(O), 300), Some(Action::SpeedUp));
    }

    #[test]
    fn t14_f_yields_click_left() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(F), 300), Some(Action::ClickLeft));
    }

    #[test]
    fn t15_d_yields_click_right() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(D), 300), Some(Action::ClickRight));
    }

    #[test]
    fn t16_w_yields_scroll_up() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(W), 300), Some(Action::ScrollUp));
    }

    #[test]
    fn t17_s_yields_scroll_down() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(S), 300), Some(Action::ScrollDown));
    }

    #[test]
    fn t18_direction_key_release_yields_none() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        assert_eq!(sm.on_event(release(J), 320), None);
    }

    #[test]
    fn t19_repeat_press_yields_action_again() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(J), 300), Some(Action::MoveRight));
        sm.on_event(release(J), 320);
        assert_eq!(sm.on_event(press(J), 400), Some(Action::MoveRight));
    }

    // esc returns to initial

    #[test]
    fn t20_esc_in_mouse_returns_to_initial() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(Esc), 300);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t21_esc_in_initial_yields_nothing() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.on_event(press(Esc), 0), None);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    // exit semantics stop bindings

    #[test]
    fn t22_j_inert_after_leader_release() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(release(CapsLock), 400);
        assert_eq!(sm.on_event(press(J), 420), None);
    }

    #[test]
    fn t23_j_while_leader_held_processes_binding() {
        let mut sm = StateMachine::new();
        sm.on_event(press(CapsLock), 0);
        assert_eq!(sm.on_event(press(J), 500), Some(Action::MoveRight));
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t24_capslock_press_inside_mouse_yields_nothing() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.on_event(press(CapsLock), 300), None);
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t25_esc_exit_then_new_hold_reenters_mouse() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(Esc), 300);
        sm.on_event(press(CapsLock), 1000);
        sm.poll(1000 + LEADER_HOLD_MS);
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t26_second_tap_after_first_does_not_stick() {
        let mut sm = StateMachine::new();
        sm.on_event(press(CapsLock), 0);
        sm.on_event(release(CapsLock), 50);
        sm.on_event(press(CapsLock), 100);
        sm.on_event(release(CapsLock), 150);
        sm.poll(10_000);
        assert_eq!(sm.layer(), Layer::Initial);
    }
}
