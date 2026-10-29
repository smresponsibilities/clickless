use clickless_backend_api::{Button, Dir, OutputBackend, OverlayBackend};
use clickless_core::grid::{GridConfig, OverlayFrame};
use clickless_core::{Action, LogicalKey, MotionConfig};
use clickless_macos::MacosHook;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct TestOutput {
    buttons: Vec<(Button, Dir)>,
    scrolls: Vec<(i32, i32)>,
    moves: Vec<(i32, i32)>,
    abs_moves: Vec<(i32, i32)>,
}

impl TestOutput {
    fn new() -> Self {
        Self {
            buttons: Vec::new(),
            scrolls: Vec::new(),
            moves: Vec::new(),
            abs_moves: Vec::new(),
        }
    }
}

impl OutputBackend for TestOutput {
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.moves.push((dx, dy));
        Ok(())
    }
    fn move_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.abs_moves.push((x, y));
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

struct TestOverlay {
    shows: Arc<Mutex<Vec<u8>>>,
    hides: Arc<Mutex<usize>>,
}

impl OverlayBackend for TestOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.shows.lock().unwrap().push(frame.level);
        Ok(())
    }
    fn hide(&mut self) -> Result<(), String> {
        *self.hides.lock().unwrap() += 1;
        Ok(())
    }
}

#[cfg(test)]
mod macos_tests {
    use super::*;

    #[test]
    fn unmapped_keys_pass_and_motion_binding_dispatches() {
        let mut hook = MacosHook::new(TestOutput::new());
        // Unmapped return key (0x24) passes through
        let outcome = hook.process_key(0x24, true, 0).unwrap();
        assert_eq!(outcome, None);
        assert!(!hook.is_intercepting());

        // CapsLock hold enters mouse layer
        hook.process_key(0x39, true, 0).unwrap();
        hook.process_key(0x39, true, 200).unwrap();
        assert!(hook.is_intercepting());

        // 'J' (0x26) yields MoveRight
        let action = hook.process_key(0x26, true, 300).unwrap();
        assert_eq!(action, Some(Action::MoveRight));
    }

    #[test]
    fn pause_releases_hook_capture() {
        let mut hook = MacosHook::new(TestOutput::new());
        // CapsLock hold enters mouse layer
        hook.process_key(0x39, true, 0).unwrap();
        hook.process_key(0x39, true, 200).unwrap();
        assert!(hook.is_intercepting());

        // Pausing resets hook interception; permission APIs are tested separately.
        hook.set_paused(true).unwrap();
        assert!(!hook.is_intercepting());

        hook.set_paused(false).unwrap();
        assert!(!hook.is_intercepting());
    }

    #[test]
    fn grid_transition_shows_and_hides_overlay() {
        let shows = Arc::new(Mutex::new(Vec::new()));
        let hides = Arc::new(Mutex::new(0usize));
        let mut hook = MacosHook::new(TestOutput::new());
        hook.set_overlay(Box::new(TestOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        hook.sm_mut().enable_grid(2560, 1440, GridConfig::default());

        // CapsLock hold opens grid level 1
        hook.process_key(0x39, true, 0).unwrap();
        hook.process_key(0x39, true, 200).unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1]);

        // 'K' (0x28) enters level 2
        hook.process_key(0x28, true, 300).unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        // Esc (0x35) exits grid
        hook.process_key(0x35, true, 400).unwrap();
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn apply_config_changes_leader_and_bindings() {
        let mut hook = MacosHook::new(TestOutput::new());
        let mut custom_bindings = HashMap::new();
        custom_bindings.insert(LogicalKey::H, Action::ClickLeft);

        hook.apply_config(
            LogicalKey::ControlLeft,
            custom_bindings,
            MotionConfig::default(),
            180,
            3,
        )
        .unwrap();

        assert_eq!(hook.sm().leader_key(), LogicalKey::ControlLeft);
        assert_eq!(hook.sm().hold_ms(), 180);

        // ControlLeft press
        hook.process_key(0x3B, true, 0).unwrap();
        hook.process_key(0x3B, true, 190).unwrap();
        assert!(hook.is_intercepting());

        let action = hook.process_key(0x04, true, 250).unwrap(); // 'H' -> ClickLeft
        assert_eq!(action, Some(Action::ClickLeft));
    }

    #[test]
    fn scroll_step_controls_emitted_notches() {
        let mut hook = MacosHook::new(TestOutput::new());
        hook.set_scroll_step(4);
        hook.process_key(0x39, true, 0).unwrap();
        hook.process_key(0x39, true, 200).unwrap();
        let action = hook.process_key(0x0D, true, 300).unwrap(); // 'W' -> ScrollUp
        assert_eq!(action, Some(Action::ScrollUp));
        assert_eq!(hook.out().scrolls, vec![(0, 4)]);
    }
}
