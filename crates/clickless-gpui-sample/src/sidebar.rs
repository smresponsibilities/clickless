use gpui_kit::gpui::*;
use super::theme::Theme;

pub struct Sidebar {
    pub theme: Theme,
}

impl Render for Sidebar {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(rems(14.0)) // ~224 units
            .bg(self.theme.nav)
            .border_r_1()
            .border_color(self.theme.border)
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(rems(2.25)) // 36 units
                    .px_2()
                    .rounded_md()
                    .hover(|s| s.bg(self.theme.selection))
                    .child(
                        div()
                            .text_color(self.theme.text)
                            .text_sm()
                            .child("Movement")
                    )
            )
            // Additional items would be repeated here
    }
}
