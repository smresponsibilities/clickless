use crate::LogicalKey;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridConfig {
    pub rows: u32,
    pub cols: u32,
    pub keys: Vec<LogicalKey>,
    pub auto_free_mode_after_move: bool,
    pub nudge_enabled: bool,
    pub nudge_step_px: i64,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            rows: 3,
            cols: 3,
            keys: vec![
                LogicalKey::U,
                LogicalKey::I,
                LogicalKey::O,
                LogicalKey::J,
                LogicalKey::K,
                LogicalKey::L,
                LogicalKey::M,
                LogicalKey::Comma,
                LogicalKey::Dot,
            ],
            auto_free_mode_after_move: true,
            nudge_enabled: true,
            nudge_step_px: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

impl Rect {
    pub fn new(x: i64, y: i64, width: i64, height: i64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn center(&self) -> (i64, i64) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    pub fn subcell(&self, row: u32, col: u32, total_rows: u32, total_cols: u32) -> Self {
        let w = self.width / total_cols as i64;
        let h = self.height / total_rows as i64;
        let x = self.x + (col as i64 * w);
        let y = self.y + (row as i64 * h);
        Self {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridState {
    Inactive,
    Level1,
    Level2 {
        parent: Rect,
    },
    Nudging {
        current_pos: (i64, i64),
        held_key: LogicalKey,
    },
}

#[derive(Debug)]
pub struct GridNavigator {
    screen: Rect,
    config: GridConfig,
    state: GridState,
    key_to_cell: HashMap<LogicalKey, (u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridNavAction {
    ShowOverlayLevel1,
    ShowOverlayLevel2(Rect),
    HideOverlay,
    MoveCursorTo(i64, i64),
    Nudge(i64, i64),
    ClickAt(i64, i64),
    EnterFreeMode,
}

impl GridNavigator {
    pub fn new(screen_width: i64, screen_height: i64, config: GridConfig) -> Self {
        let mut key_to_cell = HashMap::new();
        for (idx, &k) in config.keys.iter().enumerate() {
            let row = idx as u32 / config.cols;
            let col = idx as u32 % config.cols;
            key_to_cell.insert(k, (row, col));
        }

        Self {
            screen: Rect::new(0, 0, screen_width, screen_height),
            config,
            state: GridState::Inactive,
            key_to_cell,
        }
    }

    pub fn state(&self) -> GridState {
        self.state
    }

    pub fn activate(&mut self) -> GridNavAction {
        self.state = GridState::Level1;
        GridNavAction::ShowOverlayLevel1
    }

    pub fn deactivate(&mut self) -> GridNavAction {
        self.state = GridState::Inactive;
        GridNavAction::HideOverlay
    }

    pub fn on_key_press(&mut self, key: LogicalKey) -> Option<GridNavAction> {
        match self.state {
            GridState::Inactive => None,
            GridState::Level1 => {
                if let Some(&(row, col)) = self.key_to_cell.get(&key) {
                    let cell = self
                        .screen
                        .subcell(row, col, self.config.rows, self.config.cols);
                    self.state = GridState::Level2 { parent: cell };
                    Some(GridNavAction::ShowOverlayLevel2(cell))
                } else if key == LogicalKey::Esc {
                    Some(self.deactivate())
                } else {
                    None
                }
            }
            GridState::Level2 { parent } => {
                if let Some(&(row, col)) = self.key_to_cell.get(&key) {
                    let subcell = parent.subcell(row, col, self.config.rows, self.config.cols);
                    let target = subcell.center();

                    if self.config.nudge_enabled {
                        self.state = GridState::Nudging {
                            current_pos: target,
                            held_key: key,
                        };
                        Some(GridNavAction::MoveCursorTo(target.0, target.1))
                    } else if self.config.auto_free_mode_after_move {
                        self.state = GridState::Inactive;
                        Some(GridNavAction::MoveCursorTo(target.0, target.1))
                    } else {
                        self.state = GridState::Inactive;
                        Some(GridNavAction::ClickAt(target.0, target.1))
                    }
                } else if key == LogicalKey::Esc {
                    Some(self.deactivate())
                } else {
                    None
                }
            }
            GridState::Nudging {
                mut current_pos,
                held_key,
            } => {
                let step = self.config.nudge_step_px;
                let delta = match key {
                    LogicalKey::H => Some((-step, 0)),
                    LogicalKey::J => Some((0, step)),
                    LogicalKey::K => Some((0, -step)),
                    LogicalKey::L => Some((step, 0)),
                    _ => None,
                };

                if let Some((dx, dy)) = delta {
                    current_pos.0 += dx;
                    current_pos.1 += dy;
                    self.state = GridState::Nudging {
                        current_pos,
                        held_key,
                    };
                    Some(GridNavAction::Nudge(dx, dy))
                } else {
                    None
                }
            }
        }
    }

    pub fn on_key_release(&mut self, key: LogicalKey) -> Option<GridNavAction> {
        match self.state {
            GridState::Nudging {
                current_pos,
                held_key,
            } if key == held_key => {
                self.state = GridState::Inactive;
                if self.config.auto_free_mode_after_move {
                    Some(GridNavAction::EnterFreeMode)
                } else {
                    Some(GridNavAction::ClickAt(current_pos.0, current_pos.1))
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_grid_2level_selection_and_click() {
        let config = GridConfig {
            nudge_enabled: false,
            auto_free_mode_after_move: false,
            ..Default::default()
        };

        let mut nav = GridNavigator::new(1920, 1080, config);
        assert_eq!(nav.activate(), GridNavAction::ShowOverlayLevel1);

        // Level 1: select center cell (K -> row 1, col 1)
        let act1 = nav.on_key_press(LogicalKey::K).unwrap();
        match act1 {
            GridNavAction::ShowOverlayLevel2(r) => {
                assert_eq!(r.x, 640);
                assert_eq!(r.y, 360);
                assert_eq!(r.width, 640);
                assert_eq!(r.height, 360);
            }
            other => panic!("expected ShowOverlayLevel2, got {:?}", other),
        }

        // Level 2: select center subcell (K -> row 1, col 1)
        let act2 = nav.on_key_press(LogicalKey::K).unwrap();
        assert_eq!(act2, GridNavAction::ClickAt(959, 540));
        assert_eq!(nav.state(), GridState::Inactive);
    }

    #[test]
    fn t02_subgrid_nudge_micro_adjust_then_release() {
        let config = GridConfig {
            nudge_enabled: true,
            auto_free_mode_after_move: false,
            nudge_step_px: 5,
            ..Default::default()
        };

        let mut nav = GridNavigator::new(1920, 1080, config);
        nav.activate();
        nav.on_key_press(LogicalKey::K); // Level 1 center (640..1280, 360..720)

        // Level 2 press K (center target: 959, 540)
        let act2 = nav.on_key_press(LogicalKey::K).unwrap();
        assert_eq!(act2, GridNavAction::MoveCursorTo(959, 540));

        // Nudge with H (left -5px) and J (down +5px) while holding K
        assert_eq!(
            nav.on_key_press(LogicalKey::H).unwrap(),
            GridNavAction::Nudge(-5, 0)
        );
        assert_eq!(
            nav.on_key_press(LogicalKey::J).unwrap(),
            GridNavAction::Nudge(0, 5)
        );

        // Release K -> clicks at adjusted (954, 545)
        let act_release = nav.on_key_release(LogicalKey::K).unwrap();
        assert_eq!(act_release, GridNavAction::ClickAt(954, 545));
        assert_eq!(nav.state(), GridState::Inactive);
    }

    #[test]
    fn t03_quick_select_plus_free_mode() {
        let config = GridConfig {
            nudge_enabled: true,
            auto_free_mode_after_move: true,
            ..Default::default()
        };

        let mut nav = GridNavigator::new(1920, 1080, config);
        nav.activate();
        nav.on_key_press(LogicalKey::K);
        nav.on_key_press(LogicalKey::K);

        // On release of final key -> Enters Free Mode
        let act = nav.on_key_release(LogicalKey::K).unwrap();
        assert_eq!(act, GridNavAction::EnterFreeMode);
    }
}
