use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Window, rgb};

#[derive(Clone, Copy)]
pub struct Palette {
    pub page: Hsla,
    pub rail: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub secondary: Hsla,
    pub border: Hsla,
    pub selection: Hsla,
    pub accent: Hsla,
    pub error: Hsla,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        let colors = if dark {
            [
                0x202126, 0x191a1e, 0x28292f, 0xf2f2f5, 0xb5b6c0, 0x3b3d47, 0x343541, 0xc6b9fa,
                0xffa7ac,
            ]
        } else {
            [
                0xfafafc, 0xf0f0f4, 0xffffff, 0x252630, 0x5b5d6b, 0xd4d5df, 0xe6e1f4, 0x66529b,
                0xb42336,
            ]
        };
        Self {
            page: rgb(colors[0]).into(),
            rail: rgb(colors[1]).into(),
            surface: rgb(colors[2]).into(),
            text: rgb(colors[3]).into(),
            secondary: rgb(colors[4]).into(),
            border: rgb(colors[5]).into(),
            selection: rgb(colors[6]).into(),
            accent: rgb(colors[7]).into(),
            error: rgb(colors[8]).into(),
        }
    }
}

pub fn apply(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let p = Palette::new(dark);
    let theme = Theme::global_mut(cx);
    theme.font_size = gpui_kit::px(14.);
    theme.background = p.page;
    theme.foreground = p.text;
    theme.sidebar = p.rail;
    theme.sidebar_foreground = p.text;
    theme.sidebar_accent = p.selection;
    theme.sidebar_accent_foreground = p.text;
    theme.border = p.border;
    theme.input = p.surface;
    theme.muted = p.rail;
    theme.muted_foreground = p.secondary;
    theme.secondary = p.surface;
    theme.secondary_hover = p.selection;
    theme.secondary_active = p.selection;
    theme.secondary_foreground = p.text;
    theme.primary = p.accent;
    theme.primary_hover = p.accent;
    theme.primary_active = p.accent;
    theme.primary_foreground = p.rail;
    theme.ring = p.accent;
    theme.danger = p.error;
    theme.button_primary = p.accent;
    theme.button_primary_hover = p.accent;
    theme.button_primary_active = p.accent;
    theme.button_primary_foreground = p.rail;
    Theme::sync_base(cx);
}
