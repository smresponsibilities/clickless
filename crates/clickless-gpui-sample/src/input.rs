use super::theme::Theme;
use gpui_kit::gpui::*;

#[derive(IntoElement)]
pub struct SettingsInput {
    pub theme: Theme,
    pub label: String,
    pub raw_text: String,
    pub validation_error: Option<String>,
}

impl RenderOnce for SettingsInput {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.label.clone())
            .child(
                div()
                    .flex()
                    .w_full()
                    .h(rems(2.0))
                    .bg(self.theme.bg)
                    .border_1()
                    .border_color(if self.validation_error.is_some() {
                        self.theme.error
                    } else {
                        self.theme.border
                    })
                    .rounded_md()
                    .px_3()
                    .items_center()
                    .child(self.raw_text.clone()),
            )
            .children(self.validation_error.as_ref().map(|err| {
                div()
                    .text_color(self.theme.error)
                    .text_sm()
                    .child(err.clone())
            }))
    }
}
