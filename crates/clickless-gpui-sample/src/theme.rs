use gpui_kit::gpui::*;
use gpui_kit::components::*;

pub struct Theme {
    pub bg: Color,
    pub nav: Color,
    pub surface: Color,
    pub text: Color,
    pub secondary: Color,
    pub border: Color,
    pub selection: Color,
    pub focus: Color,
    pub error: Color,
    pub button: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x1e1e1e),
            nav: rgb(0x181818),
            surface: rgb(0x2d2d2d),
            text: rgb(0xe0e0e0),
            secondary: rgb(0xa0a0a0),
            border: rgb(0x404040),
            selection: rgb(0x3a3a3a),
            focus: rgb(0x007acc),
            error: rgb(0xf44336),
            button: rgb(0x007acc),
        }
    }
}
