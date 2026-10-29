//! Grid overlay for macOS: a borderless, transparent, click-through `NSWindow`.
//!
//! Pixel content comes from `clickless_backend_api::overlay::render_frame`, so the
//! drawing is covered by tests that run everywhere. This file owns the window, the
//! `CGImage` hand-off and the show/hide calls.
//!
use clickless_backend_api::OverlayBackend;
use clickless_backend_api::overlay::{OverlayTheme, render_frame_with_theme};
use clickless_core::grid::OverlayFrame;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSImage, NSImageView, NSScreen, NSStatusWindowLevel, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::ffi::c_void;
use std::ptr::NonNull;

/// Frees the pixel buffer that `CGDataProviderCreateWithData` borrowed.
unsafe extern "C-unwind" fn release_pixels(
    info: *mut c_void,
    _data: NonNull<c_void>,
    _size: usize,
) {
    if !info.is_null() {
        drop(unsafe { Box::from_raw(info as *mut Vec<u8>) });
    }
}

fn image_from(pixels: &[u8], width: u32, height: u32) -> Result<CFRetained<CGImage>, String> {
    let bytes_per_row = width as usize * 4;
    // The provider borrows this buffer and the release callback gives it back.
    let owned: *mut Vec<u8> = Box::into_raw(Box::new(pixels.to_vec()));
    let (data, size) = unsafe { ((*owned).as_ptr() as *const c_void, (*owned).len()) };

    unsafe {
        let Some(provider) =
            CGDataProvider::with_data(owned as *mut c_void, data, size, Some(release_pixels))
        else {
            drop(Box::from_raw(owned));
            return Err("CGDataProviderCreateWithData returned null".to_string());
        };
        // Premultiplied first with 32-bit little-endian words matches the BGRA buffer.
        let bitmap_info = CGBitmapInfo(
            CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0,
        );
        let colorspace =
            CGColorSpace::new_device_rgb().ok_or("CGColorSpaceCreateDeviceRGB returned null")?;
        CGImage::new(
            width as usize,
            height as usize,
            8,
            32,
            bytes_per_row,
            Some(&colorspace),
            bitmap_info,
            Some(&provider),
            std::ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
        .ok_or_else(|| "CGImageCreate returned null".to_string())
    }
}

/// Transparent, click-through, always-on-top overlay window.
pub struct MacosOverlay {
    theme: OverlayTheme,
    window: Retained<NSWindow>,
    image_view: Retained<NSImageView>,
    visible: bool,
}

// AppKit windows are used from the thread that created them, which is the thread
// running the hook loop. The bound exists because the hook stores the renderer as
// a boxed trait object.
unsafe impl Send for MacosOverlay {}

impl MacosOverlay {
    pub fn new() -> Result<Self, String> {
        let mtm = MainThreadMarker::new()
            .ok_or("the overlay window must be created on the main thread")?;
        let size = NSSize::new(1.0, 1.0);
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), size);
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                rect,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        let clear = NSColor::clearColor();
        window.setOpaque(false);
        window.setBackgroundColor(Some(&clear));
        window.setIgnoresMouseEvents(true);
        window.setHasShadow(false);
        window.setLevel(NSStatusWindowLevel);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        unsafe { window.setReleasedWhenClosed(false) };

        let image_view = NSImageView::initWithFrame(mtm.alloc(), rect);
        window.setContentView(Some(&image_view));

        Ok(Self {
            theme: OverlayTheme::default(),
            window,
            image_view,
            visible: false,
        })
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn with_theme(mut self, theme: OverlayTheme) -> Self {
        self.theme = theme;
        self
    }

    fn render(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        let Some(target) = render_frame_with_theme(frame, &self.theme) else {
            return self.hide();
        };
        let image = image_from(&target.pixels, target.width, target.height)?;
        let size = NSSize::new(target.width as f64, target.height as f64);
        let mtm = MainThreadMarker::new()
            .ok_or("the overlay window must be updated on the main thread")?;
        let ns_image = NSImage::initWithCGImage_size(mtm.alloc(), &image, size);

        self.image_view.setImage(Some(&ns_image));
        self.window.setContentSize(size);
        let primary = NSScreen::screens(mtm)
            .firstObject()
            .ok_or("No primary macOS screen available")?;
        let primary_frame = primary.frame();
        let (x, y) = super::geometry::appkit_top_left(
            target.origin,
            primary_frame.origin.y + primary_frame.size.height,
        );
        self.window.setFrameTopLeftPoint(NSPoint::new(x, y));
        self.window.orderFrontRegardless();

        self.visible = true;
        Ok(())
    }
}

impl OverlayBackend for MacosOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        self.render(frame)
    }

    fn hide(&mut self) -> Result<(), String> {
        self.window.orderOut(None);
        self.visible = false;
        Ok(())
    }
}

impl Drop for MacosOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_overlay_creation_reports_success_or_reason() {
        match MacosOverlay::new() {
            Ok(overlay) => assert!(!overlay.is_visible()),
            Err(reason) => {
                eprintln!("overlay window unavailable here: {reason}");
                assert!(!reason.is_empty());
            }
        }
    }

    #[test]
    fn t02_overlay_implements_backend_trait() {
        fn assert_backend<T: OverlayBackend>() {}
        assert_backend::<MacosOverlay>();
    }
}
