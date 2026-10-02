use crate::LogicalKey;
use std::collections::HashMap;

/// Smallest nested cell width that still shows a one-character label at
/// glyph scale 2. The shared rasterizer reserves 4 px of padding inside
/// every cell and draws a 5x7 glyph, so a doubled glyph (10 px wide) needs
/// 16 px of cell.
pub const MIN_NESTED_CELL_W: i64 = 16;
/// Smallest nested cell height for the same scale-2 label: a doubled 7 px
/// glyph plus the 4 px of padding.
pub const MIN_NESTED_CELL_H: i64 = 18;
/// Smallest free area worth drawing help into. Below this the text would be
/// clipped, so help is withheld rather than drawn half off-screen.
pub const MIN_HELP_W: i64 = 160;
/// Smallest free area height for help, tall enough for three lines.
pub const MIN_HELP_H: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridConfig {
    pub dense: bool,
    pub column_keys: Vec<LogicalKey>,
    pub row_keys: Vec<LogicalKey>,
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
            dense: false,
            column_keys: vec![],
            row_keys: vec![],
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

impl GridConfig {
    /// Simple two-key selection: outer cell, inner cell, immediate click.
    pub fn simple() -> Self {
        Self {
            nudge_enabled: false,
            auto_free_mode_after_move: false,
            ..Self::default()
        }
    }

    pub fn dense() -> Self {
        use LogicalKey::*;
        Self {
            dense: true,
            column_keys: vec![A, S, D, F, G, H, J, K, L, Semicolon],
            row_keys: vec![
                Q, W, E, R, T, Y, U, I, O, P, A, S, D, F, G, H, J, K, L, Semicolon, Z, X, C, V, B,
                N, M, Comma, Dot, Slash,
            ],
            rows: 3,
            cols: 10,
            keys: vec![
                Q, W, E, R, T, Y, U, I, O, P, A, S, D, F, G, H, J, K, L, Semicolon, Z, X, C, V, B,
                N, M, Comma, Dot, Slash,
            ],
            auto_free_mode_after_move: false,
            nudge_step_px: 1,
            ..Self::default()
        }
    }

    /// Nested rows and columns that keep every one-character label at
    /// glyph scale 2 inside `area`. The configured grid is used whenever it
    /// fits; each axis is clamped independently to what the area can carry
    /// at the minimum legible cell size, so a small display loses targets
    /// instead of drawing labels too small to read. Both axes stay at 1.
    pub fn nested_layout(&self, area: Rect) -> (u32, u32) {
        let rows = ((area.height / MIN_NESTED_CELL_H).clamp(1, i64::from(self.rows))) as u32;
        let cols = ((area.width / MIN_NESTED_CELL_W).clamp(1, i64::from(self.cols))) as u32;
        (rows, cols)
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

/// Keyboard help for the active grid, placed in screen space the cells do
/// not cover. Lines are plain text; renderers pick their own presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayHelp {
    pub rect: Rect,
    pub lines: Vec<String>,
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
    /// Recovery and mode keys, when the cells leave room for them.
    pub help: Option<OverlayHelp>,
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
impl GridNavigator {
    /// The active grid configuration, for callers that need to re-seed an
    /// editor from live state.
    pub fn config(&self) -> &GridConfig {
        &self.config
    }
}

#[derive(Debug)]
pub struct GridNavigator {
    prefix: Option<usize>,
    selection_held: Option<LogicalKey>,
    monitors: Vec<Rect>,
    active_monitor: usize,
    config: GridConfig,
    state: GridState,
    /// Position of each nested key in reading order over the nested grid.
    /// The cell itself depends on the effective nested layout, which shrinks
    /// on small displays, so the index is stored rather than a (row, col).
    key_index: HashMap<LogicalKey, usize>,
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
    EnterFreeMode(i64, i64),
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
        let mut key_index = HashMap::new();
        for (idx, &k) in config.keys.iter().enumerate() {
            key_index.insert(k, idx);
        }
        let mut nav = Self {
            prefix: None,
            selection_held: None,
            monitors,
            active_monitor: 0,
            config,
            state: GridState::Inactive,
            key_index,
        };
        nav.select_monitor_at(cursor);
        nav
    }

    /// Effective nested rows and columns for a selection area. Clamps the
    /// configured subgrid to what the area can show legibly.
    fn nested_dims(&self, area: Rect) -> (u32, u32) {
        self.config.nested_layout(area)
    }

    /// The subcell a nested key picks inside `area`, or `None` when the key
    /// falls outside the shrunken subgrid on a small display.
    fn nested_cell(&self, area: Rect, key: LogicalKey) -> Option<Rect> {
        let index = *self.key_index.get(&key)?;
        let (rows, cols) = self.nested_dims(area);
        let index = index as u32;
        if index >= rows * cols {
            return None;
        }
        Some(area.subcell(index / cols, index % cols, rows, cols))
    }

    /// Recovery and mode keys for the current level, in words the 5x7 overlay
    /// font can draw. Every line is derived from live state, never a fixed
    /// string: the configured release mode decides whether a selected key
    /// drags, returns to pointer mode, or clicks.
    fn help_lines(&self) -> Vec<String> {
        let mut lines = vec!["bksp back".to_string(), "esc cancel".to_string()];
        if self.state == GridState::Inactive {
            lines.clear();
            return lines;
        }
        // What releasing the final key will do, from the active config.
        let release = if self.config.drag_after_select {
            "release to drag"
        } else if self.config.auto_free_mode_after_move {
            "release to move"
        } else {
            "release to click"
        };
        lines.push(release.to_string());
        match self.state {
            GridState::Level1 => lines.push("press a label key".to_string()),
            GridState::Level2 { .. } | GridState::Nudging { .. } => {
                if self.config.dense {
                    lines.push("space click center".to_string());
                }
                if matches!(self.state, GridState::Nudging { .. }) {
                    lines.push("arrows nudge".to_string());
                }
            }
            GridState::Inactive => lines.clear(),
        }
        lines
    }

    /// Screen area the current cells leave unused, or `None` when the cells
    /// cover the monitor. The dense level-1 view tiles all 300 cells across
    /// the screen, so help is suppressed there; once a key narrows the view,
    /// the rest of the monitor is free and becomes the help area.
    fn free_area(&self, cells: &[OverlayCell]) -> Option<Rect> {
        let monitor = self.active_monitor();
        if monitor.width <= 0 || monitor.height <= 0 {
            return None;
        }
        let mut left = monitor.x + monitor.width;
        let mut right = monitor.x;
        let mut top = monitor.y + monitor.height;
        let mut bottom = monitor.y;
        for cell in cells {
            left = left.min(cell.rect.x);
            right = right.max(cell.rect.x + cell.rect.width);
            top = top.min(cell.rect.y);
            bottom = bottom.max(cell.rect.y + cell.rect.height);
        }
        let free_below = monitor.y + monitor.height - bottom;
        let band = if bottom - top < right - left {
            // Cells are a wide, short band: the free space is above or below.
            if top - monitor.y >= free_below {
                Rect::new(monitor.x, monitor.y, monitor.width, top - monitor.y)
            } else {
                Rect::new(monitor.x, bottom, monitor.width, free_below)
            }
        } else if left - monitor.x >= monitor.x + monitor.width - right {
            Rect::new(monitor.x, monitor.y, left - monitor.x, monitor.height)
        } else {
            let width = monitor.x + monitor.width - right;
            Rect::new(right, monitor.y, width, monitor.height)
        };
        (band.width >= MIN_HELP_W && band.height >= MIN_HELP_H).then_some(band)
    }

    /// The active monitor, or a zero rect when the list shrank or the display
    /// reported no area. Callers must treat a zero rect as "no grid".
    pub fn active_monitor(&self) -> Rect {
        self.monitors
            .get(self.active_monitor)
            .copied()
            .unwrap_or(Rect::new(0, 0, 0, 0))
    }

    /// True when the active monitor has usable area. A removed display, a
    /// resize to nothing, or a stale index all make this false, and the grid
    /// must not present or accept keys rather than aiming at (0,0).
    pub fn has_usable_monitor(&self) -> bool {
        let monitor = self.active_monitor();
        monitor.width > 0 && monitor.height > 0
    }

    /// Re-points the navigator at a new monitor list, as after a display
    /// change. The cursor picks the active monitor when it sits on one, and
    /// the previous index is kept otherwise as long as it still exists, so a
    /// grid in progress does not jump to another display. A stale index is
    /// clamped. Returns whether a usable monitor is now active.
    pub fn set_monitors(&mut self, monitors: Vec<Rect>, cursor: (i64, i64)) -> bool {
        let previous = self.active_monitor;
        self.monitors = monitors;
        if self.select_monitor_at(cursor) {
            return self.has_usable_monitor();
        }
        self.active_monitor = if previous < self.monitors.len() {
            previous
        } else {
            0
        };
        self.has_usable_monitor()
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
        self.prefix = None;
        self.selection_held = None;
        // A removed or zero-sized display must not produce a grid: every cell
        // would be empty and a key press would aim at the screen corner.
        if !self.has_usable_monitor() {
            self.state = GridState::Inactive;
            return GridNavAction::HideOverlay;
        }
        self.state = GridState::Level1;
        GridNavAction::ShowOverlayLevel1
    }

    /// Re-selects the monitor under the cursor, then activates the grid.
    pub fn activate_at(&mut self, cursor: (i64, i64)) -> GridNavAction {
        self.select_monitor_at(cursor);
        self.activate()
    }

    fn dense_cells(&self, selected: Option<Rect>) -> Vec<OverlayCell> {
        let mut cells = Vec::new();
        for (col, first) in self.config.column_keys.iter().enumerate() {
            if self.state == GridState::Level1 && self.prefix.is_some_and(|prefix| prefix != col) {
                continue;
            }
            for (row, second) in self.config.row_keys.iter().enumerate() {
                let rect = self.active_monitor().subcell(
                    row as u32,
                    col as u32,
                    self.config.row_keys.len() as u32,
                    self.config.column_keys.len() as u32,
                );
                if Some(rect) != selected {
                    cells.push(OverlayCell {
                        rect,
                        label: format!("{}{}", first.label(), second.label()),
                    });
                }
            }
        }
        cells
    }

    /// Shows the active selection level without unrelated outer labels.
    pub fn overlay_frame(&self) -> Option<OverlayFrame> {
        if self.config.dense && self.state == GridState::Level1 {
            let cells = self.dense_cells(None);
            let help = self.build_help(&cells);
            return Some(OverlayFrame {
                level: 1,
                cells,
                highlight: None,
                pointer: None,
                help,
            });
        }
        let (level, area) = match self.state {
            GridState::Inactive => return None,
            GridState::Level1 => (1, self.active_monitor()),
            GridState::Level2 { parent } => (2, parent),
            GridState::Nudging { parent, .. } => (2, parent),
        };
        // A display that vanished mid-selection leaves nothing to draw.
        if area.width <= 0 || area.height <= 0 {
            return None;
        }
        let mut cells = Vec::with_capacity(self.key_index.len());
        let (rows, cols) = self.nested_dims(area);
        for (idx, key) in self.config.keys.iter().enumerate() {
            let index = idx as u32;
            if index >= rows * cols {
                continue;
            }
            cells.push(OverlayCell {
                rect: area.subcell(index / cols, index % cols, rows, cols),
                label: key.label().to_string(),
            });
        }
        let help = self.build_help(&cells);
        Some(OverlayFrame {
            level,
            cells,
            highlight: self.active_cell(),
            pointer: match self.state {
                GridState::Nudging { current_pos, .. } => Some(current_pos),
                GridState::Level2 { parent } if self.config.dense => Some(parent.center()),
                _ => None,
            },
            help,
        })
    }

    /// Help for the current level, or `None` when the cells leave no usable
    /// free area. The dense 300-cell view covers the monitor, so it carries no
    /// help; once a key narrows the view the rest of the screen is free.
    fn build_help(&self, cells: &[OverlayCell]) -> Option<OverlayHelp> {
        let lines = self.help_lines();
        if lines.is_empty() {
            return None;
        }
        let rect = self.free_area(cells)?;
        Some(OverlayHelp { rect, lines })
    }

    /// The cell the current level-2 selection or nudge sits inside.
    pub fn active_cell(&self) -> Option<Rect> {
        match self.state {
            GridState::Level2 { parent } => Some(parent),
            GridState::Nudging {
                parent, held_key, ..
            } => {
                let cell = self.nested_cell(parent, held_key)?;
                Some(cell)
            }
            _ => None,
        }
    }

    pub fn deactivate(&mut self) -> GridNavAction {
        self.state = GridState::Inactive;
        GridNavAction::HideOverlay
    }

    /// True when the grid owns this key at any level (labels, outer banks).
    /// Hooks use it to keep repeats and releases of grid keys suppressed.
    pub fn is_grid_key(&self, key: LogicalKey) -> bool {
        self.key_index.contains_key(&key)
            || self.config.column_keys.contains(&key)
            || self.config.row_keys.contains(&key)
    }

    pub fn on_key_press(&mut self, key: LogicalKey) -> Option<GridNavAction> {
        if key == LogicalKey::Esc && self.state != GridState::Inactive {
            return Some(self.deactivate());
        }
        // With no usable monitor there is nothing to aim at, so the grid stays
        // shut instead of selecting a zero-sized cell.
        if !self.has_usable_monitor() {
            self.state = GridState::Inactive;
            return Some(GridNavAction::HideOverlay);
        }
        if self.config.dense {
            if self.selection_held == Some(key) && !matches!(self.state, GridState::Nudging { .. })
            {
                return None;
            }
            self.selection_held = Some(key);
            if key == LogicalKey::Backspace {
                return match self.state {
                    GridState::Level1 => {
                        self.prefix = None;
                        Some(GridNavAction::ShowOverlayLevel1)
                    }
                    GridState::Level2 { .. } => {
                        self.state = GridState::Level1;
                        Some(GridNavAction::ShowOverlayLevel1)
                    }
                    GridState::Nudging { parent, .. } => {
                        self.state = GridState::Level2 { parent };
                        let (x, y) = parent.center();
                        Some(GridNavAction::MoveCursorTo(x, y))
                    }
                    GridState::Inactive => None,
                };
            }
            if key == LogicalKey::Space {
                let target = match self.state {
                    GridState::Level2 { parent } => Some(parent.center()),
                    GridState::Nudging { current_pos, .. } => Some(current_pos),
                    _ => None,
                };
                if let Some((x, y)) = target {
                    self.deactivate();
                    return Some(GridNavAction::ClickAt(x, y));
                }
            }
            if self.state == GridState::Level1 {
                if let Some(col) = self.prefix {
                    let row = self
                        .config
                        .row_keys
                        .iter()
                        .position(|&candidate| candidate == key)?;
                    let parent = self.active_monitor().subcell(
                        row as u32,
                        col as u32,
                        self.config.row_keys.len() as u32,
                        self.config.column_keys.len() as u32,
                    );
                    self.state = GridState::Level2 { parent };
                    let (x, y) = parent.center();
                    return Some(GridNavAction::MoveCursorTo(x, y));
                }
                self.prefix = self
                    .config
                    .column_keys
                    .iter()
                    .position(|&candidate| candidate == key);
                return self.prefix.map(|_| GridNavAction::ShowOverlayLevel1);
            }
        }
        match self.state {
            GridState::Inactive => None,
            GridState::Level1 => {
                if let Some(cell) = self.nested_cell(self.active_monitor(), key) {
                    self.state = GridState::Level2 { parent: cell };
                    Some(GridNavAction::ShowOverlayLevel2(cell))
                } else if key == LogicalKey::Esc {
                    Some(self.deactivate())
                } else {
                    None
                }
            }
            GridState::Level2 { parent } => {
                if let Some(subcell) = self.nested_cell(parent, key) {
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
                        Some(GridNavAction::EnterFreeMode(target.0, target.1))
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
                if key == LogicalKey::Space {
                    self.deactivate();
                    return Some(GridNavAction::ClickAt(current_pos.0, current_pos.1));
                }
                if key == held_key {
                    return None;
                }
                let step = self.config.nudge_step_px;
                let delta = match key {
                    LogicalKey::H | LogicalKey::A => Some((-step, 0)),
                    LogicalKey::J | LogicalKey::S => Some((0, step)),
                    LogicalKey::K | LogicalKey::W => Some((0, -step)),
                    LogicalKey::L | LogicalKey::D => Some((step, 0)),
                    LogicalKey::ArrowLeft => Some((-step, 0)),
                    LogicalKey::ArrowDown => Some((0, step)),
                    LogicalKey::ArrowUp => Some((0, -step)),
                    LogicalKey::ArrowRight => Some((step, 0)),
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
        if self.selection_held == Some(key) {
            self.selection_held = None;
        }
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
                    Some(GridNavAction::EnterFreeMode(current_pos.0, current_pos.1))
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
        assert_eq!(act, GridNavAction::EnterFreeMode(959, 540));
    }

    /// Ticket 046: a nested label is only readable at glyph scale 2, so the
    /// subgrid shrinks until every cell clears the legible minimum. The
    /// level-1 grid itself is never touched.
    #[test]
    fn nested_subgrid_shrinks_until_every_label_is_legible() {
        let config = GridConfig::dense();
        for (width, height) in [
            (1920i64, 1080i64),
            (2880, 1620),
            (3840, 2160),
            (1366, 768),
            (1280, 720),
        ] {
            let mut nav = GridNavigator::new(width, height, config.clone());
            nav.activate();
            let level1 = nav.overlay_frame().unwrap();
            assert_eq!(
                level1.cells.len(),
                300,
                "level 1 must keep the full 30x10 grid at {width}x{height}"
            );
            for cell in &level1.cells {
                assert!(cell.rect.width >= MIN_NESTED_CELL_W * 2);
                assert!(cell.rect.height >= MIN_NESTED_CELL_H);
            }

            nav.on_key_press(LogicalKey::K);
            nav.on_key_release(LogicalKey::K);
            nav.on_key_press(LogicalKey::K);
            let level2 = nav.overlay_frame().unwrap();
            let nested: Vec<_> = level2
                .cells
                .iter()
                .filter(|c| c.label.chars().count() == 1)
                .collect();
            assert!(!nested.is_empty(), "no nested cells at {width}x{height}");
            for cell in &nested {
                assert!(
                    cell.rect.width >= MIN_NESTED_CELL_W && cell.rect.height >= MIN_NESTED_CELL_H,
                    "nested cell {:?} is below the legible minimum at {width}x{height}",
                    cell.rect
                );
            }
        }
    }

    /// The clamped subgrid is a real grid: cells tile the parent in reading
    /// order, and every visible key selects the cell it is drawn in.
    #[test]
    fn shrunken_nested_keys_select_the_cell_they_are_drawn_in() {
        for (width, height) in [(1920i64, 1080i64), (2880, 1620), (1366, 768), (1280, 720)] {
            let mut nav = GridNavigator::new(width, height, GridConfig::dense());
            nav.activate();
            nav.on_key_press(LogicalKey::K);
            nav.on_key_release(LogicalKey::K);
            nav.on_key_press(LogicalKey::K);
            let nested: Vec<_> = nav
                .overlay_frame()
                .unwrap()
                .cells
                .into_iter()
                .filter(|c| c.label.chars().count() == 1)
                .collect();

            // Reading order: within a row each cell starts where the
            // previous one ended, and a new row restarts at the first x.
            let first_x = nested[0].rect.x;
            for pair in nested.windows(2) {
                let expected = if pair[1].rect.x < pair[0].rect.x {
                    first_x
                } else {
                    pair[0].rect.x + pair[0].rect.width
                };
                assert_eq!(
                    pair[1].rect.x, expected,
                    "nested cells do not tile in reading order at {width}x{height}"
                );
            }
            // The subgrid tiles its bounding box exactly: the summed cell
            // area equals the box, so no gap or overlap is left behind.
            let left = nested.iter().map(|c| c.rect.x).min().unwrap();
            let top = nested.iter().map(|c| c.rect.y).min().unwrap();
            let right = nested
                .iter()
                .map(|c| c.rect.x + c.rect.width)
                .max()
                .unwrap();
            let bottom = nested
                .iter()
                .map(|c| c.rect.y + c.rect.height)
                .max()
                .unwrap();
            let box_area = (right - left) * (bottom - top);
            let cell_area: i64 = nested.iter().map(|c| c.rect.width * c.rect.height).sum();
            assert_eq!(
                cell_area, box_area,
                "subgrid leaves gaps or overlaps at {width}x{height}"
            );
            // And the box is the level-1 cell that was selected.
            let parent = nav.state();
            if let GridState::Level2 { parent } = parent {
                assert_eq!(
                    (left, top, right - left, bottom - top),
                    (parent.x, parent.y, parent.width, parent.height),
                    "subgrid does not span the selected cell at {width}x{height}"
                );
            }

            for cell in nested {
                let key = dense_key_for(cell.label.chars().next().unwrap());
                let mut fresh = GridNavigator::new(width, height, GridConfig::dense());
                fresh.activate();
                fresh.on_key_press(LogicalKey::K);
                fresh.on_key_release(LogicalKey::K);
                fresh.on_key_press(LogicalKey::K);
                fresh.on_key_release(LogicalKey::K);
                assert_eq!(
                    fresh.on_key_press(key),
                    Some(GridNavAction::MoveCursorTo(
                        cell.rect.x + cell.rect.width / 2,
                        cell.rect.y + cell.rect.height / 2,
                    )),
                    "key {} did not select its own cell at {width}x{height}",
                    cell.label
                );
            }
        }
    }

    /// A key the shrunken subgrid no longer shows must not select anything
    /// and must not change the grid state.
    #[test]
    fn nested_key_outside_the_shrunken_grid_is_ignored() {
        let mut nav = GridNavigator::new(1920, 1080, GridConfig::dense());
        nav.activate();
        nav.on_key_press(LogicalKey::K);
        nav.on_key_release(LogicalKey::K);
        nav.on_key_press(LogicalKey::K);
        let before = nav.overlay_frame().unwrap();
        assert_eq!(
            before
                .cells
                .iter()
                .filter(|c| c.label.chars().count() == 1)
                .count(),
            20
        );
        // Slash is the last key of the 30-key bank, outside the 2x10 clamp.
        assert_eq!(nav.on_key_press(LogicalKey::Slash), None);
        assert_eq!(nav.overlay_frame().unwrap(), before);
    }

    /// Ticket 047: the dense 300-cell view covers the whole monitor, so it
    /// carries no help. Once a key narrows the view, the freed screen space
    /// carries the recovery keys the user needs at that level.
    #[test]
    fn help_appears_once_a_key_frees_screen_space() {
        for (width, height) in [(1920i64, 1080i64), (2880, 1620), (1366, 768)] {
            let mut nav = GridNavigator::new(width, height, GridConfig::dense());
            nav.activate();
            let level1 = nav.overlay_frame().unwrap();
            assert_eq!(level1.cells.len(), 300);
            assert!(
                level1.help.is_none(),
                "the full 300-cell view has no free space for help at {width}x{height}"
            );

            // One outer key narrows to a single column bank, freeing the rest.
            nav.on_key_press(LogicalKey::K);
            let bank = nav.overlay_frame().unwrap();
            let help = bank.help.as_ref().expect("narrowed view has room for help");
            assert!(
                help.lines.iter().any(|l| l.contains("bksp")),
                "back instruction missing at {width}x{height}: {:?}",
                help.lines
            );
            assert!(
                help.lines.iter().any(|l| l.contains("esc")),
                "cancel instruction missing at {width}x{height}: {:?}",
                help.lines
            );
            // The help box must not sit on top of any cell.
            for cell in &bank.cells {
                let overlap = cell.rect.x < help.rect.x + help.rect.width
                    && help.rect.x < cell.rect.x + cell.rect.width
                    && cell.rect.y < help.rect.y + help.rect.height
                    && help.rect.y < cell.rect.y + cell.rect.height;
                assert!(!overlap, "help covers a cell at {width}x{height}");
            }
        }
    }

    /// Ticket 047: the help must describe the mode that is actually active,
    /// not a fixed string. Drag, free mode and click differ per config.
    #[test]
    fn help_follows_the_active_press_mode() {
        let bank_help = |drag: bool, auto_free: bool| {
            let mut nav = GridNavigator::new(
                1920,
                1080,
                GridConfig {
                    drag_after_select: drag,
                    auto_free_mode_after_move: auto_free,
                    ..GridConfig::dense()
                },
            );
            nav.activate();
            nav.on_key_press(LogicalKey::K);
            nav.overlay_frame().unwrap().help.unwrap().lines
        };
        assert!(bank_help(true, false).iter().any(|l| l.contains("drag")));
        assert!(bank_help(false, true).iter().any(|l| l.contains("move")));
        assert!(bank_help(false, false).iter().any(|l| l.contains("click")));
    }

    /// Ticket 048: a display that disappears must not leave a grid that aims
    /// at the screen corner. Before this, a removed monitor left a zero-size
    /// active rect, 300 empty cells, and a key press targeting (0,0).
    #[test]
    fn removed_display_stops_the_grid_instead_of_aiming_at_the_corner() {
        let mut nav = GridNavigator::with_monitors(
            vec![Rect::new(0, 0, 1920, 1080), Rect::new(1920, 0, 1280, 720)],
            (2500, 400),
            GridConfig::dense(),
        );
        assert_eq!(nav.active_monitor(), Rect::new(1920, 0, 1280, 720));
        nav.activate();

        // The second display is unplugged; the cursor still sits where it was.
        let usable = nav.set_monitors(vec![Rect::new(0, 0, 1920, 1080)], (2500, 400));
        assert!(usable, "the remaining display is usable");
        // The stale index is clamped to a real monitor, not left dangling.
        assert_eq!(nav.active_monitor(), Rect::new(0, 0, 1920, 1080));
        assert!(nav.has_usable_monitor());

        // Unplug the last display too.
        assert!(!nav.set_monitors(vec![], (2500, 400)));
        assert!(!nav.has_usable_monitor());
        assert_eq!(nav.activate(), GridNavAction::HideOverlay);
        assert_eq!(nav.state(), GridState::Inactive);
        assert!(nav.overlay_frame().is_none());
        // No key may resolve to a target while there is no display.
        assert_eq!(
            nav.on_key_press(LogicalKey::K),
            Some(GridNavAction::HideOverlay)
        );
        assert!(nav.overlay_frame().is_none());
    }

    /// A display that reports no area, as on a resolution change caught
    /// mid-transition, must not produce a grid either.
    #[test]
    fn zero_sized_display_never_produces_a_grid() {
        let mut nav = GridNavigator::new(0, 0, GridConfig::dense());
        assert!(!nav.has_usable_monitor());
        assert_eq!(nav.activate(), GridNavAction::HideOverlay);
        assert!(nav.overlay_frame().is_none());
        assert_eq!(
            nav.on_key_press(LogicalKey::K),
            Some(GridNavAction::HideOverlay)
        );
    }

    /// Re-arming keeps the grid on the display it was already using when the
    /// cursor sits in a gap, so a selection in progress does not jump screens.
    #[test]
    fn rearm_keeps_the_current_display_when_the_cursor_is_in_a_gap() {
        let mut nav = GridNavigator::with_monitors(
            vec![Rect::new(0, 0, 1920, 1080), Rect::new(4000, 0, 1280, 720)],
            (4500, 300),
            GridConfig::dense(),
        );
        assert_eq!(nav.active_monitor().x, 4000);
        // Cursor moves into the gap between the two displays.
        let usable = nav.set_monitors(
            vec![Rect::new(0, 0, 1920, 1080), Rect::new(4000, 0, 1280, 720)],
            (2500, 300),
        );
        assert!(usable);
        assert_eq!(
            nav.active_monitor().x,
            4000,
            "the grid must stay on the display it was using"
        );
        // Now the cursor is on the first display, so the grid follows it.
        nav.set_monitors(
            vec![Rect::new(0, 0, 1920, 1080), Rect::new(4000, 0, 1280, 720)],
            (100, 100),
        );
        assert_eq!(nav.active_monitor().x, 0);
    }

    /// The logical key behind a one-character dense label.
    fn dense_key_for(label: char) -> LogicalKey {
        use LogicalKey::*;
        const BANK: [LogicalKey; 30] = [
            Q, W, E, R, T, Y, U, I, O, P, A, S, D, F, G, H, J, K, L, Semicolon, Z, X, C, V, B, N,
            M, Comma, Dot, Slash,
        ];
        BANK.iter()
            .copied()
            .find(|key| key.label() == label.to_string())
            .unwrap_or_else(|| panic!("no dense key for label {label}"))
    }
}
