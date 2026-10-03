use gpui_kit::gpui::*;
pub mod editor_bind;
mod input;
mod settings_group;
mod sidebar;
mod state_sheet;
mod switch;
mod theme;

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| state_sheet::StateSheet {
                theme: theme::Theme::dark(),
            })
        })
        .expect("failed to open GPUI sample window");
    });
}
