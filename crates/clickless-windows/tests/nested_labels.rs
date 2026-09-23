//! Ticket 025: nested subgrid shows no labels after two-letter press.
//! Drives the live dense seams (WindowsHook queue/flush + shared rasterizer)
//! and asserts the presented level-2 frame carries 30 visible one-char labels
//! at desktop and small-laptop geometries.

use clickless_backend_api::overlay::{
    LABEL_RGB, OverlayTheme, RenderTarget, frame_bounds, render_frame_with_theme,
};
use clickless_backend_api::{OutputBackend, OverlayBackend};
use clickless_core::grid::{GridConfig, OverlayCell, OverlayFrame};
use clickless_core::{LogicalKey, MotionConfig};
use clickless_windows::WindowsHook;
use std::sync::{Arc, Mutex};

struct MockOut;

impl OutputBackend for MockOut {
    fn move_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
    fn button(
        &mut self,
        _b: clickless_backend_api::Button,
        _d: clickless_backend_api::Dir,
    ) -> Result<(), String> {
        Ok(())
    }
    fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
struct FrameGrab {
    frames: Arc<Mutex<Vec<OverlayFrame>>>,
}

impl OverlayBackend for FrameGrab {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.frames.lock().unwrap().push(frame.clone());
        Ok(())
    }
    fn hide(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn label_pixels(target: &RenderTarget, x: i64, y: i64, w: i64, h: i64) -> usize {
    let (r, g, b) = LABEL_RGB;
    let mut count = 0;
    for yy in y..y + h {
        for xx in x..x + w {
            let i = ((yy * target.width as i64 + xx) * 4) as usize;
            if target.pixels[i] == b
                && target.pixels[i + 1] == g
                && target.pixels[i + 2] == r
                && target.pixels[i + 3] == 255
            {
                count += 1;
            }
        }
    }
    count
}

/// Runs the exact owner sequence (CapsLock hold, Space, D, release, G) and
/// returns the last presented frame.
fn presented_level2(width: i64, height: i64) -> OverlayFrame {
    let frames = Arc::new(Mutex::new(Vec::new()));
    let mut hook = WindowsHook::with_config(
        MockOut,
        LogicalKey::CapsLock,
        clickless_core::default_bindings(),
        MotionConfig::default(),
    );
    hook.sm_mut()
        .enable_grid(width, height, GridConfig::dense());
    hook.set_overlay(Box::new(FrameGrab {
        frames: frames.clone(),
    }));

    hook.process_key(0x14, true, 0).unwrap(); // CapsLock press
    hook.process_key(0x14, true, 200).unwrap(); // hold past threshold -> Mouse
    hook.process_key(0x20, true, 300).unwrap(); // Space -> grid level 1
    hook.process_key(0x44, true, 400).unwrap(); // D -> column bank
    hook.process_key(0x44, false, 450).unwrap(); // release D (repeat guard)
    hook.process_key(0x47, true, 500).unwrap(); // G -> nested level 2
    hook.process_key(0x47, false, 550).unwrap();
    for _ in 0..8 {
        hook.flush_overlay().unwrap();
    }

    let frames = frames.lock().unwrap();
    assert!(!frames.is_empty(), "no overlay frame was ever presented");
    frames.last().unwrap().clone()
}

fn assert_nested_labels_drawn(frame: &OverlayFrame) {
    assert_eq!(frame.level, 2, "screen still shows level {}", frame.level);
    let nested: Vec<&OverlayCell> = frame
        .cells
        .iter()
        .filter(|c| c.label.chars().count() == 1)
        .collect();
    assert_eq!(
        nested.len(),
        30,
        "expected 30 nested cells, got {}",
        nested.len()
    );
    let theme = OverlayTheme::default();
    let target = render_frame_with_theme(frame, &theme).unwrap();
    let bounds = frame_bounds(frame).unwrap();
    for cell in nested {
        assert!(!cell.label.is_empty(), "nested cell with empty label");
        let lx = cell.rect.x - bounds.x;
        let ly = cell.rect.y - bounds.y;
        let n = label_pixels(&target, lx, ly, cell.rect.width, cell.rect.height);
        assert!(
            n > 0,
            "nested label '{}' drew no pixels in {:?}",
            cell.label,
            cell.rect
        );
    }
}

#[test]
fn nested_labels_visible_after_two_letter_press() {
    assert_nested_labels_drawn(&presented_level2(1920, 1080));
}

#[test]
fn nested_labels_visible_on_small_laptop_geometry() {
    for (w, h) in [(1366, 768), (1280, 720)] {
        assert_nested_labels_drawn(&presented_level2(w, h));
    }
}
