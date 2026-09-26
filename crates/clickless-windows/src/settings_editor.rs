//! Pure Settings editor semantics (Prompt 3).
//!
//! No Win32 here: this module turns the window's raw control strings into a
//! `Config`, validates it, and decides what may reach the runtime or disk.
//! `SettingsEditor` is the state machine the native window drives; Win32
//! code in `settings::win` only reads control text and calls these methods.

use clickless_config::{
    Config, ConfigError, ThemeConfig, parse_action, parse_hex_rgb, parse_logical_key,
};
use clickless_core::{Action, LogicalKey};
use std::collections::HashMap;

/// Raw control values, exactly as the window holds them. Parsing happens in
/// `apply_fields`, the single doorway into a `Config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fields {
    pub enabled: bool,
    pub leader: String,
    pub start_speed_px_s: String,
    pub max_speed_px_s: String,
    pub ramp_ms: String,
    pub hold_ms: String,
    pub scroll_step: String,
    pub layout: String,
    pub mouse_bindings: Vec<(String, String)>,
    pub grid_rows: String,
    pub grid_cols: String,
    pub grid_keys: Vec<String>,
    pub column_keys: Vec<String>,
    pub row_keys: Vec<String>,
    pub nudge_enabled: bool,
    pub nudge_step_px: String,
    pub drag_after_select: bool,
    pub auto_free_mode: bool,
    pub panel: String,
    pub panel_opacity: String,
    pub border: String,
    pub highlight_opacity: String,
    pub border_px: String,
    pub highlight: String,
    pub label: String,
    pub pointer: String,
    pub label_size: String,
}

/// Reads every supported field out of a Config using the same canonical
/// names the TOML export writes.
pub fn fields_from_config(config: &Config) -> Fields {
    Fields {
        enabled: config.enabled,
        leader: clickless_config::logical_key_name(config.settings.leader).to_string(),
        start_speed_px_s: config.settings.start_speed_px_s.to_string(),
        max_speed_px_s: config.settings.max_speed_px_s.to_string(),
        ramp_ms: config.settings.ramp_ms.to_string(),
        hold_ms: config.settings.hold_ms.to_string(),
        scroll_step: config.settings.scroll_step.to_string(),
        layout: if config.grid.dense {
            "dense".into()
        } else {
            "simple".into()
        },
        mouse_bindings: {
            let mut bindings: Vec<_> = config
                .mouse_bindings
                .iter()
                .map(|(key, action)| {
                    (
                        clickless_config::logical_key_name(*key).to_string(),
                        clickless_config::action_name(*action).to_string(),
                    )
                })
                .collect();
            bindings.sort();
            bindings
        },
        grid_rows: config.grid.rows.to_string(),
        grid_cols: config.grid.cols.to_string(),
        grid_keys: config
            .grid
            .keys
            .iter()
            .map(|key| clickless_config::logical_key_name(*key).to_string())
            .collect(),
        column_keys: config
            .grid
            .column_keys
            .iter()
            .map(|key| clickless_config::logical_key_name(*key).to_string())
            .collect(),
        row_keys: config
            .grid
            .row_keys
            .iter()
            .map(|key| clickless_config::logical_key_name(*key).to_string())
            .collect(),
        nudge_enabled: config.grid.nudge_enabled,
        nudge_step_px: config.grid.nudge_step_px.to_string(),
        drag_after_select: config.grid.drag_after_select,
        auto_free_mode: config.grid.auto_free_mode_after_move,
        panel: hex(config.theme.panel),
        panel_opacity: config.theme.panel_opacity.to_string(),
        border: hex(config.theme.border),
        highlight_opacity: config.theme.highlight_opacity.to_string(),
        border_px: config.theme.border_px.to_string(),
        highlight: hex(config.theme.highlight),
        label: hex(config.theme.label),
        pointer: hex(config.theme.pointer),
        label_size: config.theme.label_size.to_string(),
    }
}

fn hex(rgb: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
}

fn parse_u64(name: &str, raw: &str) -> Result<u64, String> {
    raw.trim()
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a whole number, got \"{raw}\""))
}

fn parse_i64(name: &str, raw: &str) -> Result<i64, String> {
    raw.trim()
        .parse::<i64>()
        .map_err(|_| format!("{name} must be a number, got \"{raw}\""))
}

/// The only parser from Fields to Config. Keeps the window free of any
/// duplicate validation; errors name the offending field.
pub fn apply_fields(config: &mut Config, fields: &Fields) -> Result<(), String> {
    config.enabled = fields.enabled;
    config.settings.leader = parse_logical_key(&fields.leader)
        .map_err(|_| format!("leader key \"{}\" is not a known key", fields.leader))?;
    config.settings.start_speed_px_s = parse_u64("start speed", &fields.start_speed_px_s)?;
    config.settings.max_speed_px_s = parse_u64("max speed", &fields.max_speed_px_s)?;
    config.settings.ramp_ms = parse_u64("ramp ms", &fields.ramp_ms)?;
    config.settings.hold_ms = parse_u64("hold ms", &fields.hold_ms)?;
    config.settings.scroll_step = parse_i64("scroll step", &fields.scroll_step)?;

    config.grid.dense = match fields.layout.as_str() {
        "dense" => true,
        "simple" => false,
        other => return Err(format!("layout must be dense or simple, got \"{other}\"")),
    };
    config.mouse_bindings = parse_mouse_bindings(&fields.mouse_bindings)?;
    config.grid.rows = parse_grid_dimension("rows", &fields.grid_rows)?;
    config.grid.cols = parse_grid_dimension("columns", &fields.grid_cols)?;
    config.grid.keys = parse_grid_keys("nested keys", &fields.grid_keys)?;
    if config.grid.dense {
        config.grid.column_keys = if fields.column_keys.is_empty() {
            Config::default().grid.column_keys
        } else {
            parse_grid_keys("column keys", &fields.column_keys)?
        };
        config.grid.row_keys = if fields.row_keys.is_empty() {
            Config::default().grid.row_keys
        } else {
            parse_grid_keys("row keys", &fields.row_keys)?
        };
    } else {
        config.grid.column_keys.clear();
        config.grid.row_keys.clear();
    }
    config.grid.nudge_enabled = fields.nudge_enabled;
    config.grid.nudge_step_px = parse_i64("nudge step", &fields.nudge_step_px)?;
    config.grid.drag_after_select = fields.drag_after_select;
    config.grid.auto_free_mode_after_move = fields.auto_free_mode;
    config.validate().map_err(|error| match error {
        ConfigError::InvalidStartSpeed(value) => {
            format!("start speed must be above 0, got {value}")
        }
        ConfigError::InvalidMaxSpeed(max, start) => {
            format!("max speed {max} must be at least the start speed {start}")
        }
        ConfigError::InvalidRampMs(value) => format!("ramp ms must be above 0, got {value}"),
        ConfigError::InvalidNudgeStep => "nudge step must be above 0".into(),
        ConfigError::DuplicateGridKey => "grid: key appears more than once".into(),
        ConfigError::InvalidGridKeyCount(found, expected) => {
            format!("grid: nested key count is {found}; expected {expected}")
        }
        ConfigError::InvalidGridDimensions(rows, cols) => {
            format!("grid: rows and columns must be above 0, got {rows}x{cols}")
        }
        ConfigError::GridDimensionsOverflow(rows, cols) => {
            format!("grid: dimensions overflow for {rows}x{cols}")
        }
        ConfigError::ReservedKey(key) => {
            format!("bindings: {key} is reserved by the engine and cannot be bound")
        }
        ConfigError::DuplicateBinding(key) => {
            format!("bindings: {key} is bound more than once")
        }
        other => other.to_string(),
    })?;
    let theme = ThemeConfig {
        panel: parse_hex_rgb("panel", &fields.panel).map_err(|e| e.to_string())?,
        panel_opacity: parse_opacity(&fields.panel_opacity)?,
        border: parse_hex_rgb("border", &fields.border).map_err(|e| e.to_string())?,
        highlight_opacity: parse_opacity_named("highlight opacity", &fields.highlight_opacity)?,
        border_px: parse_i64("border width", &fields.border_px)?,
        highlight: parse_hex_rgb("highlight", &fields.highlight).map_err(|e| e.to_string())?,
        label: parse_hex_rgb("label", &fields.label).map_err(|e| e.to_string())?,
        pointer: parse_hex_rgb("pointer", &fields.pointer).map_err(|e| e.to_string())?,
        label_size: parse_i64("label size", &fields.label_size)?,
    };
    config.theme = theme;
    for (reserved, action) in [
        (LogicalKey::Esc, "esc"),
        (LogicalKey::Backspace, "backspace"),
    ] {
        if config.mouse_bindings.contains_key(&reserved) {
            return Err(format!(
                "bindings: {action} is reserved by the engine and cannot be bound"
            ));
        }
    }
    if config.mouse_bindings.contains_key(&config.settings.leader) {
        return Err(format!(
            "bindings: {} is reserved by the engine and cannot be bound",
            clickless_config::logical_key_name(config.settings.leader)
        ));
    }
    let parsed = Config::parse(&config.to_toml()).map_err(|error| match error {
        ConfigError::InvalidGridLayout(message) => format!("grid: {message}"),
        ConfigError::InvalidGridDimensions(rows, cols) => {
            format!("grid: rows and columns must be above 0, got {rows}x{cols}")
        }
        ConfigError::InvalidGridKeyCount(found, expected) => {
            format!("grid: nested key count is {found}; expected {expected}")
        }
        ConfigError::GridDimensionsOverflow(rows, cols) => {
            format!("grid: dimensions overflow for {rows}x{cols}")
        }
        ConfigError::DuplicateGridKey => "grid: key appears more than once".into(),
        ConfigError::ReservedKey(key) => {
            format!("bindings: {key} is reserved by the engine and cannot be bound")
        }
        ConfigError::DuplicateBinding(key) => {
            format!("bindings: {key} is bound more than once")
        }
        other => other.to_string(),
    })?;
    *config = parsed;
    config.validate().map_err(|e| match e {
        ConfigError::InvalidStartSpeed(v) => format!("start speed must be above 0, got {v}"),
        ConfigError::InvalidMaxSpeed(max, start) => {
            format!("max speed {max} must be at least the start speed {start}")
        }
        ConfigError::InvalidRampMs(v) => format!("ramp ms must be above 0, got {v}"),
        ConfigError::InvalidNudgeStep => "nudge step must be above 0".into(),
        ConfigError::ReservedKey(k) => {
            format!("bindings: {k} is reserved by the engine and cannot be bound")
        }
        ConfigError::DuplicateBinding(k) => format!("bindings: {k} is bound more than once"),
        ConfigError::DuplicateGridKey => "grid: key appears more than once".into(),
        ConfigError::InvalidGridKeyCount(found, expected) => {
            format!("grid: nested key count is {found}; expected {expected}")
        }
        ConfigError::InvalidGridDimensions(rows, cols) => {
            format!("grid: rows and columns must be above 0, got {rows}x{cols}")
        }
        ConfigError::GridDimensionsOverflow(rows, cols) => {
            format!("grid: dimensions overflow for {rows}x{cols}")
        }
        ConfigError::InvalidGridLayout(layout) => format!("grid: {layout}"),
        ConfigError::InvalidTheme(m) => format!("theme: {m}"),
        other => other.to_string(),
    })
}

fn parse_mouse_bindings(
    bindings: &[(String, String)],
) -> Result<HashMap<LogicalKey, Action>, String> {
    let mut parsed = HashMap::new();
    for (key_name, action_name) in bindings {
        if key_name.trim().is_empty() {
            continue;
        }
        let key = parse_logical_key(key_name).map_err(|error| format!("bindings: {error}"))?;
        let action = parse_action(action_name).map_err(|error| format!("bindings: {error}"))?;
        if parsed.insert(key, action).is_some() {
            return Err(format!(
                "bindings: {} is bound more than once",
                clickless_config::logical_key_name(key)
            ));
        }
    }
    Ok(parsed)
}

fn parse_grid_dimension(name: &str, raw: &str) -> Result<u32, String> {
    raw.trim()
        .parse::<u32>()
        .map_err(|_| format!("grid: {name} must be a whole number, got \"{raw}\""))
}

fn parse_grid_keys(name: &str, keys: &[String]) -> Result<Vec<LogicalKey>, String> {
    keys.iter()
        .map(|key| parse_logical_key(key).map_err(|error| format!("grid: {name}: {error}")))
        .collect()
}

fn parse_opacity(raw: &str) -> Result<u8, String> {
    parse_opacity_named("panel opacity", raw)
}

fn parse_opacity_named(name: &str, raw: &str) -> Result<u8, String> {
    let value = parse_u64(name, raw)?;
    u8::try_from(value).map_err(|_| format!("{name} must be 0-255, got {value}"))
}

/// A preset-resettable group of the draft. Matches the Settings window
/// groups: Settings covers enabled plus the motion rows, Grid covers layout
/// and every grid row, Appearance covers the theme rows, Bindings covers
/// the mouse binding table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Settings,
    Grid,
    Appearance,
    Bindings,
}

/// Editor state: the last applied Config, the working draft, and dirtiness.
/// The native window mirrors these values into controls.
#[derive(Debug)]
pub struct SettingsEditor {
    applied: Config,
    draft: Config,
    dirty: bool,
}

impl SettingsEditor {
    pub fn new(config: Config) -> Self {
        Self {
            applied: config.clone(),
            draft: config,
            dirty: false,
        }
    }

    /// Loads control values into the draft. Parse errors leave the previous
    /// draft untouched and name the offending field.
    pub fn edit(&mut self, fields: &Fields) -> Result<(), String> {
        let mut config = self.draft.clone();
        apply_fields(&mut config, fields)?;
        self.draft = config;
        self.dirty = self.draft != self.applied;
        Ok(())
    }

    /// Validates the whole draft and publishes it as the applied config.
    /// The caller pushes `applied()` to the runtime afterwards. An invalid
    /// draft changes nothing.
    pub fn apply(&mut self) -> Result<&Config, String> {
        let draft = self.draft.clone();
        self.publish_applied(draft)
    }

    pub fn apply_to_runtime(
        &mut self,
        mut runtime_apply: impl FnMut(&Config) -> Result<(), String>,
    ) -> Result<&Config, String> {
        self.draft.validate().map_err(|e| e.to_string())?;
        runtime_apply(&self.draft)?;
        let draft = self.draft.clone();
        self.publish_applied(draft)
    }

    fn publish_applied(&mut self, config: Config) -> Result<&Config, String> {
        config.validate().map_err(|e| e.to_string())?;
        self.applied = config;
        self.dirty = false;
        Ok(&self.applied)
    }

    /// The config to push to the engine after a successful apply.
    pub fn applied(&self) -> &Config {
        &self.applied
    }

    /// The working draft, for refreshing controls.
    pub fn draft(&self) -> &Config {
        &self.draft
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Writes the applied config through the existing atomic save. A failed
    /// write preserves the old file; the applied config stays authoritative.
    pub fn save<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), clickless_config::ConfigError> {
        self.applied.save_to_file(path)
    }

    pub fn save_after_runtime_apply<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
        runtime_apply: impl FnMut(&Config) -> Result<(), String>,
    ) -> Result<(), String> {
        self.apply_to_runtime(runtime_apply)?;
        self.save(path).map_err(|e| e.to_string())
    }

    /// Cancel and window-close semantics: restore the draft to the last
    /// applied config and clear the dirty flag.
    pub fn cancel(&mut self) {
        self.draft = self.applied.clone();
        self.dirty = false;
    }

    /// Preset reset: replaces the whole draft with factory defaults.
    /// Dirty exactly when the defaults differ from the applied config.
    pub fn reset_all(&mut self) {
        self.draft = Config::default();
        self.dirty = self.draft != self.applied;
    }

    /// Preset reset for one window group; the other groups keep their draft.
    pub fn reset_section(&mut self, section: Section) {
        let defaults = Config::default();
        match section {
            Section::Settings => {
                self.draft.enabled = defaults.enabled;
                self.draft.settings = defaults.settings;
            }
            Section::Grid => self.draft.grid = defaults.grid,
            Section::Appearance => self.draft.theme = defaults.theme,
            Section::Bindings => self.draft.mouse_bindings = defaults.mouse_bindings,
        }
        self.dirty = self.draft != self.applied;
    }
}

/// The leader key set default; re-exported for window layout code.
pub const DEFAULT_LEADER_FIELD: &str = "capslock";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_from_config_uses_canonical_names() {
        let fields = fields_from_config(&Config::default());
        assert_eq!(fields.leader, "capslock");
        assert_eq!(fields.layout, "dense");
        assert_eq!(fields.panel, "181C26");
    }
}
