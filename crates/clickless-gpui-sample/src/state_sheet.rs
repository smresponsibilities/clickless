use super::input::SettingsInput;
use super::switch::SettingsSwitch;
use super::theme::Theme;
use gpui_kit::gpui::*;

pub struct StateSheet {
    pub theme: Theme,
}

impl Render for StateSheet {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_8()
            .p_8()
            .bg(self.theme.surface)
            .child(super::sidebar::Sidebar {
                theme: Theme::dark(),
            })
            .child(super::settings_group::SettingsGroup {
                theme: Theme::dark(),
                title: "Movement".into(),
                help_text: "Visual prototype. Settings controls are not connected.".into(),
            })
            // G05.01 visual state sheet representation
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child("Settings Input States")
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
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child("Settings Switch States")
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
                    }),
            )
    }
}
