use clickless_backend_api::{Button, Dir, OutputBackend};
use enigo::{
    Axis, Button as EnigoButton, Coordinate, Direction as EnigoDirection, Enigo, Mouse, Settings,
};

pub struct EnigoAdapter {
    enigo: Enigo,
}

impl EnigoAdapter {
    pub fn new() -> Result<Self, String> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| format!("{e}"))?;
        Ok(Self { enigo })
    }
}

fn map_button(b: Button) -> EnigoButton {
    match b {
        Button::Left => EnigoButton::Left,
        Button::Right => EnigoButton::Right,
        Button::Middle => EnigoButton::Middle,
    }
}

fn map_dir(d: Dir) -> EnigoDirection {
    match d {
        Dir::Down => EnigoDirection::Press,
        Dir::Up => EnigoDirection::Release,
    }
}

impl OutputBackend for EnigoAdapter {
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.enigo
            .move_mouse(dx, dy, Coordinate::Rel)
            .map_err(|e| format!("{e}"))
    }

    fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
        self.enigo
            .button(map_button(b), map_dir(d))
            .map_err(|e| format!("{e}"))
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if dy != 0 {
            self.enigo
                .scroll(dy, Axis::Vertical)
                .map_err(|e| format!("{e}"))?;
        }
        if dx != 0 {
            self.enigo
                .scroll(dx, Axis::Horizontal)
                .map_err(|e| format!("{e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_backend_api::OutputBackend;

    #[test]
    fn t01_adapter_compiles_with_trait() {
        fn assert_output_backend<T: OutputBackend>() {}
        assert_output_backend::<EnigoAdapter>();
    }

    #[test]
    fn t02_new_returns_ok_or_headless_error() {
        let result = EnigoAdapter::new();
        match result {
            Ok(_) => {}
            Err(e) => {
                assert!(e.contains("connection") || e.contains("display") || e.contains("init"));
            }
        }
    }

    #[test]
    fn t03_move_rel_method_exists() {
        fn assert_impl<T: OutputBackend>() {}
        assert_impl::<EnigoAdapter>();
    }

    #[test]
    fn t04_map_button_covers_all_variants() {
        assert_eq!(map_button(Button::Left), EnigoButton::Left);
        assert_eq!(map_button(Button::Right), EnigoButton::Right);
        assert_eq!(map_button(Button::Middle), EnigoButton::Middle);
    }

    #[test]
    fn t05_map_dir_covers_all_variants() {
        assert_eq!(map_dir(Dir::Down), EnigoDirection::Press);
        assert_eq!(map_dir(Dir::Up), EnigoDirection::Release);
    }
}
