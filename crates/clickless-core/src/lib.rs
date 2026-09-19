pub const LEADER_HOLD_MS: u64 = 200;

pub mod grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Initial,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalKey {
    CapsLock,
    H,
    J,
    K,
    L,
    U,
    I,
    O,
    F,
    D,
    W,
    S,
    M,
    Comma,
    Dot,
    Space,
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

pub const START_SPEED_PX_S: u64 = 300;
pub const MAX_SPEED_PX_S: u64 = 3000;
pub const RAMP_MS: u64 = 500;
pub const MULT_MIN_PCT: u64 = 25;
pub const MULT_MAX_PCT: u64 = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

fn direction_of(action: Action) -> Option<Direction> {
    match action {
        Action::MoveLeft => Some(Direction::Left),
        Action::MoveRight => Some(Direction::Right),
        Action::MoveUp => Some(Direction::Up),
        Action::MoveDown => Some(Direction::Down),
        _ => None,
    }
}

#[derive(Debug)]
pub struct StateMachine {
    layer: Layer,
    leader_pressed_at: Option<u64>,
    held: Vec<(LogicalKey, Direction, u64)>,
    ramp_elapsed_ms: u64,
    mult_pct: u64,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            layer: Layer::Initial,
            leader_pressed_at: None,
            held: Vec::new(),
            ramp_elapsed_ms: 0,
            mult_pct: 100,
        }
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn poll(&mut self, now_ms: u64) {
        if self.layer == Layer::Initial
            && let Some(start) = self.leader_pressed_at
            && now_ms.saturating_sub(start) >= LEADER_HOLD_MS
        {
            self.layer = Layer::Mouse;
        }
    }

    pub fn on_event(&mut self, event: KeyEvent, now_ms: u64) -> Option<Action> {
        self.poll(now_ms);
        match (self.layer, event.key, event.phase) {
            (Layer::Initial, LogicalKey::CapsLock, Phase::Press) => {
                self.leader_pressed_at = Some(now_ms);
                None
            }
            (Layer::Initial, LogicalKey::CapsLock, Phase::Release) => {
                self.leader_pressed_at = None;
                None
            }
            (Layer::Mouse, LogicalKey::CapsLock, Phase::Release) => {
                self.exit_to_initial();
                None
            }
            (Layer::Mouse, LogicalKey::Esc, Phase::Press) => {
                self.exit_to_initial();
                None
            }
            (Layer::Mouse, key, Phase::Press) => {
                let action = binding(key);
                if let Some(dir) = action.and_then(direction_of) {
                    if !self.held.iter().any(|(k, _, _)| *k == key) {
                        self.held.push((key, dir, 0));
                    }
                } else if action == Some(Action::SpeedUp) {
                    self.mult_pct = (self.mult_pct * 2).min(MULT_MAX_PCT);
                } else if action == Some(Action::SpeedDown) {
                    self.mult_pct = (self.mult_pct / 2).max(MULT_MIN_PCT);
                }
                action
            }
            (Layer::Mouse, key, Phase::Release) => {
                self.held.retain(|(k, _, _)| *k != key);
                if self.held.is_empty() {
                    self.ramp_elapsed_ms = 0;
                }
                None
            }
            _ => None,
        }
    }

    fn exit_to_initial(&mut self) {
        self.leader_pressed_at = None;
        self.held.clear();
        self.ramp_elapsed_ms = 0;
        self.mult_pct = 100;
        self.layer = Layer::Initial;
    }

    pub fn current_speed_px_s(&self) -> u64 {
        if self.held.is_empty() {
            0
        } else {
            ramp_speed(self.ramp_elapsed_ms) * self.mult_pct / 100
        }
    }

    pub fn tick(&mut self, dt_ms: u64) -> Vec<(i64, i64)> {
        if self.held.is_empty() {
            return Vec::new();
        }
        let speed = ramp_speed(self.ramp_elapsed_ms) * self.mult_pct / 100;
        self.ramp_elapsed_ms = (self.ramp_elapsed_ms + dt_ms).min(RAMP_MS);
        let mut moves = Vec::with_capacity(self.held.len());
        for entry in self.held.iter_mut() {
            let acc = entry.2 as u128 + speed as u128 * dt_ms as u128;
            let px = (acc / 1000).min(i64::MAX as u128) as i64;
            entry.2 = (acc % 1000) as u64;
            let disp = match entry.1 {
                Direction::Left => (-px, 0),
                Direction::Right => (px, 0),
                Direction::Up => (0, -px),
                Direction::Down => (0, px),
            };
            moves.push(disp);
        }
        moves
    }

    pub fn speed_px_s_at(elapsed_ms: u64) -> u64 {
        START_SPEED_PX_S + (MAX_SPEED_PX_S - START_SPEED_PX_S) * elapsed_ms.min(RAMP_MS) / RAMP_MS
    }
}

fn ramp_speed(elapsed_ms: u64) -> u64 {
    StateMachine::speed_px_s_at(elapsed_ms)
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

    // movement physics: ramp, multiplier, tick, drift

    #[test]
    fn t27_tick_without_directions_is_empty() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        assert_eq!(sm.tick(16), Vec::new());
    }

    #[test]
    fn t28_first_tick_right_moves_at_start_speed() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        assert_eq!(sm.tick(100), vec![(30, 0)]);
    }

    #[test]
    fn t29_held_h_moves_left_negative_x() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(H), 300);
        assert_eq!(sm.tick(100), vec![(-30, 0)]);
    }

    #[test]
    fn t30_k_and_l_move_on_y_axis_screen_down_positive() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(K), 300);
        assert_eq!(sm.tick(100), vec![(0, -30)]);
        sm.on_event(release(K), 400);
        sm.on_event(press(L), 410);
        assert_eq!(sm.tick(100), vec![(0, 30)]);
    }

    #[test]
    fn t31_ramp_midpoint_speed_after_250ms() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(250);
        assert_eq!(sm.current_speed_px_s(), 1650);
    }

    #[test]
    fn t32_ramp_reaches_max_speed_at_500ms_cumulative() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(250);
        sm.tick(250);
        assert_eq!(sm.current_speed_px_s(), MAX_SPEED_PX_S);
    }

    #[test]
    fn t33_ramp_stops_growing_past_window() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(250);
        sm.tick(5000);
        assert_eq!(sm.current_speed_px_s(), MAX_SPEED_PX_S);
        assert_eq!(sm.tick(100), vec![(300, 0)]);
    }

    #[test]
    fn t34_speedup_doubles_current_velocity() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(250);
        sm.on_event(press(O), 600);
        assert_eq!(sm.current_speed_px_s(), 3300);
        assert_eq!(sm.tick(100), vec![(330, 0)]);
    }

    #[test]
    fn t35_speeddown_halves_current_velocity() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.on_event(press(U), 310);
        assert_eq!(sm.current_speed_px_s(), 150);
        assert_eq!(sm.tick(100), vec![(15, 0)]);
    }

    #[test]
    fn t36_multiplier_clamped_to_bounds() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        for _ in 0..5 {
            sm.on_event(press(O), 400);
            sm.on_event(release(O), 401);
        }
        assert_eq!(sm.current_speed_px_s(), 2400);
        for _ in 0..6 {
            sm.on_event(press(U), 500);
            sm.on_event(release(U), 501);
        }
        assert_eq!(sm.current_speed_px_s(), 75);
    }

    #[test]
    fn t37_no_drift_after_all_direction_keys_release() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(200);
        sm.on_event(release(J), 600);
        assert_eq!(sm.tick(100), Vec::new());
        assert_eq!(sm.current_speed_px_s(), 0);
    }

    #[test]
    fn t38_ramp_resets_on_fresh_hold_after_release() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.tick(500);
        sm.on_event(release(J), 900);
        sm.on_event(press(J), 950);
        assert_eq!(sm.current_speed_px_s(), START_SPEED_PX_S);
    }

    #[test]
    fn t39_diagonal_returns_one_displacement_per_direction() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(H), 300);
        sm.on_event(press(K), 310);
        assert_eq!(sm.tick(100), vec![(-30, 0), (0, -30)]);
    }

    #[test]
    fn t40_subpixel_carry_prevents_slow_speed_starvation() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        let total_x: i64 = (0..4).map(|_| sm.tick(3)[0].0).sum();
        assert_eq!(total_x, 3);
    }

    #[test]
    fn t41_motion_resets_when_layer_exits_then_reenters() {
        let mut sm = StateMachine::new();
        enter_mouse(&mut sm);
        sm.on_event(press(J), 300);
        sm.on_event(press(O), 310);
        sm.on_event(press(Esc), 320);
        assert_eq!(sm.layer(), Layer::Initial);
        sm.on_event(press(CapsLock), 1000);
        sm.poll(1000 + LEADER_HOLD_MS);
        sm.on_event(press(J), 1210);
        assert_eq!(sm.current_speed_px_s(), START_SPEED_PX_S);
    }
}
