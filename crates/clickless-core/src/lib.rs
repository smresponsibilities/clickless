pub const LEADER_HOLD_MS: u64 = 200;

pub mod grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Initial,
    Mouse,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalKey {
    A,
    B,
    C,
    E,
    G,
    N,
    P,
    Q,
    R,
    T,
    V,
    X,
    Y,
    Z,
    Semicolon,
    Slash,
    Backspace,
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

impl LogicalKey {
    /// Short overlay label for this key.
    pub fn label(&self) -> &'static str {
        match self {
            LogicalKey::CapsLock => "caps",
            LogicalKey::A => "a",
            LogicalKey::B => "b",
            LogicalKey::C => "c",
            LogicalKey::E => "e",
            LogicalKey::G => "g",
            LogicalKey::N => "n",
            LogicalKey::P => "p",
            LogicalKey::Q => "q",
            LogicalKey::R => "r",
            LogicalKey::T => "t",
            LogicalKey::V => "v",
            LogicalKey::X => "x",
            LogicalKey::Y => "y",
            LogicalKey::Z => "z",
            LogicalKey::Semicolon => ";",
            LogicalKey::Slash => "/",
            LogicalKey::Backspace => "backspace",
            LogicalKey::H => "h",
            LogicalKey::J => "j",
            LogicalKey::K => "k",
            LogicalKey::L => "l",
            LogicalKey::U => "u",
            LogicalKey::I => "i",
            LogicalKey::O => "o",
            LogicalKey::F => "f",
            LogicalKey::D => "d",
            LogicalKey::W => "w",
            LogicalKey::S => "s",
            LogicalKey::M => "m",
            LogicalKey::Comma => ",",
            LogicalKey::Dot => ".",
            LogicalKey::Space => "space",
            LogicalKey::Esc => "esc",
        }
    }
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
    EnterGrid,
    MoveTo(i64, i64),
    ClickAt(i64, i64),
    DragTo(i64, i64),
    DragEnd,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionConfig {
    pub start_speed_px_s: u64,
    pub max_speed_px_s: u64,
    pub ramp_ms: u64,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            start_speed_px_s: START_SPEED_PX_S,
            max_speed_px_s: MAX_SPEED_PX_S,
            ramp_ms: RAMP_MS,
        }
    }
}

#[derive(Debug)]
pub struct StateMachine {
    layer: Layer,
    leader_key: LogicalKey,
    bindings: std::collections::HashMap<LogicalKey, Action>,
    motion: MotionConfig,
    grid_nav: Option<grid::GridNavigator>,
    grid_cursor: Option<(i64, i64)>,
    drag_active: bool,
    leader_pressed_at: Option<u64>,
    held: Vec<(LogicalKey, Direction, u64)>,
    ramp_elapsed_ms: u64,
    mult_pct: u64,
    paused: bool,
}

impl StateMachine {
    pub fn new() -> Self {
        Self::with_config(
            LogicalKey::CapsLock,
            default_bindings(),
            MotionConfig::default(),
        )
    }

    pub fn with_config(
        leader_key: LogicalKey,
        bindings: std::collections::HashMap<LogicalKey, Action>,
        motion: MotionConfig,
    ) -> Self {
        Self {
            layer: Layer::Initial,
            leader_key,
            bindings,
            motion,
            grid_nav: None,
            grid_cursor: None,
            drag_active: false,
            leader_pressed_at: None,
            held: Vec::new(),
            ramp_elapsed_ms: 0,
            mult_pct: 100,
            paused: false,
        }
    }

    pub fn enable_grid(&mut self, screen_width: i64, screen_height: i64, config: grid::GridConfig) {
        self.enable_grid_with_monitors(
            vec![grid::Rect::new(0, 0, screen_width, screen_height)],
            (0, 0),
            config,
        );
    }

    pub fn enable_grid_with_monitors(
        &mut self,
        monitors: Vec<grid::Rect>,
        cursor: (i64, i64),
        config: grid::GridConfig,
    ) {
        self.grid_nav = Some(grid::GridNavigator::with_monitors(monitors, cursor, config));
    }

    /// Overlay description for the active grid level, if any.
    pub fn grid_overlay(&self) -> Option<grid::OverlayFrame> {
        let mut frame = self.grid_nav.as_ref().and_then(|nav| nav.overlay_frame())?;
        frame.pointer = self.grid_cursor;
        Some(frame)
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Pauses capture. Pausing forces an exit from any active layer, ending a
    /// held drag first, so no button stays pressed while paused. Returns the
    /// action the caller must execute to release app-held output, if any.
    pub fn set_paused(&mut self, paused: bool) -> Option<Action> {
        self.paused = paused;
        if paused { self.force_exit() } else { None }
    }

    /// Tray-level grid toggle: shows the overlay grid without a leader hold.
    /// No-op while paused, without grid support, or already shown.
    pub fn show_grid(&mut self) -> Option<Action> {
        if self.paused || self.layer == Layer::Grid || self.grid_nav.is_none() {
            return None;
        }
        if let Some(nav) = self.grid_nav.as_mut() {
            nav.activate();
        }
        self.layer = Layer::Grid;
        None
    }

    /// Public escape hatch: full reset to the initial layer, releasing any
    /// app-held drag button. Used by pause and by the tray hide command.
    pub fn force_exit(&mut self) -> Option<Action> {
        self.exit_to_initial()
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
        if self.paused {
            return None;
        }
        self.poll(now_ms);
        let leader = self.leader_key;
        if self.layer == Layer::Initial && event.key == leader && event.phase == Phase::Press {
            self.leader_pressed_at = Some(now_ms);
            return None;
        }
        if self.layer == Layer::Initial && event.key == leader && event.phase == Phase::Release {
            self.leader_pressed_at = None;
            return None;
        }
        if (self.layer == Layer::Mouse || self.layer == Layer::Grid)
            && event.key == leader
            && event.phase == Phase::Release
        {
            return self.exit_to_initial();
        }
        if (self.layer == Layer::Mouse || self.layer == Layer::Grid)
            && event.key == LogicalKey::Esc
            && event.phase == Phase::Press
        {
            return self.exit_to_initial();
        }

        if self.layer == Layer::Grid {
            if let Some(nav) = self.grid_nav.as_mut() {
                match event.phase {
                    Phase::Press => {
                        if let Some(act) = nav.on_key_press(event.key) {
                            return self.map_grid_action(act);
                        }
                    }
                    Phase::Release => {
                        if let Some(act) = nav.on_key_release(event.key) {
                            return self.map_grid_action(act);
                        }
                    }
                }
            }
            return None;
        }

        match (self.layer, event.phase) {
            (Layer::Mouse, Phase::Press) => {
                let action = self.bindings.get(&event.key).copied();
                if action == Some(Action::EnterGrid) {
                    if let Some(nav) = self.grid_nav.as_mut() {
                        if let Some(cursor) = self.grid_cursor {
                            nav.activate_at(cursor);
                        } else {
                            nav.activate();
                        }
                        self.layer = Layer::Grid;
                    }
                    return action;
                }
                if let Some(dir) = action.and_then(direction_of) {
                    if !self.held.iter().any(|(k, _, _)| *k == event.key) {
                        self.held.push((event.key, dir, 0));
                    }
                } else if action == Some(Action::SpeedUp) {
                    self.mult_pct = (self.mult_pct * 2).min(MULT_MAX_PCT);
                } else if action == Some(Action::SpeedDown) {
                    self.mult_pct = (self.mult_pct / 2).max(MULT_MIN_PCT);
                }
                action
            }
            (Layer::Mouse, Phase::Release) => {
                self.held.retain(|(k, _, _)| *k != event.key);
                if self.held.is_empty() {
                    self.ramp_elapsed_ms = 0;
                }
                None
            }
            _ => None,
        }
    }

    fn map_grid_action(&mut self, act: grid::GridNavAction) -> Option<Action> {
        match act {
            grid::GridNavAction::MoveCursorTo(x, y) => {
                self.grid_cursor = Some((x, y));
                Some(Action::MoveTo(x, y))
            }
            grid::GridNavAction::Nudge(dx, dy) => {
                let (x, y) = self.grid_cursor.unwrap_or((0, 0));
                let target = (x + dx, y + dy);
                self.grid_cursor = Some(target);
                Some(Action::MoveTo(target.0, target.1))
            }
            grid::GridNavAction::ClickAt(x, y) => {
                self.grid_cursor = Some((x, y));
                self.layer = Layer::Mouse;
                Some(Action::ClickAt(x, y))
            }
            grid::GridNavAction::StartDrag(x, y) => {
                self.grid_cursor = Some((x, y));
                self.drag_active = true;
                self.layer = Layer::Mouse;
                Some(Action::DragTo(x, y))
            }
            grid::GridNavAction::EnterFreeMode => {
                self.layer = Layer::Mouse;
                None
            }
            grid::GridNavAction::HideOverlay => {
                self.layer = Layer::Mouse;
                None
            }
            grid::GridNavAction::ShowOverlayLevel1 | grid::GridNavAction::ShowOverlayLevel2(_) => {
                None
            }
        }
    }

    fn exit_to_initial(&mut self) -> Option<Action> {
        if let Some(nav) = self.grid_nav.as_mut() {
            nav.deactivate();
        }
        self.leader_pressed_at = None;
        self.grid_cursor = None;
        self.held.clear();
        self.ramp_elapsed_ms = 0;
        self.mult_pct = 100;
        self.layer = Layer::Initial;
        let end_drag = self.drag_active.then_some(Action::DragEnd);
        self.drag_active = false;
        end_drag
    }

    pub fn current_speed_px_s(&self) -> u64 {
        if self.held.is_empty() {
            0
        } else {
            self.ramp_speed(self.ramp_elapsed_ms) * self.mult_pct / 100
        }
    }

    pub fn tick(&mut self, dt_ms: u64) -> Vec<(i64, i64)> {
        if self.paused || self.held.is_empty() {
            return Vec::new();
        }
        let speed = self.ramp_speed(self.ramp_elapsed_ms) * self.mult_pct / 100;
        self.ramp_elapsed_ms = (self.ramp_elapsed_ms + dt_ms).min(self.motion.ramp_ms);
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
        Self::speed_px_s_at_with_motion(&MotionConfig::default(), elapsed_ms)
    }

    pub fn speed_px_s_at_with_motion(motion: &MotionConfig, elapsed_ms: u64) -> u64 {
        let ramp = if motion.ramp_ms == 0 {
            1
        } else {
            motion.ramp_ms
        };
        motion.start_speed_px_s
            + (motion
                .max_speed_px_s
                .saturating_sub(motion.start_speed_px_s))
                * elapsed_ms.min(ramp)
                / ramp
    }

    fn ramp_speed(&self, elapsed_ms: u64) -> u64 {
        Self::speed_px_s_at_with_motion(&self.motion, elapsed_ms)
    }
}

pub fn default_bindings() -> std::collections::HashMap<LogicalKey, Action> {
    let mut map = std::collections::HashMap::new();
    map.insert(LogicalKey::H, Action::MoveLeft);
    map.insert(LogicalKey::J, Action::MoveRight);
    map.insert(LogicalKey::K, Action::MoveUp);
    map.insert(LogicalKey::L, Action::MoveDown);
    map.insert(LogicalKey::U, Action::SpeedDown);
    map.insert(LogicalKey::O, Action::SpeedUp);
    map.insert(LogicalKey::F, Action::ClickLeft);
    map.insert(LogicalKey::D, Action::ClickRight);
    map.insert(LogicalKey::W, Action::ScrollUp);
    map.insert(LogicalKey::S, Action::ScrollDown);
    map.insert(LogicalKey::Space, Action::EnterGrid);
    map
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

    #[test]
    fn t42_grid_mode_activation_and_navigation() {
        let mut sm = StateMachine::new();
        let mut bindings = default_bindings();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        sm.bindings = bindings;
        sm.enable_grid(1920, 1080, grid::GridConfig::default());

        enter_mouse(&mut sm);
        assert_eq!(sm.layer(), Layer::Mouse);

        // Press Space to enter Grid mode
        let act = sm.on_event(press(LogicalKey::Space), 300);
        assert_eq!(act, Some(Action::EnterGrid));
        assert_eq!(sm.layer(), Layer::Grid);

        // Grid Level 1: press K (center cell)
        let _ = sm.on_event(press(LogicalKey::K), 400);
        assert_eq!(sm.layer(), Layer::Grid);

        // Grid Level 2: press K (center subcell)
        let act2 = sm.on_event(press(LogicalKey::K), 500);
        assert_eq!(act2, Some(Action::MoveTo(959, 540)));
    }

    fn grid_machine_with(config: grid::GridConfig) -> StateMachine {
        let mut bindings = default_bindings();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        let mut sm =
            StateMachine::with_config(LogicalKey::CapsLock, bindings, MotionConfig::default());
        sm.enable_grid(1920, 1080, config);
        sm
    }

    fn grid_machine() -> StateMachine {
        grid_machine_with(grid::GridConfig::default())
    }

    fn enter_grid(sm: &mut StateMachine) {
        enter_mouse(sm);
        sm.on_event(press(LogicalKey::Space), 300);
    }

    #[test]
    fn t43_grid_release_after_subgrid_click() {
        let mut sm = grid_machine_with(grid::GridConfig {
            auto_free_mode_after_move: false,
            ..grid::GridConfig::default()
        });
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        assert_eq!(sm.on_event(press(K), 500), Some(Action::MoveTo(959, 540)));
        assert_eq!(
            sm.on_event(release(K), 600),
            Some(Action::ClickAt(959, 540))
        );
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t44_grid_nudge_returns_absolute_moves() {
        let mut sm = grid_machine();
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        assert_eq!(sm.on_event(press(K), 500), Some(Action::MoveTo(959, 540)));
        assert_eq!(sm.on_event(press(J), 510), Some(Action::MoveTo(959, 545)));
        assert_eq!(sm.on_event(press(L), 520), Some(Action::MoveTo(964, 545)));
    }

    #[test]
    fn t45_grid_overlay_frames_follow_grid_state() {
        let mut sm = grid_machine();
        assert!(sm.grid_overlay().is_none());
        enter_grid(&mut sm);
        let level1 = sm.grid_overlay().unwrap();
        assert_eq!(level1.level, 1);
        assert_eq!(level1.cells.len(), 9);

        sm.on_event(press(K), 400);
        assert_eq!(sm.grid_overlay().unwrap().level, 2);

        sm.on_event(press(Esc), 500);
        assert_eq!(sm.layer(), Layer::Initial);
        assert!(sm.grid_overlay().is_none());
    }

    #[test]
    fn t51_grid_overlay_highlights_the_selected_cell_and_pointer() {
        let mut sm = grid_machine();
        enter_grid(&mut sm);
        assert!(sm.grid_overlay().unwrap().highlight.is_none());
        assert!(sm.grid_overlay().unwrap().pointer.is_none());

        sm.on_event(press(K), 400);
        let level2 = sm.grid_overlay().unwrap();
        assert_eq!(level2.highlight, Some(grid::Rect::new(640, 360, 640, 360)));
        assert!(level2.pointer.is_none());

        sm.on_event(press(K), 500); // selects the centre subcell
        assert_eq!(sm.grid_overlay().unwrap().pointer, Some((959, 540)));
        assert_eq!(
            sm.grid_overlay().unwrap().highlight,
            Some(grid::Rect::new(853, 480, 213, 120))
        );

        sm.on_event(press(J), 600); // nudge down
        assert_eq!(sm.grid_overlay().unwrap().pointer, Some((959, 545)));
    }

    #[test]
    fn t47_default_bindings_enter_grid_on_space() {
        assert_eq!(
            default_bindings().get(&LogicalKey::Space),
            Some(&Action::EnterGrid)
        );
    }

    #[test]
    fn t48_grid_drag_after_select_starts_and_ends_drag() {
        let mut sm = grid_machine_with(grid::GridConfig {
            drag_after_select: true,
            auto_free_mode_after_move: false,
            ..grid::GridConfig::default()
        });
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        assert_eq!(sm.on_event(press(K), 500), Some(Action::MoveTo(959, 540)));

        // Release starts the drag instead of clicking.
        assert_eq!(sm.on_event(release(K), 600), Some(Action::DragTo(959, 540)));
        assert_eq!(sm.layer(), Layer::Mouse);

        // Flow keys now drag the selected target.
        sm.on_event(press(L), 700);
        assert_eq!(sm.tick(100), vec![(0, 30)]);
        sm.on_event(release(L), 800);

        assert_eq!(sm.on_event(release(CapsLock), 900), Some(Action::DragEnd));
        assert_eq!(sm.layer(), Layer::Initial);
        assert_eq!(sm.on_event(release(CapsLock), 910), None);
    }

    #[test]
    fn t49_grid_esc_during_drag_ends_drag() {
        let mut sm = grid_machine_with(grid::GridConfig {
            drag_after_select: true,
            auto_free_mode_after_move: false,
            ..grid::GridConfig::default()
        });
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        sm.on_event(press(K), 500);
        assert_eq!(sm.on_event(release(K), 600), Some(Action::DragTo(959, 540)));
        assert_eq!(sm.on_event(press(Esc), 700), Some(Action::DragEnd));
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t50_drag_is_opt_in_per_grid_config() {
        assert!(!grid::GridConfig::default().drag_after_select);
    }

    #[test]
    fn t46_grid_uses_monitor_under_cursor() {
        let mut bindings = default_bindings();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        let mut sm =
            StateMachine::with_config(LogicalKey::CapsLock, bindings, MotionConfig::default());
        sm.enable_grid_with_monitors(
            vec![
                grid::Rect::new(0, 0, 1920, 1080),
                grid::Rect::new(1920, 0, 2560, 1440),
            ],
            (2500, 700),
            grid::GridConfig::default(),
        );
        enter_grid(&mut sm);
        assert_eq!(
            sm.grid_overlay().unwrap().cells[0].rect,
            grid::Rect::new(1920, 0, 853, 480)
        );
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

    // tray lifecycle: pause, show_grid, force_exit

    #[test]
    fn t52_set_paused_forces_exit_and_releases_drag() {
        let mut sm = grid_machine_with(grid::GridConfig {
            drag_after_select: true,
            auto_free_mode_after_move: false,
            ..grid::GridConfig::default()
        });
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        sm.on_event(press(K), 500);
        assert_eq!(sm.on_event(release(K), 600), Some(Action::DragTo(959, 540)));

        assert_eq!(sm.set_paused(true), Some(Action::DragEnd));
        assert!(sm.is_paused());
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t53_paused_ignores_keys_and_ticks() {
        let mut sm = grid_machine();
        sm.set_paused(true);
        assert_eq!(sm.on_event(press(CapsLock), 0), None);
        sm.poll(LEADER_HOLD_MS);
        assert_eq!(sm.layer(), Layer::Initial);
        assert_eq!(sm.tick(100), Vec::new());
    }

    #[test]
    fn t54_resume_allows_leader_hold_again() {
        let mut sm = grid_machine();
        sm.set_paused(true);
        sm.set_paused(false);
        assert!(!sm.is_paused());
        sm.on_event(press(CapsLock), 0);
        sm.poll(LEADER_HOLD_MS);
        assert_eq!(sm.layer(), Layer::Mouse);
    }

    #[test]
    fn t55_show_grid_enters_grid_layer_without_leader() {
        let mut sm = grid_machine();
        assert_eq!(sm.show_grid(), None);
        assert_eq!(sm.layer(), Layer::Grid);
        assert!(sm.grid_overlay().is_some());
    }

    #[test]
    fn t56_show_grid_is_noop_when_paused_or_gridless() {
        let mut sm = StateMachine::new(); // no grid_nav
        assert_eq!(sm.show_grid(), None);
        assert_eq!(sm.layer(), Layer::Initial);

        let mut sm = grid_machine();
        sm.set_paused(true);
        assert_eq!(sm.show_grid(), None);
        assert_eq!(sm.layer(), Layer::Initial);
    }

    #[test]
    fn t57_force_exit_resets_everything_and_emits_drag_end() {
        let mut sm = grid_machine_with(grid::GridConfig {
            drag_after_select: true,
            auto_free_mode_after_move: false,
            ..grid::GridConfig::default()
        });
        enter_grid(&mut sm);
        sm.on_event(press(K), 400);
        sm.on_event(press(K), 500);
        sm.on_event(release(K), 600);
        assert_eq!(sm.force_exit(), Some(Action::DragEnd));
        assert_eq!(sm.layer(), Layer::Initial);
        assert!(sm.grid_overlay().is_none());
        assert_eq!(sm.force_exit(), None); // idempotent
    }

    #[test]
    fn t58_pause_inside_grid_hides_overlay_and_stops_selection() {
        let mut sm = grid_machine();
        enter_grid(&mut sm);
        assert_eq!(sm.set_paused(true), None);
        assert!(sm.grid_overlay().is_none());
        // a grid key while paused must not select anything
        sm.set_paused(false);
        assert_eq!(sm.layer(), Layer::Initial);
        assert_eq!(sm.on_event(press(K), 400), None);
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
