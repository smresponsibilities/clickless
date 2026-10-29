use clickless_backend_api::{Button, Dir, OutputBackend, OverlayBackend};
use clickless_core::grid::{GridConfig, OverlayFrame};
use clickless_core::{Action, LogicalKey, MotionConfig};
use clickless_linux::LinuxHook;
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
mod linux_tests {
    use super::*;

    #[test]
    fn unmapped_keys_pass_and_motion_binding_dispatches() {
        let mut hook = LinuxHook::new(TestOutput::new());
        // Ordinary typing key (e.g. KEY_ENTER = 28) passes through (yields None)
        let outcome = hook.process_key(28, true, 0).unwrap();
        assert_eq!(outcome, None);
        assert!(!hook.is_intercepting());

        // Leader key hold enters mouse mode
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        assert!(hook.is_intercepting());

        // 'J' in mouse mode yields MoveRight
        let action = hook.process_key(36, true, 300).unwrap();
        assert_eq!(action, Some(Action::MoveRight));

        // Esc returns to Initial mode
        hook.process_key(1, true, 400).unwrap();
        assert!(!hook.is_intercepting());
    }

    #[test]
    fn input_error_releases_capture_and_attempts_ungrab() {
        let mut hook = LinuxHook::new(TestOutput::new());
        let shows = Arc::new(Mutex::new(Vec::new()));
        let hides = Arc::new(Mutex::new(0usize));
        hook.set_overlay(Box::new(TestOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        // Enter mouse mode
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        assert!(hook.is_intercepting());

        // Simulate device disconnection error during finish_input
        let ungrab_called = Arc::new(Mutex::new(false));
        let ungrab_cb = ungrab_called.clone();
        let result = hook.finish_input(Err("Device disconnected".into()), || {
            *ungrab_cb.lock().unwrap() = true;
            Ok(())
        });

        assert_eq!(result, Err("Device disconnected".into()));
        assert!(*ungrab_called.lock().unwrap());
        assert!(!hook.is_intercepting());
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn pause_releases_hook_capture() {
        let mut hook = LinuxHook::new(TestOutput::new());
        // Enter mouse mode
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        assert!(hook.is_intercepting());

        // Pause releases capture and disables interception
        hook.set_paused(true).unwrap();
        assert!(!hook.is_intercepting());

        // Unpause restores engine state capability
        hook.set_paused(false).unwrap();
        assert!(!hook.is_intercepting());
    }

    #[test]
    fn grid_transition_shows_and_hides_overlay() {
        let shows = Arc::new(Mutex::new(Vec::new()));
        let hides = Arc::new(Mutex::new(0usize));
        let mut hook = LinuxHook::new(TestOutput::new());
        hook.set_overlay(Box::new(TestOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());

        // Hold leader to open grid level 1
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1]);

        // Press 'K' to enter level 2
        hook.process_key(37, true, 300).unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        // Esc hides overlay
        hook.process_key(1, true, 400).unwrap();
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn apply_config_changes_leader_and_bindings() {
        let mut hook = LinuxHook::new(TestOutput::new());
        let mut custom_bindings = HashMap::new();
        custom_bindings.insert(LogicalKey::H, Action::ClickLeft);

        hook.apply_config(
            LogicalKey::Space,
            custom_bindings,
            MotionConfig::default(),
            150,
            2,
        )
        .unwrap();

        assert_eq!(hook.sm().leader_key(), LogicalKey::Space);
        assert_eq!(hook.sm().hold_ms(), 150);

        // Space leader press
        hook.process_key(57, true, 0).unwrap();
        hook.process_key(57, true, 160).unwrap();
        assert!(hook.is_intercepting());

        // 'H' triggers ClickLeft
        let action = hook.process_key(35, true, 200).unwrap();
        assert_eq!(action, Some(Action::ClickLeft));
    }

    #[test]
    fn scroll_step_controls_emitted_notches() {
        // Verify default config instantiation and scroll step setter
        let mut hook = LinuxHook::new(TestOutput::new());
        hook.set_scroll_step(5);
        hook.process_key(58, true, 0).unwrap();
        hook.process_key(58, true, 200).unwrap();
        let action = hook.process_key(17, true, 300).unwrap(); // 'W' -> ScrollUp
        assert_eq!(action, Some(Action::ScrollUp));
        assert_eq!(hook.out().scrolls, vec![(0, 5)]);
    }
}
