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
    /// Release the final subgrid key with the left button held, so flow keys drag.
    pub drag_after_select: bool,
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
            drag_after_select: false,
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

    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    /// Splits the rect into `total_rows` x `total_cols` cells. The last row and
    /// column absorb the division remainder so cells tile the rect exactly.
    pub fn subcell(&self, row: u32, col: u32, total_rows: u32, total_cols: u32) -> Self {
        let (x, width) = partition(self.x, self.width, col, total_cols);
        let (y, height) = partition(self.y, self.height, row, total_rows);
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

fn partition(start: i64, total: i64, index: u32, count: u32) -> (i64, i64) {
    if count == 0 {
        return (start, total);
    }
    let count = count as i64;
    let base = total.div_euclid(count);
    let remainder = total.rem_euclid(count);
    let index = index as i64;
    let size = if index == count - 1 {
        base + remainder
    } else {
        base
    };
    (start + base * index, size)
}

/// One grid cell as handed to an overlay renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayCell {
    pub rect: Rect,
    pub label: String,
}

/// Pure overlay description. Renderers draw it; core never touches a screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFrame {
    pub level: u8,
    pub cells: Vec<OverlayCell>,
    /// Cell the pointer is being placed in, drawn with emphasis.
    pub highlight: Option<Rect>,
    /// Selected point, drawn as a marker. `None` until a cell is chosen.
    pub pointer: Option<(i64, i64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridState {
    Inactive,
    Level1,
    Level2 {
        parent: Rect,
    },
    Nudging {
        parent: Rect,
        current_pos: (i64, i64),
        held_key: LogicalKey,
    },
}

#[derive(Debug)]
pub struct GridNavigator {
    monitors: Vec<Rect>,
    active_monitor: usize,
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
    StartDrag(i64, i64),
    EnterFreeMode,
}

impl GridNavigator {
    pub fn new(screen_width: i64, screen_height: i64, config: GridConfig) -> Self {
        Self::with_monitors(
            vec![Rect::new(0, 0, screen_width, screen_height)],
            (0, 0),
            config,
        )
    }

    /// Grid over the monitor holding `cursor`; falls back to the first monitor.
    pub fn with_monitors(monitors: Vec<Rect>, cursor: (i64, i64), config: GridConfig) -> Self {
        let mut key_to_cell = HashMap::new();
        for (idx, &k) in config.keys.iter().enumerate() {
            let row = idx as u32 / config.cols.max(1);
            let col = idx as u32 % config.cols.max(1);
            key_to_cell.insert(k, (row, col));
        }
        let mut nav = Self {
            monitors,
            active_monitor: 0,
            config,
            state: GridState::Inactive,
            key_to_cell,
        };
        nav.select_monitor_at(cursor);
        nav
    }

    pub fn active_monitor(&self) -> Rect {
        self.monitors
            .get(self.active_monitor)
            .copied()
            .unwrap_or(Rect::new(0, 0, 0, 0))
    }

    pub fn select_monitor_at(&mut self, cursor: (i64, i64)) -> bool {
        if let Some(idx) = self
            .monitors
            .iter()
            .position(|m| m.contains(cursor.0, cursor.1))
        {
            self.active_monitor = idx;
            true
        } else {
            false
        }
    }

    pub fn state(&self) -> GridState {
        self.state
    }

    pub fn activate(&mut self) -> GridNavAction {
        self.state = GridState::Level1;
        GridNavAction::ShowOverlayLevel1
    }

    /// Re-selects the monitor under the cursor, then activates the grid.
    pub fn activate_at(&mut self, cursor: (i64, i64)) -> GridNavAction {
        self.select_monitor_at(cursor);
        self.activate()
    }

    /// Overlay description for the current level, or none when inactive.
    pub fn overlay_frame(&self) -> Option<OverlayFrame> {
        let (level, area) = match self.state {
            GridState::Inactive => return None,
            GridState::Level1 => (1, self.active_monitor()),
            GridState::Level2 { parent } => (2, parent),
            GridState::Nudging { parent, .. } => (2, parent),
        };
        let mut cells = Vec::with_capacity(self.key_to_cell.len());
        for (idx, key) in self.config.keys.iter().enumerate() {
            let row = idx as u32 / self.config.cols.max(1);
            let col = idx as u32 % self.config.cols.max(1);
            if row >= self.config.rows || col >= self.config.cols {
                continue;
            }
            cells.push(OverlayCell {
                rect: area.subcell(row, col, self.config.rows, self.config.cols),
                label: key.label().to_string(),
            });
        }
        Some(OverlayFrame {
            level,
            cells,
            highlight: self.active_cell(),
            pointer: None,
        })
    }

    /// The cell the current level-2 selection or nudge sits inside.
    pub fn active_cell(&self) -> Option<Rect> {
        match self.state {
            GridState::Level2 { parent } => Some(parent),
            GridState::Nudging { parent, .. } => Some(parent),
            _ => None,
        }
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
                    let cell =
                        self.active_monitor()
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
                            parent,
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
                parent,
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
                        parent,
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
                ..
            } if key == held_key => {
                self.state = GridState::Inactive;
                if self.config.drag_after_select {
                    Some(GridNavAction::StartDrag(current_pos.0, current_pos.1))
                } else if self.config.auto_free_mode_after_move {
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
    fn t04_subcell_partition_covers_full_rect_without_gaps() {
        let screen = Rect::new(0, 0, 1000, 1000);
        let first = screen.subcell(0, 0, 3, 3);
        let last = screen.subcell(2, 2, 3, 3);
        assert_eq!((first.x, first.width), (0, 333));
        assert_eq!((last.x, last.width), (666, 334));
        assert_eq!(last.x + last.width, screen.x + screen.width);
        assert_eq!(last.y + last.height, screen.y + screen.height);
    }

    #[test]
    fn t05_monitor_under_cursor_is_used_for_level_one_grid() {
        let monitors = vec![Rect::new(0, 0, 1920, 1080), Rect::new(1920, 0, 2560, 1440)];
        let mut nav =
            GridNavigator::with_monitors(monitors.clone(), (2500, 700), GridConfig::default());
        assert_eq!(nav.active_monitor(), monitors[1]);
        nav.activate();
        let act = nav.on_key_press(LogicalKey::K).unwrap();
        match act {
            GridNavAction::ShowOverlayLevel2(r) => {
                assert_eq!(r.x, 2773);
                assert_eq!(r.y, 480);
                assert_eq!(r.width, 853);
                assert_eq!(r.height, 480);
            }
            other => panic!("expected ShowOverlayLevel2, got {other:?}"),
        }
    }

    #[test]
    fn t06_overlay_frame_reports_cells_and_key_labels_per_level() {
        let mut nav = GridNavigator::new(1920, 1080, GridConfig::default());
        assert!(nav.overlay_frame().is_none());

        nav.activate();
        let level1 = nav.overlay_frame().unwrap();
        assert_eq!(level1.level, 1);
        assert_eq!(level1.cells.len(), 9);
        assert_eq!(level1.cells[0].label, "u");
        assert_eq!(level1.cells[0].rect, Rect::new(0, 0, 640, 360));
        assert_eq!(level1.cells[8].label, ".");

        nav.on_key_press(LogicalKey::K);
        let level2 = nav.overlay_frame().unwrap();
        assert_eq!(level2.level, 2);
        assert_eq!(level2.cells[4].label, "k");
        assert_eq!(level2.cells[4].rect, Rect::new(853, 480, 213, 120));
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
