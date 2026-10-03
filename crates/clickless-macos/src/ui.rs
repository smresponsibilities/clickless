//! macOS AppKit UI prototyping for Home, Settings and safe Practice.

use clickless_config::Config;
use clickless_config::settings_model::SettingsPage;
use objc2_app_kit::{NSApplication, NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, ns_string};

// Prototype Settings Window using AppKit
pub fn show_settings(_config: &Config) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Must be on main thread to show UI")?;
    let app = NSApplication::sharedApplication(mtm);

    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    window.setTitle(ns_string!("Clickless Settings"));
    window.center();

    // Iterate over pages to prove descriptor usage in the prototype
    for page in SettingsPage::ALL {
        println!("Prototype: would render sidebar item for {}", page.title());
    }

    window.makeKeyAndOrderFront(None);

    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    // app.run(); // Disabled in prototype to prevent blocking CLI tests

    Ok(())
}
