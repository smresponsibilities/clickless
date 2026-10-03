use gpui_kit::gpui::*;
mod theme;
mod sidebar;
mod settings_group;

fn main() {
    Application::new().run(|cx: &mut AppContext| {
        println!("GPUI Application initialized successfully.");
        cx.quit();
    });
}
