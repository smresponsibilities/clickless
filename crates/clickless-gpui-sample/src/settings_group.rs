use super::theme::Theme;
use gpui_kit::gpui::*;

#[derive(IntoElement)]
pub struct SettingsGroup {
    pub theme: Theme,
    pub title: String,
    pub help_text: String,
}

impl RenderOnce for SettingsGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(rems(40.0)) // 640 units max bound
            .p_6()
            .gap_6() // 24-unit group gap
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_color(self.theme.text)
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_color(self.theme.secondary)
                            .text_sm()
                            .child(self.help_text.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .w_full()
                    .h(rems(2.5))
                    .bg(self.theme.surface)
                    .border_1()
                    .border_color(self.theme.border)
                    .rounded_md()
                    // Editor placeholder
                    .child("Editor Area"),
            )
    }
}
