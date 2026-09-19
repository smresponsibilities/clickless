use clickless_core::grid::GridConfig;
use clickless_core::{Action, LogicalKey};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub leader: LogicalKey,
    pub start_speed_px_s: u64,
    pub max_speed_px_s: u64,
    pub ramp_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            leader: LogicalKey::CapsLock,
            start_speed_px_s: 300,
            max_speed_px_s: 3000,
            ramp_ms: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub settings: Settings,
    pub grid: GridConfig,
    pub initial_bindings: HashMap<LogicalKey, String>,
    pub mouse_bindings: HashMap<LogicalKey, Action>,
}

impl Default for Config {
    fn default() -> Self {
        let mouse_bindings = clickless_core::default_bindings();

        let mut initial_bindings = HashMap::new();
        initial_bindings.insert(LogicalKey::CapsLock, "mouse".to_string());

        Self {
            settings: Settings::default(),
            grid: GridConfig::dense(),
            initial_bindings,
            mouse_bindings,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("Invalid grid layout or selection keys: {0}")]
    InvalidGridLayout(String),
    #[error("TOML syntax error: {0}")]
    ParseError(String),
    #[error("Unknown key name: '{0}'")]
    UnknownKey(String),
    #[error("Unknown action verb: '{0}'")]
    UnknownAction(String),
    #[error("Invalid speed: start_speed_px_s ({0}) must be greater than 0")]
    InvalidStartSpeed(u64),
    #[error(
        "Invalid speed: max_speed_px_s ({0}) must be greater than or equal to start_speed_px_s ({1})"
    )]
    InvalidMaxSpeed(u64, u64),
    #[error("Invalid ramp: ramp_ms ({0}) must be greater than 0")]
    InvalidRampMs(u64),
    #[error("Invalid grid dimensions: rows ({0}) and cols ({1}) must be greater than 0")]
    InvalidGridDimensions(u32, u32),
    #[error("Invalid grid keys: count ({0}) does not match rows * cols ({1})")]
    InvalidGridKeyCount(usize, usize),
    #[error("Grid dimensions overflow: {0} * {1}")]
    GridDimensionsOverflow(u32, u32),
    #[error("Grid keys must be unique")]
    DuplicateGridKey,
    #[error("Grid nudge_step_px must be greater than 0")]
    InvalidNudgeStep,
    #[error("IO error: {0}")]
    IoError(String),
}

#[derive(Debug, Deserialize)]
struct RawTomlConfig {
    settings: Option<RawSettings>,
    grid: Option<RawGrid>,
    layers: Option<RawLayers>,
}

#[derive(Debug, Deserialize)]
struct RawSettings {
    leader: Option<String>,
    start_speed_px_s: Option<u64>,
    max_speed_px_s: Option<u64>,
    ramp_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawGrid {
    layout: Option<String>,
    column_keys: Option<Vec<String>>,
    row_keys: Option<Vec<String>>,
    rows: Option<u32>,
    cols: Option<u32>,
    keys: Option<Vec<String>>,
    auto_free_mode_after_move: Option<bool>,
    nudge_enabled: Option<bool>,
    nudge_step_px: Option<i64>,
    drag_after_select: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawLayers {
    initial: Option<HashMap<String, String>>,
    mouse: Option<HashMap<String, String>>,
}

pub fn parse_logical_key(name: &str) -> Result<LogicalKey, ConfigError> {
    match name.to_ascii_lowercase().as_str() {
        "capslock" | "caps_lock" | "caps" => Ok(LogicalKey::CapsLock),
        "a" => Ok(LogicalKey::A),
        "b" => Ok(LogicalKey::B),
        "c" => Ok(LogicalKey::C),
        "e" => Ok(LogicalKey::E),
        "g" => Ok(LogicalKey::G),
        "n" => Ok(LogicalKey::N),
        "p" => Ok(LogicalKey::P),
        "q" => Ok(LogicalKey::Q),
        "r" => Ok(LogicalKey::R),
        "t" => Ok(LogicalKey::T),
        "v" => Ok(LogicalKey::V),
        "x" => Ok(LogicalKey::X),
        "y" => Ok(LogicalKey::Y),
        "z" => Ok(LogicalKey::Z),
        ";" => Ok(LogicalKey::Semicolon),
        "/" => Ok(LogicalKey::Slash),
        "backspace" => Ok(LogicalKey::Backspace),
        "h" => Ok(LogicalKey::H),
        "j" => Ok(LogicalKey::J),
        "k" => Ok(LogicalKey::K),
        "l" => Ok(LogicalKey::L),
        "u" => Ok(LogicalKey::U),
        "i" => Ok(LogicalKey::I),
        "o" => Ok(LogicalKey::O),
        "f" => Ok(LogicalKey::F),
        "d" => Ok(LogicalKey::D),
        "w" => Ok(LogicalKey::W),
        "s" => Ok(LogicalKey::S),
        "m" => Ok(LogicalKey::M),
        "comma" | "," => Ok(LogicalKey::Comma),
        "dot" | "." => Ok(LogicalKey::Dot),
        "space" => Ok(LogicalKey::Space),
        "esc" | "escape" => Ok(LogicalKey::Esc),
        _ => Err(ConfigError::UnknownKey(name.to_string())),
    }
}

pub fn parse_action(verb: &str) -> Result<Action, ConfigError> {
    match verb.to_ascii_lowercase().as_str() {
        "move_left" | "moveleft" | "left" => Ok(Action::MoveLeft),
        "move_right" | "moveright" | "right" => Ok(Action::MoveRight),
        "move_up" | "moveup" | "up" => Ok(Action::MoveUp),
        "move_down" | "movedown" | "down" => Ok(Action::MoveDown),
        "speed_down" | "speeddown" => Ok(Action::SpeedDown),
        "speed_up" | "speedup" => Ok(Action::SpeedUp),
        "click_left" | "clickleft" | "click" => Ok(Action::ClickLeft),
        "click_right" | "clickright" => Ok(Action::ClickRight),
        "scroll_up" | "scrollup" => Ok(Action::ScrollUp),
        "scroll_down" | "scrolldown" => Ok(Action::ScrollDown),
        "enter_grid" | "entergrid" | "grid" => Ok(Action::EnterGrid),
        _ => Err(ConfigError::UnknownAction(verb.to_string())),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepLinkCommand {
    ToggleOverlay,
    ShowOverlay,
    HideOverlay,
    ToggleFreeMode,
    EnterFreeMode,
    ExitFreeMode,
    ToggleEnabled,
    Settings,
}

pub fn parse_deep_link(url: &str) -> Option<DeepLinkCommand> {
    let stripped = url
        .strip_prefix("mouseless://")
        .or_else(|| url.strip_prefix("clickless://"))?;
    let cmd = stripped.trim_end_matches('/');
    match cmd {
        "toggle-overlay" => Some(DeepLinkCommand::ToggleOverlay),
        "show-overlay" => Some(DeepLinkCommand::ShowOverlay),
        "hide-overlay" => Some(DeepLinkCommand::HideOverlay),
        "toggle-free-mode" => Some(DeepLinkCommand::ToggleFreeMode),
        "enter-free-mode" => Some(DeepLinkCommand::EnterFreeMode),
        "exit-free-mode" => Some(DeepLinkCommand::ExitFreeMode),
        "toggle-enabled" => Some(DeepLinkCommand::ToggleEnabled),
        "settings" => Some(DeepLinkCommand::Settings),
        _ => None,
    }
}

impl Config {
    pub fn parse(toml_str: &str) -> Result<Self, ConfigError> {
        if toml_str.trim().is_empty() {
            return Ok(Self::default());
        }

        let raw: RawTomlConfig =
            toml::from_str(toml_str).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        let mut config = Self::default();

        if let Some(s) = raw.settings {
            if let Some(leader_str) = s.leader {
                config.settings.leader = parse_logical_key(&leader_str)?;
            }
            if let Some(start_speed) = s.start_speed_px_s {
                if start_speed == 0 {
                    return Err(ConfigError::InvalidStartSpeed(0));
                }
                config.settings.start_speed_px_s = start_speed;
            }
            if let Some(max_speed) = s.max_speed_px_s {
                config.settings.max_speed_px_s = max_speed;
            }
            if let Some(ramp) = s.ramp_ms {
                if ramp == 0 {
                    return Err(ConfigError::InvalidRampMs(0));
                }
                config.settings.ramp_ms = ramp;
            }
        }

        if config.settings.max_speed_px_s < config.settings.start_speed_px_s {
            return Err(ConfigError::InvalidMaxSpeed(
                config.settings.max_speed_px_s,
                config.settings.start_speed_px_s,
            ));
        }

        if let Some(g) = raw.grid {
            if let Some(layout) = g.layout {
                config.grid = match layout.as_str() {
                    "dense" => GridConfig::dense(),
                    "simple" => GridConfig::default(),
                    _ => return Err(ConfigError::InvalidGridLayout(layout)),
                };
            }
            for (names, keys) in [
                (g.column_keys, &mut config.grid.column_keys),
                (g.row_keys, &mut config.grid.row_keys),
            ] {
                if let Some(names) = names {
                    if !config.grid.dense {
                        return Err(ConfigError::InvalidGridLayout(
                            "selection keys require dense layout".into(),
                        ));
                    }
                    *keys = names
                        .iter()
                        .map(|name| parse_logical_key(name))
                        .collect::<Result<_, _>>()?;
                }
                if config.grid.dense {
                    let mut seen = std::collections::HashSet::new();
                    if keys.is_empty()
                        || keys
                            .iter()
                            .any(|key| key.label().len() != 1 || !seen.insert(*key))
                    {
                        return Err(ConfigError::InvalidGridLayout(
                            "selection keys must be nonempty, unique printable keys".into(),
                        ));
                    }
                }
            }
            let rows = g.rows.unwrap_or(config.grid.rows);
            let cols = g.cols.unwrap_or(config.grid.cols);
            if rows == 0 || cols == 0 {
                return Err(ConfigError::InvalidGridDimensions(rows, cols));
            }
            config.grid.rows = rows;
            config.grid.cols = cols;
            let expected = rows
                .checked_mul(cols)
                .and_then(|count| usize::try_from(count).ok())
                .ok_or(ConfigError::GridDimensionsOverflow(rows, cols))?;

            if let Some(key_strings) = g.keys {
                if key_strings.len() != expected {
                    return Err(ConfigError::InvalidGridKeyCount(
                        key_strings.len(),
                        expected,
                    ));
                }
                let mut parsed_keys = Vec::with_capacity(expected);
                for k in key_strings {
                    parsed_keys.push(parse_logical_key(&k)?);
                }
                config.grid.keys = parsed_keys;
            }
            if config.grid.keys.len() != expected {
                return Err(ConfigError::InvalidGridKeyCount(
                    config.grid.keys.len(),
                    expected,
                ));
            }
            let mut seen = std::collections::HashSet::new();
            if config.grid.keys.iter().any(|key| !seen.insert(*key)) {
                return Err(ConfigError::DuplicateGridKey);
            }

            if let Some(afm) = g.auto_free_mode_after_move {
                config.grid.auto_free_mode_after_move = afm;
            }
            if let Some(nudge) = g.nudge_enabled {
                config.grid.nudge_enabled = nudge;
            }
            if let Some(step) = g.nudge_step_px {
                if step <= 0 {
                    return Err(ConfigError::InvalidNudgeStep);
                }
                config.grid.nudge_step_px = step;
            }
            if let Some(drag) = g.drag_after_select {
                config.grid.drag_after_select = drag;
            }
        }

        if let Some(layers) = raw.layers {
            if let Some(initial) = layers.initial {
                config.initial_bindings.clear();
                for (k, v) in initial {
                    let key = parse_logical_key(&k)?;
                    config.initial_bindings.insert(key, v);
                }
            }
            if let Some(mouse) = layers.mouse {
                config.mouse_bindings.clear();
                for (k, v) in mouse {
                    let key = parse_logical_key(&k)?;
                    if !v.eq_ignore_ascii_case("initial") && !v.eq_ignore_ascii_case("exit") {
                        let action = parse_action(&v)?;
                        config.mouse_bindings.insert(key, action);
                    }
                }
            }
        }

        Ok(config)
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::IoError(format!("{}: {}", path.as_ref().display(), e)))?;
        Self::parse(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_parse_valid_full_config_with_custom_grid() {
        let toml_str = r#"
[settings]
leader = "capslock"
start_speed_px_s = 400
max_speed_px_s = 2500
ramp_ms = 300

[grid]
rows = 2
cols = 2
keys = ["u", "i", "j", "k"]
auto_free_mode_after_move = false
nudge_enabled = true
nudge_step_px = 10

[layers.initial]
capslock = "mouse"

[layers.mouse]
h = "move_left"
j = "move_right"
k = "move_up"
l = "move_down"
u = "speed_down"
o = "speed_up"
f = "click_left"
d = "click_right"
w = "scroll_up"
s = "scroll_down"
esc = "initial"
"#;
        let cfg = Config::parse(toml_str).unwrap();
        assert_eq!(cfg.settings.leader, LogicalKey::CapsLock);
        assert_eq!(cfg.grid.rows, 2);
        assert_eq!(cfg.grid.cols, 2);
        assert_eq!(
            cfg.grid.keys,
            vec![LogicalKey::U, LogicalKey::I, LogicalKey::J, LogicalKey::K]
        );
        assert!(!cfg.grid.auto_free_mode_after_move);
        assert!(cfg.grid.nudge_enabled);
        assert_eq!(cfg.grid.nudge_step_px, 10);
        assert_eq!(
            cfg.mouse_bindings.get(&LogicalKey::H),
            Some(&Action::MoveLeft)
        );
    }

    #[test]
    fn t02_parse_empty_gives_defaults() {
        let cfg = Config::parse("").unwrap();
        assert_eq!(cfg.settings.leader, LogicalKey::CapsLock);
        assert_eq!(cfg.settings.start_speed_px_s, 300);
        assert_eq!(cfg.settings.max_speed_px_s, 3000);
        assert_eq!(cfg.settings.ramp_ms, 500);
        assert_eq!(cfg.grid.rows, 3);
        assert_eq!(cfg.grid.cols, 10);
        assert_eq!(cfg.grid.keys.len(), 30);
    }

    #[test]
    fn t03_parse_grid_key_count_mismatch_fails() {
        let toml_str = r#"
[grid]
rows = 2
cols = 2
keys = ["u", "i", "j"]
"#;
        let err = Config::parse(toml_str).unwrap_err();
        assert_eq!(err, ConfigError::InvalidGridKeyCount(3, 4));
    }

    #[test]
    fn t05_parse_action_supports_grid_entry_binding() {
        assert_eq!(parse_action("enter_grid"), Ok(Action::EnterGrid));
        let toml_str = r#"
[layers.mouse]
space = "enter_grid"
"#;
        let cfg = Config::parse(toml_str).unwrap();
        assert_eq!(
            cfg.mouse_bindings.get(&LogicalKey::Space),
            Some(&Action::EnterGrid)
        );
    }

    #[test]
    fn t06_default_config_binds_grid_entry_and_parses_drag_flag() {
        assert_eq!(
            Config::default().mouse_bindings.get(&LogicalKey::Space),
            Some(&Action::EnterGrid)
        );
        let cfg = Config::parse("[grid]\ndrag_after_select = true\n").unwrap();
        assert!(cfg.grid.drag_after_select);
        assert!(
            !Config::parse("[grid]\nnudge_step_px = 4\n")
                .unwrap()
                .grid
                .drag_after_select
        );
    }

    #[test]
    fn t04_parse_deep_links() {
        assert_eq!(
            parse_deep_link("mouseless://toggle-overlay"),
            Some(DeepLinkCommand::ToggleOverlay)
        );
        assert_eq!(
            parse_deep_link("clickless://enter-free-mode"),
            Some(DeepLinkCommand::EnterFreeMode)
        );
        assert_eq!(
            parse_deep_link("clickless://settings"),
            Some(DeepLinkCommand::Settings)
        );
        assert_eq!(parse_deep_link("invalid://something"), None);
    }
}
