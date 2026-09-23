pub mod key_code;
#[cfg(target_os = "linux")]
pub mod overlay;

use clickless_backend_api::{Button, Dir, NullOverlay, OutputBackend, OverlayBackend};
use clickless_core::grid::OverlayFrame;
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Phase, StateMachine};
use std::collections::HashMap;

pub struct LinuxHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
    overlay: Box<dyn OverlayBackend + Send>,
    shown_overlay: Option<OverlayFrame>,
    scroll_step: i64,
}

impl<O: OutputBackend> LinuxHook<O> {
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
        let key = match key_code::evdev_to_logical(code) {
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

#[cfg(target_os = "linux")]
pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    mut hook: LinuxHook<O>,
    mut is_running: impl FnMut() -> bool,
) -> Result<(), String> {
    use std::fs;
    use std::time::Instant;

    let mut keyboard_device = None;
    if let Ok(entries) = fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(mut dev) = evdev::Device::open(&path) else {
                continue;
            };
            let looks_like_a_keyboard = dev.supported_keys().is_some_and(|keys| {
                keys.contains(evdev::Key::KEY_CAPSLOCK) && keys.contains(evdev::Key::KEY_A)
            });
            if looks_like_a_keyboard && dev.grab().is_ok() {
                keyboard_device = Some(dev);
                break;
            }
        }
    }

    let mut dev =
        keyboard_device.ok_or("No accessible grabbed keyboard device found in /dev/input")?;
    let start_time = Instant::now();
    let mut last_tick = Instant::now();

    while is_running() {
        let mut read_error = None;
        match dev.fetch_events() {
            Ok(events) => {
                for ev in events {
                    if ev.event_type() == evdev::EventType::KEY {
                        let is_down = ev.value() == 1 || ev.value() == 2;
                        let now_ms = start_time.elapsed().as_millis() as u64;
                        let _ = hook.process_key(ev.code(), is_down, now_ms);
                    }
                }
            }
            // evdev is non-blocking, so an empty read is a normal poll miss.
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            // The error iterator borrows `dev`, so ungrab after the match ends.
            Err(e) => read_error = Some(e),
        }
        if let Some(e) = read_error {
            let _ = hook.hide_overlay();
            let _ = dev.ungrab();
            return Err(format!("Device read error: {e}"));
        }

        let now = Instant::now();
        let dt_ms = now.duration_since(last_tick).as_millis() as u64;
        if dt_ms >= 10 {
            let _ = hook.tick(dt_ms);
            last_tick = now;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let _ = hook.hide_overlay();
    let _ = dev.ungrab();
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

    fn enter_mouse(hook: &mut LinuxHook<MockOut>) {
        let _ = hook.process_key(58, true, 0); // CapsLock press
        let _ = hook.process_key(58, true, 200); // poll past threshold
    }

    #[test]
    fn t01_unmapped_code_yields_none() {
        let mut hook = LinuxHook::new(MockOut::new());
        assert_eq!(hook.process_key(28, true, 0).unwrap(), None); // KEY_ENTER
    }

    #[test]
    fn t02_capslock_hold_enters_mouse_layer() {
        let mut hook = LinuxHook::new(MockOut::new());
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t03_j_in_mouse_yields_move_right() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(36, true, 300).unwrap(),
            Some(Action::MoveRight)
        );
    }

    #[test]
    fn t04_h_in_mouse_yields_move_left() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(35, true, 300).unwrap(),
            Some(Action::MoveLeft)
        );
    }

    #[test]
    fn t05_k_in_mouse_yields_move_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(37, true, 300).unwrap(),
            Some(Action::MoveUp)
        );
    }

    #[test]
    fn t06_l_in_mouse_yields_move_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(38, true, 300).unwrap(),
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn t07_f_in_mouse_yields_click_left() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(33, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t08_d_in_mouse_yields_click_right() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(32, true, 300).unwrap(),
            Some(Action::ClickRight)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Right, Dir::Down), (Button::Right, Dir::Up)]
        );
    }

    #[test]
    fn t09_w_in_mouse_yields_scroll_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(17, true, 300).unwrap(),
            Some(Action::ScrollUp)
        );
        assert_eq!(hook.out().scrolls, vec![(0, 1)]);
    }

    #[test]
    fn t20_set_scroll_step_scales_scroll_actions() {
        let mut hook = LinuxHook::new(MockOut::new());
        hook.set_scroll_step(3);
        enter_mouse(&mut hook);
        hook.process_key(17, true, 300).unwrap(); // W -> ScrollUp
        hook.process_key(31, true, 400).unwrap(); // S -> ScrollDown
        assert_eq!(hook.out().scrolls, vec![(0, 3), (0, -3)]);
    }

    #[test]
    fn t10_s_in_mouse_yields_scroll_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(31, true, 300).unwrap(),
            Some(Action::ScrollDown)
        );
        assert_eq!(hook.out().scrolls, vec![(0, -1)]);
    }

    #[test]
    fn t11_tick_moves_cursor() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(36, true, 300).unwrap(); // J
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);
    }

    #[test]
    fn t12_esc_exits_mouse_layer() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(1, true, 300).unwrap(); // Esc
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t13_capslock_release_exits_mouse() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(58, false, 400).unwrap(); // CapsLock release
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t14_u_in_mouse_yields_speed_down() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(22, true, 300).unwrap(),
            Some(Action::SpeedDown)
        );
    }

    #[test]
    fn t15_o_in_mouse_yields_speed_up() {
        let mut hook = LinuxHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(24, true, 300).unwrap(),
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
        let mut hook = LinuxHook::with_config(
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

        hook.process_key(57, true, 300).unwrap(); // Space -> grid level 1
        hook.process_key(37, true, 400).unwrap(); // K -> level 2
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        hook.process_key(1, true, 500).unwrap(); // Esc leaves grid
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn t18_grid_drag_holds_button_until_leader_release() {
        use clickless_core::grid::GridConfig;

        let mut hook = LinuxHook::with_config(
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
        hook.process_key(57, true, 300).unwrap(); // Space -> grid
        hook.process_key(37, true, 400).unwrap(); // K -> level 2
        hook.process_key(37, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(37, false, 600).unwrap(); // release -> drag start

        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);

        hook.process_key(36, true, 700).unwrap(); // J drags right
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);

        hook.process_key(58, false, 800).unwrap(); // leader release
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
        let mut hook = LinuxHook::with_config(
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
        hook.process_key(57, true, 300).unwrap(); // Space -> grid
        hook.process_key(37, true, 400).unwrap(); // K -> level 2
        hook.process_key(37, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(37, false, 600).unwrap(); // release -> click

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
            LinuxHook::with_config(MockOut::new(), LogicalKey::Space, custom_bindings, motion);

        let _ = hook.process_key(57, true, 0); // Space press
        let _ = hook.process_key(57, true, 200); // poll past threshold
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);

        assert_eq!(
            hook.process_key(35, true, 300).unwrap(),
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }
}
