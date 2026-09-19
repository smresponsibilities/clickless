//! Grid overlay for X11: an override-redirect ARGB window that passes input through.
//!
//! Pixel content comes from `clickless_backend_api::overlay::render_frame`, so the
//! drawing is covered by tests that run everywhere. This file only owns the X11
//! window, the input shape and the `put_image` blit.
//!
//! Transparency needs a compositing manager; without one the cells paint over the
//! desktop as opaque rectangles. Wayland is not supported here and needs a
//! layer-shell client instead.

use clickless_backend_api::OverlayBackend;
use clickless_backend_api::overlay::render_frame;
use clickless_core::grid::OverlayFrame;
use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as ShapeExt, SK, SO};
use x11rb::protocol::xproto::{
    ClipOrdering, ColormapAlloc, ConfigureWindowAux, ConnectionExt as XprotoExt, CreateGCAux,
    CreateWindowAux, EventMask, Gcontext, ImageFormat, StackMode, VisualClass, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

fn argb_visual(connection: &RustConnection, screen_num: usize) -> Result<(u8, u32), String> {
    let screen = &connection.setup().roots[screen_num];
    for depth in &screen.allowed_depths {
        if depth.depth != 32 {
            continue;
        }
        for visual in &depth.visuals {
            if visual.class == VisualClass::TRUE_COLOR {
                return Ok((32, visual.visual_id));
            }
        }
    }
    Err("no 32-bit ARGB visual on this X screen".to_string())
}

/// Transparent, input-passing overlay window on the X server's root screen.
pub struct LinuxOverlay {
    connection: RustConnection,
    window: Window,
    gc: Gcontext,
    depth: u8,
    visible: bool,
}

impl LinuxOverlay {
    pub fn new() -> Result<Self, String> {
        let (connection, screen_num) =
            x11rb::connect(None).map_err(|e| format!("X11 connect failed: {e}"))?;
        let (depth, visual) = argb_visual(&connection, screen_num)?;
        let root = connection.setup().roots[screen_num].root;

        let window = connection
            .generate_id()
            .map_err(|e| format!("X11 id allocation failed: {e}"))?;
        let colormap = connection
            .generate_id()
            .map_err(|e| format!("X11 id allocation failed: {e}"))?;
        connection
            .create_colormap(ColormapAlloc::NONE, colormap, root, visual)
            .map_err(|e| format!("X11 create_colormap failed: {e}"))?;
        connection
            .create_window(
                depth,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                visual,
                &CreateWindowAux::new()
                    .background_pixel(0)
                    .border_pixel(0)
                    .colormap(colormap)
                    .override_redirect(1)
                    .event_mask(EventMask::EXPOSURE),
            )
            .map_err(|e| format!("X11 create_window failed: {e}"))?;

        // An empty input shape sends every pointer and key event to what is below.
        connection
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )
            .map_err(|e| format!("X11 shape_rectangles failed: {e}"))?;

        let gc = connection
            .generate_id()
            .map_err(|e| format!("X11 id allocation failed: {e}"))?;
        connection
            .create_gc(gc, window, &CreateGCAux::new())
            .map_err(|e| format!("X11 create_gc failed: {e}"))?;
        connection
            .flush()
            .map_err(|e| format!("X11 flush failed: {e}"))?;

        Ok(Self {
            connection,
            window,
            gc,
            depth,
            visible: false,
        })
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn render(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        let Some(target) = render_frame(frame) else {
            return self.hide();
        };
        let width = target.width.clamp(1, u16::MAX as u32) as u16;
        let height = target.height.clamp(1, u16::MAX as u32) as u16;

        self.connection
            .configure_window(
                self.window,
                &ConfigureWindowAux::new()
                    .x(target.origin.0 as i32)
                    .y(target.origin.1 as i32)
                    .width(width as u32)
                    .height(height as u32)
                    .stack_mode(StackMode::ABOVE),
            )
            .map_err(|e| format!("X11 configure_window failed: {e}"))?;
        self.connection
            .put_image(
                ImageFormat::Z_PIXMAP,
                self.window,
                self.gc,
                width,
                height,
                0,
                0,
                0,
                self.depth,
                &target.pixels,
            )
            .map_err(|e| format!("X11 put_image failed: {e}"))?;
        self.connection
            .map_window(self.window)
            .map_err(|e| format!("X11 map_window failed: {e}"))?;
        self.connection
            .flush()
            .map_err(|e| format!("X11 flush failed: {e}"))?;

        self.visible = true;
        Ok(())
    }
}

impl OverlayBackend for LinuxOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.render(frame)
    }

    fn hide(&mut self) -> Result<(), String> {
        self.connection
            .unmap_window(self.window)
            .map_err(|e| format!("X11 unmap_window failed: {e}"))?;
        self.connection
            .flush()
            .map_err(|e| format!("X11 flush failed: {e}"))?;
        self.visible = false;
        Ok(())
    }
}

impl Drop for LinuxOverlay {
    fn drop(&mut self) {
        let _ = self.connection.unmap_window(self.window);
        let _ = self.connection.free_gc(self.gc);
        let _ = self.connection.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_overlay_connects_or_reports_why_not() {
        match LinuxOverlay::new() {
            Ok(overlay) => assert!(!overlay.is_visible()),
            Err(reason) => {
                eprintln!("overlay unavailable here: {reason}");
                assert!(!reason.is_empty());
            }
        }
    }

    #[test]
    fn t02_overlay_implements_backend_trait() {
        fn assert_backend<T: OverlayBackend>() {}
        assert_backend::<LinuxOverlay>();
    }
}
