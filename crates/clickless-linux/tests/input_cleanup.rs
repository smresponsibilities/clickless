use clickless_backend_api::{Button, Dir, OutputBackend, OverlayBackend};
use clickless_core::grid::{GridConfig, OverlayFrame};
use clickless_linux::LinuxHook;
use std::sync::{Arc, Mutex};

type Trace = Arc<Mutex<Vec<&'static str>>>;

struct Output {
    trace: Trace,
    fail_release: bool,
}

impl OutputBackend for Output {
    fn move_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
        Ok(())
    }
    fn scroll(&mut self, _: i32, _: i32) -> Result<(), String> {
        Ok(())
    }
    fn button(&mut self, button: Button, direction: Dir) -> Result<(), String> {
        assert_eq!(button, Button::Left);
        self.trace
            .lock()
            .unwrap()
            .push(if direction == Dir::Down { "down" } else { "up" });
        if direction == Dir::Up && self.fail_release {
            Err("release rejected".into())
        } else {
            Ok(())
        }
    }
}

struct Overlay(Trace);
impl OverlayBackend for Overlay {
    fn show(&mut self, _: &OverlayFrame) -> Result<(), String> {
        Ok(())
    }
    fn hide(&mut self) -> Result<(), String> {
        self.0.lock().unwrap().push("hide");
        Ok(())
    }
}

fn dragging_hook(fail_release: bool) -> (LinuxHook<Output>, Trace) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut hook = LinuxHook::new(Output {
        trace: trace.clone(),
        fail_release,
    });
    hook.set_overlay(Box::new(Overlay(trace.clone())));
    hook.sm_mut().enable_grid(
        1920,
        1080,
        GridConfig {
            drag_after_select: true,
            ..GridConfig::default()
        },
    );
    for (code, down, time) in [
        (58, true, 0),
        (58, true, 200),
        (57, true, 300),
        (37, true, 400),
        (37, true, 500),
        (37, false, 600),
    ] {
        hook.process_key(code, down, time).unwrap();
    }
    assert!(trace.lock().unwrap().contains(&"down"));
    trace.lock().unwrap().clear();
    (hook, trace)
}

#[test]
fn device_error_releases_drag_and_hides_before_ungrab() {
    let (mut hook, trace) = dragging_hook(false);
    let result = hook.finish_input(Err("Device read error: disconnected".into()), || {
        trace.lock().unwrap().push("ungrab");
        Ok(())
    });
    assert_eq!(*trace.lock().unwrap(), ["up", "hide", "ungrab"]);
    assert!(!hook.is_intercepting());
    assert_eq!(result, Err("Device read error: disconnected".into()));
}

#[test]
fn cleanup_failures_preserve_read_error_and_attempt_every_step() {
    let (mut hook, trace) = dragging_hook(true);
    let error = hook
        .finish_input(Err("Device read error: disconnected".into()), || {
            trace.lock().unwrap().push("ungrab");
            Err("ungrab rejected".into())
        })
        .unwrap_err();
    assert_eq!(*trace.lock().unwrap(), ["up", "hide", "ungrab"]);
    assert!(error.contains("Device read error: disconnected"));
    assert!(
        error.contains("release rejected"),
        "release failure must reach caller: {error}"
    );
    assert!(
        error.contains("ungrab rejected"),
        "ungrab failure must reach caller: {error}"
    );
    assert!(!hook.is_intercepting());
}
