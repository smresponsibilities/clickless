use super::theme::Theme;
use gpui_kit::gpui::*;

#[derive(IntoElement)]
pub struct SettingsSwitch {
    pub theme: Theme,
    pub is_on: bool,
    pub is_disabled: bool,
}

impl RenderOnce for SettingsSwitch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let bg_color = if self.is_disabled {
            self.theme.secondary
        } else if self.is_on {
            self.theme.button
        } else {
            self.theme.border
        };

        div()
            .flex()
            .w(rems(2.5))
            .h(rems(1.5))
            .bg(bg_color)
            .rounded_full()
            .p_1()
            .items_center()
            .child(
                div()
                    .w(rems(1.0))
                    .h(rems(1.0))
                    .bg(self.theme.text)
                    .rounded_full(),
            )
    }
}
