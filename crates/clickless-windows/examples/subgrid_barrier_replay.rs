use clickless_backend_api::{Button, Dir, OutputBackend, OverlayBackend};
use clickless_core::grid::{GridConfig, OverlayFrame};
use clickless_core::{LogicalKey, MotionConfig};
use clickless_windows::WindowsHook;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockOut {
    abs: Vec<(i32, i32)>,
    buttons: Vec<(Button, Dir)>,
}

impl OutputBackend for MockOut {
    fn move_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }

    fn move_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.abs.push((x, y));
        Ok(())
    }

    fn button(&mut self, button: Button, dir: Dir) -> Result<(), String> {
        self.buttons.push((button, dir));
        Ok(())
    }

    fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameRecord {
    level: u8,
    one: usize,
    two: usize,
}

#[derive(Default)]
struct RecordingOverlay {
    frames: Arc<Mutex<Vec<FrameRecord>>>,
}

impl OverlayBackend for RecordingOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.frames.lock().unwrap().push(FrameRecord {
            level: frame.level,
            one: frame
                .cells
                .iter()
                .filter(|cell| cell.label.chars().count() == 1)
                .count(),
            two: frame
                .cells
                .iter()
                .filter(|cell| cell.label.chars().count() == 2)
                .count(),
        });
        Ok(())
    }

    fn hide(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn run_once(width: i64, height: i64) -> Result<(), String> {
    let frames = Arc::new(Mutex::new(Vec::new()));
    let mut hook = WindowsHook::with_config(
        MockOut::default(),
        LogicalKey::CapsLock,
        clickless_core::default_bindings(),
        MotionConfig::default(),
    );
    hook.sm_mut()
        .enable_grid(width, height, GridConfig::dense());
    hook.set_overlay(Box::new(RecordingOverlay {
        frames: frames.clone(),
    }));

    hook.process_key(0x14, true, 0)?; // CapsLock
    hook.process_key(0x14, true, 200)?; // promote to mouse
    hook.process_key(0x20, true, 300)?; // Space, grid
    hook.process_key(0x4B, true, 400)?; // K outer column
    hook.process_key(0x4B, false, 410)?;
    hook.process_key(0x4B, true, 420)?; // K outer row, queues required level 2
    let abs_before = hook.out().abs.len();
    hook.process_key(0x51, true, 430)?; // Q nested, deferred
    hook.process_key(0x51, false, 440)?; // Q release, deferred

    for _ in 0..4 {
        hook.flush_overlay()?;
    }

    let frames = frames.lock().unwrap();
    let level2_index = frames
        .iter()
        .position(|frame| frame.level == 2 && frame.one == 30 && frame.two == 299)
        .ok_or_else(|| "missing required level-2 subgrid presentation".to_string())?;
    if hook.out().abs.len() <= abs_before {
        return Err("nested key did not replay after subgrid presentation".to_string());
    }
    if hook.out().buttons.len() != 2 {
        return Err("nested release did not click after subgrid presentation".to_string());
    }
    let _ = level2_index;
    Ok(())
}

fn main() -> Result<(), String> {
    for _ in 0..1_000 {
        run_once(1920, 1080)?;
        run_once(2880, 1620)?;
        run_once(3840, 2160)?;
    }
    println!("ok: 1000 batched subgrid replays passed at 100/150/200% geometry");
    Ok(())
}
