use gpui_kit::gpui::*;

pub struct Theme {
    pub bg: Rgba,
    pub nav: Rgba,
    pub surface: Rgba,
    pub text: Rgba,
    pub secondary: Rgba,
    pub border: Rgba,
    pub selection: Rgba,
    pub error: Rgba,
    pub button: Rgba,
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
            error: rgb(0xf44336),
            button: rgb(0x007acc),
        }
    }
}
