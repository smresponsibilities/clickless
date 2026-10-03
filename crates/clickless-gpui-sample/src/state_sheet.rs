use gpui_kit::gpui::*;
use super::theme::Theme;
use super::input::SettingsInput;
use super::switch::SettingsSwitch;

pub struct StateSheet {
    pub theme: Theme,
}

impl Render for StateSheet {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_8()
            .p_8()
            .bg(self.theme.surface)
            // G05.01 visual state sheet representation
            .child(
                div().flex().flex_col().gap_4().child("Settings Input States")
                    .child(SettingsInput {
                        theme: Theme::dark(),
                        label: "Normal".to_string(),
                        raw_text: "15".to_string(),
                        validation_error: None,
                    })
                    .child(SettingsInput {
                        theme: Theme::dark(),
                        label: "Invalid".to_string(),
                        raw_text: "abc".to_string(),
                        validation_error: Some("Must be a number".to_string()),
                    })
            )
            .child(
                div().flex().flex_col().gap_4().child("Settings Switch States")
                    .child(SettingsSwitch {
                        theme: Theme::dark(),
                        is_on: true,
                        is_disabled: false,
                    })
                    .child(SettingsSwitch {
                        theme: Theme::dark(),
                        is_on: false,
                        is_disabled: false,
                    })
                    .child(SettingsSwitch {
                        theme: Theme::dark(),
                        is_on: true,
                        is_disabled: true, // Should render in secondary color
                    })
            )
    }
}
