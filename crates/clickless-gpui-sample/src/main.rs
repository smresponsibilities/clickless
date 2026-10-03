use gpui_kit::gpui::*;
mod theme;
mod sidebar;
mod settings_group;
mod input;
mod switch;

fn main() {
    Application::new().run(|cx: &mut AppContext| {
        println!("GPUI Application initialized successfully.");
        cx.quit();
    });
}
