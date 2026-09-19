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
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend {
        last_move: Option<(i32, i32)>,
        move_count: usize,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                last_move: None,
                move_count: 0,
            }
        }
    }

    impl OutputBackend for MockBackend {
        fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.last_move = Some((dx, dy));
            self.move_count += 1;
            Ok(())
        }

        fn button(&mut self, _b: Button, _d: Dir) -> Result<(), String> {
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
}
