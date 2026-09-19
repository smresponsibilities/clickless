#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
}

pub trait OutputBackend {
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn button(&mut self, b: Button, d: Dir) -> Result<(), String>;
    fn click(&mut self, b: Button) -> Result<(), String> {
        self.button(b, Dir::Down)?;
        self.button(b, Dir::Up)
    }
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
}

impl<T: OutputBackend + ?Sized> OutputBackend for Box<T> {
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        (**self).move_rel(dx, dy)
    }

    fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
        (**self).button(b, d)
    }

    fn click(&mut self, b: Button) -> Result<(), String> {
        (**self).click(b)
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        (**self).scroll(dx, dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend {
        last_move: Option<(i32, i32)>,
        move_count: usize,
        buttons: Vec<(Button, Dir)>,
        fail_on_up: bool,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                last_move: None,
                move_count: 0,
                buttons: Vec::new(),
                fail_on_up: false,
            }
        }
    }

    impl OutputBackend for MockBackend {
        fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.last_move = Some((dx, dy));
            self.move_count += 1;
            Ok(())
        }

        fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
            if self.fail_on_up && d == Dir::Up {
                return Err("button release failure".to_string());
            }
            self.buttons.push((b, d));
            Ok(())
        }

        fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn t01_move_rel_records_displacement() {
        let mut b = MockBackend::new();
        b.move_rel(10, -5).unwrap();
        assert_eq!(b.last_move, Some((10, -5)));
        assert_eq!(b.move_count, 1);
    }

    #[test]
    fn t02_move_rel_chain_accumulates() {
        let mut b = MockBackend::new();
        b.move_rel(10, 0).unwrap();
        b.move_rel(5, 3).unwrap();
        assert_eq!(b.last_move, Some((5, 3)));
        assert_eq!(b.move_count, 2);
    }

    #[test]
    fn t03_button_succeeds() {
        let mut b = MockBackend::new();
        assert!(b.button(Button::Left, Dir::Down).is_ok());
        assert!(b.button(Button::Right, Dir::Up).is_ok());
    }

    #[test]
    fn t04_scroll_succeeds() {
        let mut b = MockBackend::new();
        assert!(b.scroll(0, 1).is_ok());
        assert!(b.scroll(-1, 0).is_ok());
    }

    #[test]
    fn t05_click_dispatches_balanced_down_and_up() {
        let mut b = MockBackend::new();
        b.click(Button::Left).unwrap();
        assert_eq!(
            b.buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t06_click_propagates_error_on_failure() {
        let mut b = MockBackend::new();
        b.fail_on_up = true;
        let res = b.click(Button::Right);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "button release failure");
    }
}
