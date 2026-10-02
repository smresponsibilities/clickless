//! Platform-neutral Settings presentation metadata.
//! Native hosts render this model; parsing, validation, and persistence remain
//! in `Config` and `SettingsEditor`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    General,
    Movement,
    Grid,
    Shortcuts,
    Appearance,
    About,
}

impl SettingsPage {
    /// Every page, in sidebar order. About carries no settings and stays last.
    pub const ALL: [SettingsPage; 6] = [
        SettingsPage::General,
        SettingsPage::Movement,
        SettingsPage::Grid,
        SettingsPage::Shortcuts,
        SettingsPage::Appearance,
        SettingsPage::About,
    ];

    /// Sidebar label shown by native hosts.
    pub fn title(self) -> &'static str {
        match self {
            SettingsPage::General => "General",
            SettingsPage::Movement => "Movement",
            SettingsPage::Grid => "Grid",
            SettingsPage::Shortcuts => "Shortcuts",
            SettingsPage::Appearance => "Appearance",
            SettingsPage::About => "About",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    Toggle,
    Text,
    Number,
    Choice,
    Shortcut,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingDescriptor {
    pub key: &'static str,
    pub page: SettingsPage,
    pub title: &'static str,
    pub description: &'static str,
    pub example: &'static str,
    pub kind: SettingKind,
}

pub const SETTINGS: &[SettingDescriptor] = &[
    SettingDescriptor {
        key: "enabled",
        page: SettingsPage::General,
        title: "Enabled",
        description: "Turn keyboard pointer control on or off.",
        example: "On",
        kind: SettingKind::Toggle,
    },
    SettingDescriptor {
        key: "leader",
        page: SettingsPage::General,
        title: "Activation key",
        description: "Hold this key to open the grid; release the leader to close it.",
        example: "CapsLock",
        kind: SettingKind::Shortcut,
    },
    SettingDescriptor {
        key: "hold_ms",
        page: SettingsPage::General,
        title: "Hold delay",
        description: "Wait before pointer mode starts.",
        example: "200 ms",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "start_speed",
        page: SettingsPage::Movement,
        title: "Start speed",
        description: "Pointer speed when movement begins.",
        example: "300 px/s",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "max_speed",
        page: SettingsPage::Movement,
        title: "Maximum speed",
        description: "Fastest pointer speed after ramp-up.",
        example: "3000 px/s",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "ramp_ms",
        page: SettingsPage::Movement,
        title: "Ramp duration",
        description: "Time to accelerate from start to maximum speed.",
        example: "500 ms",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "layout",
        page: SettingsPage::Grid,
        title: "Grid style",
        description: "Choose the grid workflow used for target selection.",
        example: "Simple",
        kind: SettingKind::Choice,
    },
    SettingDescriptor {
        key: "nested_size",
        page: SettingsPage::Grid,
        title: "Subgrid size",
        description: "Rows and columns in the fine-selection grid.",
        example: "3 × 3",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "nudge_enabled",
        page: SettingsPage::Grid,
        title: "Nudge after selection",
        description: "Make small pointer adjustments before clicking.",
        example: "On",
        kind: SettingKind::Toggle,
    },
    SettingDescriptor {
        key: "mouse_bindings",
        page: SettingsPage::Shortcuts,
        title: "Pointer shortcuts",
        description: "One key = action per line. Actions include move_left, move_down, move_up, move_right, click_left, click_right, scroll_up, scroll_down, speed_up, speed_down and enter_grid.",
        example: "J → move right",
        kind: SettingKind::Shortcut,
    },
    SettingDescriptor {
        key: "color_panel",
        page: SettingsPage::Appearance,
        title: "Panel color",
        description: "Background behind grid labels.",
        example: "181C26",
        kind: SettingKind::Color,
    },
    SettingDescriptor {
        key: "color_label",
        page: SettingsPage::Appearance,
        title: "Label color",
        description: "Color used for grid letters.",
        example: "F0F6FF",
        kind: SettingKind::Color,
    },
    SettingDescriptor {
        key: "about",
        page: SettingsPage::About,
        title: "About Clickless",
        description: "Keyboard-driven pointer control. Built with Rust and WinUI 3.",
        example: "Clickless v0.1.0 – a lightweight pointer control application.",
        kind: SettingKind::Text,
    },
    SettingDescriptor {
        key: "scroll_step",
        page: SettingsPage::Movement,
        title: "Scroll step",
        description: "Wheel steps for each scroll action.",
        example: "1",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "nested_keys",
        page: SettingsPage::Grid,
        title: "Target keys",
        description: "Space-separated keys, in reading order. Count must match subgrid size.",
        example: "u i o j k l m comma dot",
        kind: SettingKind::Text,
    },
    SettingDescriptor {
        key: "column_keys",
        page: SettingsPage::Grid,
        title: "Column keys",
        description: "Dense grid column labels, in left-to-right order.",
        example: "a s d f g h j k l semicolon",
        kind: SettingKind::Text,
    },
    SettingDescriptor {
        key: "row_keys",
        page: SettingsPage::Grid,
        title: "Row keys",
        description: "Dense grid row labels, in top-to-bottom order.",
        example: "q w e r t y u i o p",
        kind: SettingKind::Text,
    },
    SettingDescriptor {
        key: "nudge_step",
        page: SettingsPage::Grid,
        title: "Nudge distance",
        description: "Pixels moved by each fine-adjustment key.",
        example: "1",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "drag_after_select",
        page: SettingsPage::Grid,
        title: "Drag after selection",
        description: "Keep the left button held after selecting a target. Esc releases it.",
        example: "Off",
        kind: SettingKind::Toggle,
    },
    SettingDescriptor {
        key: "auto_free_mode",
        page: SettingsPage::Grid,
        title: "Continue in free mode",
        description: "Switch to continuous movement after a grid target.",
        example: "Off",
        kind: SettingKind::Toggle,
    },
    SettingDescriptor {
        key: "panel_opacity",
        page: SettingsPage::Appearance,
        title: "Panel opacity",
        description: "Background opacity from 0 to 255.",
        example: "150",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "color_border",
        page: SettingsPage::Appearance,
        title: "Grid line color",
        description: "Six-digit RGB hexadecimal color.",
        example: "526582",
        kind: SettingKind::Color,
    },
    SettingDescriptor {
        key: "border_px",
        page: SettingsPage::Appearance,
        title: "Grid line width",
        description: "Width of cell borders in pixels.",
        example: "1",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "color_highlight",
        page: SettingsPage::Appearance,
        title: "Selection color",
        description: "Color of the selected target.",
        example: "74B9FF",
        kind: SettingKind::Color,
    },
    SettingDescriptor {
        key: "highlight_opacity",
        page: SettingsPage::Appearance,
        title: "Selection opacity",
        description: "Selected target opacity from 0 to 255.",
        example: "180",
        kind: SettingKind::Number,
    },
    SettingDescriptor {
        key: "color_pointer",
        page: SettingsPage::Appearance,
        title: "Pointer marker color",
        description: "Color of the fine-adjustment marker.",
        example: "FFCE54",
        kind: SettingKind::Color,
    },
    SettingDescriptor {
        key: "label_size",
        page: SettingsPage::Appearance,
        title: "Label scale",
        description: "Pixel glyph scale, from 1 to 8.",
        example: "2",
        kind: SettingKind::Number,
    },
];

pub fn settings_for(page: SettingsPage) -> impl Iterator<Item = &'static SettingDescriptor> {
    SETTINGS.iter().filter(move |setting| setting.page == page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_descriptor_has_user_help_and_example() {
        assert!(!SETTINGS.is_empty());
        assert!(
            SETTINGS
                .iter()
                .all(|setting| !setting.description.is_empty() && !setting.example.is_empty())
        );
    }

    #[test]
    fn every_page_has_a_descriptor() {
        for page in [
            SettingsPage::General,
            SettingsPage::Movement,
            SettingsPage::Grid,
            SettingsPage::Shortcuts,
            SettingsPage::Appearance,
        ] {
            assert!(settings_for(page).next().is_some(), "page has no settings");
        }
    }
}
