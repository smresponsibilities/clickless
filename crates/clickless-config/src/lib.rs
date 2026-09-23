use clickless_backend_api::overlay::OverlayTheme;

pub mod settings_model;
pub use settings_model::{SettingDescriptor, SettingKind, SettingsPage};

/// Rasterizer defaults for theme fields the user has not set.
pub use clickless_backend_api::overlay::{BORDER_PX, HIGHLIGHT_ALPHA};

/// Canonical TOML name for a logical key: the inverse of `parse_logical_key`
/// for every accepted input. The Settings editor uses it to display and
/// serialize the leader key.
pub fn logical_key_name(key: LogicalKey) -> &'static str {
    match key {
        LogicalKey::CapsLock => "capslock",
        LogicalKey::A => "a",
        LogicalKey::B => "b",
        LogicalKey::C => "c",
        LogicalKey::E => "e",
        LogicalKey::G => "g",
        LogicalKey::N => "n",
        LogicalKey::P => "p",
        LogicalKey::Q => "q",
        LogicalKey::R => "r",
        LogicalKey::T => "t",
        LogicalKey::V => "v",
        LogicalKey::X => "x",
        LogicalKey::Y => "y",
        LogicalKey::Z => "z",
        LogicalKey::Semicolon => ";",
        LogicalKey::Slash => "/",
        LogicalKey::Backspace => "backspace",
        LogicalKey::H => "h",
        LogicalKey::J => "j",
        LogicalKey::K => "k",
        LogicalKey::L => "l",
        LogicalKey::U => "u",
        LogicalKey::I => "i",
        LogicalKey::O => "o",
        LogicalKey::F => "f",
        LogicalKey::D => "d",
        LogicalKey::W => "w",
        LogicalKey::S => "s",
        LogicalKey::M => "m",
        LogicalKey::Comma => ",",
        LogicalKey::Dot => ".",
        LogicalKey::Space => "space",
        LogicalKey::Esc => "esc",
    }
}

/// Platform config file location: `%APPDATA%\clickless\clickless.toml` on
/// Windows, `~/.config/clickless/clickless.toml` elsewhere. The directory is
/// not created here; saving creates it.
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    #[cfg(windows)]
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .map_err(|_| ConfigError::IoError("APPDATA not set".into()))?;
    #[cfg(not(windows))]
    let base = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".config"))
        .map_err(|_| ConfigError::IoError("HOME not set".into()))?;
    Ok(base.join("clickless").join("clickless.toml"))
}
use clickless_core::grid::GridConfig;
use clickless_core::{Action, LogicalKey};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub leader: LogicalKey,
    pub start_speed_px_s: u64,
    pub max_speed_px_s: u64,
    pub ramp_ms: u64,
    /// Leader hold time before Mouse capture, in ms. Default 200.
    pub hold_ms: u64,
    /// Scroll notches per ScrollUp/Down action. Default 1.
    pub scroll_step: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            leader: LogicalKey::CapsLock,
            start_speed_px_s: 300,
            max_speed_px_s: 3000,
            ramp_ms: 500,
            hold_ms: 200,
            scroll_step: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub settings: Settings,
    pub grid: GridConfig,
    pub theme: ThemeConfig,
    pub initial_bindings: HashMap<LogicalKey, String>,
    pub mouse_bindings: HashMap<LogicalKey, Action>,
    /// First-run practice completion mark. 0 means never completed.
    pub practice_completed_version: u32,
}

impl Default for Config {
    fn default() -> Self {
        let mouse_bindings = clickless_core::default_bindings();

        let mut initial_bindings = HashMap::new();
        initial_bindings.insert(LogicalKey::CapsLock, "mouse".to_string());

        Self {
            enabled: true,
            settings: Settings::default(),
            grid: GridConfig::dense(),
            theme: ThemeConfig::default(),
            initial_bindings,
            mouse_bindings,
            practice_completed_version: 0,
        }
    }
}

/// Overlay appearance: colours as RGB triples, `panel_opacity` as 0-255
/// alpha (the rasterizer's unit), `label_size` glyph scale >= 1. Defaults
/// equal the rasterizer's built-in constants, so the default look is
/// pixel-identical to before theming existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeConfig {
    pub panel: (u8, u8, u8),
    pub panel_opacity: u8,
    pub border: (u8, u8, u8),
    /// Highlight fill opacity, 0-255. Defaults to the rasterizer constant.
    pub highlight_opacity: u8,
    /// Grid line width in pixels. Defaults to the rasterizer constant.
    pub border_px: i64,
    pub highlight: (u8, u8, u8),
    pub label: (u8, u8, u8),
    pub pointer: (u8, u8, u8),
    pub label_size: i64,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            panel: (24, 28, 38),
            panel_opacity: 70,
            border: (100, 122, 150),
            highlight_opacity: crate::HIGHLIGHT_ALPHA,
            border_px: crate::BORDER_PX,
            highlight: (255, 196, 64),
            label: (240, 246, 255),
            pointer: (255, 96, 96),
            label_size: 3,
        }
    }
}

impl ThemeConfig {
    /// Converts the theme into the rasterizer's tunable struct. Same units
    /// in, same units out: no conversion, no drift.
    pub fn to_overlay_theme(&self) -> OverlayTheme {
        OverlayTheme {
            panel_rgb: self.panel,
            panel_alpha: self.panel_opacity,
            border_rgb: self.border,
            border_px: self.border_px,
            highlight_rgb: self.highlight,
            highlight_alpha: self.highlight_opacity,
            label_rgb: self.label,
            pointer_rgb: self.pointer,
            glyph_scale: self.label_size,
        }
    }
}

/// Parses `RRGGBB` hex (with optional `#`) into an RGB triple. Public so
/// the Settings editor shares the exact accepted format.
pub fn parse_hex_rgb(name: &str, raw: &str) -> Result<(u8, u8, u8), ConfigError> {
    let digits = raw.strip_prefix('#').unwrap_or(raw);
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ConfigError::InvalidTheme(format!(
            "{name} = \"{raw}\" must be RRGGBB hex"
        )));
    }
    let value = u32::from_str_radix(digits, 16).unwrap_or(0);
    Ok((
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    ))
}

fn hex_rgb(value: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", value.0, value.1, value.2)
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
    #[error("Invalid hold: hold_ms ({0}) must be greater than 0")]
    InvalidHoldMs(u64),
    #[error("Invalid scroll step: scroll_step ({0}) must be greater than 0")]
    InvalidScrollStep(i64),
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
    #[error("Key '{0}' is reserved by the engine and cannot be bound")]
    ReservedKey(String),
    #[error("Key '{0}' is bound more than once")]
    DuplicateBinding(String),
    #[error("Invalid theme value: {0}")]
    InvalidTheme(String),
}

#[derive(Debug, Deserialize)]
struct RawTomlConfig {
    enabled: Option<bool>,
    practice_completed_version: Option<u32>,
    settings: Option<RawSettings>,
    grid: Option<RawGrid>,
    theme: Option<RawTheme>,
    layers: Option<RawLayers>,
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    panel: Option<String>,
    panel_opacity: Option<u16>,
    border: Option<String>,
    highlight_opacity: Option<u16>,
    border_px: Option<i64>,
    highlight: Option<String>,
    label: Option<String>,
    pointer: Option<String>,
    label_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawSettings {
    leader: Option<String>,
    start_speed_px_s: Option<u64>,
    max_speed_px_s: Option<u64>,
    ramp_ms: Option<u64>,
    hold_ms: Option<u64>,
    scroll_step: Option<i64>,
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
        if let Some(enabled) = raw.enabled {
            config.enabled = enabled;
        }
        if let Some(version) = raw.practice_completed_version {
            config.practice_completed_version = version;
        }

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
            if let Some(hold) = s.hold_ms {
                if hold == 0 {
                    return Err(ConfigError::InvalidHoldMs(0));
                }
                config.settings.hold_ms = hold;
            }
            if let Some(step) = s.scroll_step {
                if step <= 0 {
                    return Err(ConfigError::InvalidScrollStep(step));
                }
                config.settings.scroll_step = step;
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

        if let Some(t) = raw.theme {
            if let Some(raw) = t.panel {
                config.theme.panel = parse_hex_rgb("panel", &raw)?;
            }
            if let Some(raw) = t.border {
                config.theme.border = parse_hex_rgb("border", &raw)?;
            }
            if let Some(raw) = t.highlight {
                config.theme.highlight = parse_hex_rgb("highlight", &raw)?;
            }
            if let Some(opacity) = t.highlight_opacity {
                if opacity > 255 {
                    return Err(ConfigError::InvalidTheme(
                        "highlight_opacity must be 0-255".into(),
                    ));
                }
                config.theme.highlight_opacity = opacity as u8;
            }
            if let Some(width) = t.border_px {
                config.theme.border_px = width;
            }
            if let Some(raw) = t.label {
                config.theme.label = parse_hex_rgb("label", &raw)?;
            }
            if let Some(raw) = t.pointer {
                config.theme.pointer = parse_hex_rgb("pointer", &raw)?;
            }
            if let Some(opacity) = t.panel_opacity {
                if opacity > 255 {
                    return Err(ConfigError::InvalidTheme(
                        "panel_opacity must be 0-255".into(),
                    ));
                }
                config.theme.panel_opacity = opacity as u8;
            }
            if let Some(size) = t.label_size {
                if !(1..=8).contains(&size) {
                    return Err(ConfigError::InvalidTheme("label_size must be 1-8".into()));
                }
                config.theme.label_size = size;
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

    /// Engine-reserved keys: Esc cancels, Backspace undoes grid selection.
    fn reserved_keys() -> [LogicalKey; 2] {
        [LogicalKey::Esc, LogicalKey::Backspace]
    }

    /// Validates the whole config in one call: speeds, grid geometry, and
    /// binding conflicts (reserved keys, leader reuse, duplicates). This is
    /// the gate Apply must pass before a draft reaches the runtime.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.settings.start_speed_px_s == 0 {
            return Err(ConfigError::InvalidStartSpeed(0));
        }
        if self.settings.max_speed_px_s < self.settings.start_speed_px_s {
            return Err(ConfigError::InvalidMaxSpeed(
                self.settings.max_speed_px_s,
                self.settings.start_speed_px_s,
            ));
        }
        if self.settings.ramp_ms == 0 {
            return Err(ConfigError::InvalidRampMs(0));
        }
        if self.settings.hold_ms == 0 {
            return Err(ConfigError::InvalidHoldMs(0));
        }
        if self.settings.scroll_step <= 0 {
            return Err(ConfigError::InvalidScrollStep(self.settings.scroll_step));
        }

        let rows = self.grid.rows;
        let cols = self.grid.cols;
        if rows == 0 || cols == 0 {
            return Err(ConfigError::InvalidGridDimensions(rows, cols));
        }
        let expected = rows
            .checked_mul(cols)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(ConfigError::GridDimensionsOverflow(rows, cols))?;
        if self.grid.keys.len() != expected {
            return Err(ConfigError::InvalidGridKeyCount(
                self.grid.keys.len(),
                expected,
            ));
        }
        let mut seen = std::collections::HashSet::new();
        if self.grid.keys.iter().any(|key| !seen.insert(*key)) {
            return Err(ConfigError::DuplicateGridKey);
        }

        for bank in [&self.grid.column_keys, &self.grid.row_keys] {
            let mut bank_seen = std::collections::HashSet::new();
            if bank.iter().any(|key| !bank_seen.insert(*key)) {
                return Err(ConfigError::DuplicateGridKey);
            }
        }

        for key in Self::reserved_keys() {
            if self.mouse_bindings.contains_key(&key) {
                return Err(ConfigError::ReservedKey(key.label().to_string()));
            }
        }
        if self.mouse_bindings.contains_key(&self.settings.leader) {
            return Err(ConfigError::ReservedKey(
                self.settings.leader.label().to_string(),
            ));
        }

        if self.theme.label_size < 1 {
            return Err(ConfigError::InvalidTheme(
                "label_size must be at least 1".into(),
            ));
        }
        if !(1..=16).contains(&self.theme.border_px) {
            return Err(ConfigError::InvalidTheme("border_px must be 1-16".into()));
        }

        Ok(())
    }

    /// Serializes the config back to TOML so `parse` round-trips losslessly.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("enabled = {}\n", self.enabled));
        out.push_str(&format!(
            "practice_completed_version = {}\n",
            self.practice_completed_version
        ));
        out.push_str("\n[settings]\n");
        out.push_str(&format!(
            "leader = {}\n",
            toml_string(self.settings.leader.label())
        ));
        out.push_str(&format!(
            "start_speed_px_s = {}\n",
            self.settings.start_speed_px_s
        ));
        out.push_str(&format!(
            "max_speed_px_s = {}\n",
            self.settings.max_speed_px_s
        ));
        out.push_str(&format!("ramp_ms = {}\n", self.settings.ramp_ms));
        out.push_str(&format!("hold_ms = {}\n", self.settings.hold_ms));
        out.push_str(&format!("scroll_step = {}\n", self.settings.scroll_step));

        out.push_str("\n[grid]\n");
        out.push_str(&format!(
            "layout = {}\n",
            toml_string(if self.grid.dense { "dense" } else { "simple" })
        ));
        out.push_str(&format!("rows = {}\n", self.grid.rows));
        out.push_str(&format!("cols = {}\n", self.grid.cols));
        out.push_str(&format!(
            "keys = [{}]\n",
            self.grid
                .keys
                .iter()
                .map(|k| toml_string(k.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if self.grid.dense {
            out.push_str(&format!(
                "column_keys = [{}]\n",
                self.grid
                    .column_keys
                    .iter()
                    .map(|k| toml_string(k.label()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!(
                "row_keys = [{}]\n",
                self.grid
                    .row_keys
                    .iter()
                    .map(|k| toml_string(k.label()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push_str(&format!(
            "auto_free_mode_after_move = {}\n",
            self.grid.auto_free_mode_after_move
        ));
        out.push_str(&format!("nudge_enabled = {}\n", self.grid.nudge_enabled));
        out.push_str(&format!("nudge_step_px = {}\n", self.grid.nudge_step_px));
        out.push_str(&format!(
            "drag_after_select = {}\n",
            self.grid.drag_after_select
        ));

        out.push_str("\n[theme]\n");
        out.push_str(&format!(
            "panel = {}\n",
            toml_string(&hex_rgb(self.theme.panel))
        ));
        out.push_str(&format!("panel_opacity = {}\n", self.theme.panel_opacity));
        out.push_str(&format!(
            "border = {}\n",
            toml_string(&hex_rgb(self.theme.border))
        ));
        out.push_str(&format!(
            "highlight_opacity = {}\n",
            self.theme.highlight_opacity
        ));
        out.push_str(&format!("border_px = {}\n", self.theme.border_px));
        out.push_str(&format!(
            "highlight = {}\n",
            toml_string(&hex_rgb(self.theme.highlight))
        ));
        out.push_str(&format!(
            "label = {}\n",
            toml_string(&hex_rgb(self.theme.label))
        ));
        out.push_str(&format!(
            "pointer = {}\n",
            toml_string(&hex_rgb(self.theme.pointer))
        ));
        out.push_str(&format!("label_size = {}\n", self.theme.label_size));

        out.push_str("\n[layers.initial]\n");
        for (key, value) in &self.initial_bindings {
            out.push_str(&format!(
                "{} = {}\n",
                toml_key(key.label()),
                toml_string(value)
            ));
        }

        out.push_str("\n[layers.mouse]\n");
        for (key, action) in &self.mouse_bindings {
            out.push_str(&format!(
                "{} = {}\n",
                toml_key(key.label()),
                toml_string(action_name(*action))
            ));
        }
        out
    }

    /// Writes TOML through a temporary sibling then renames, so a failed
    /// write never destroys the previous file.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigError::IoError(format!("{}: {}", parent.display(), e)))?;
        }
        // config.toml -> config.toml.tmp, an exact sibling of the target.
        let mut tmp_name = path.as_os_str().to_owned();
        tmp_name.push(".tmp");
        let tmp = PathBuf::from(tmp_name);

        let body = self.to_toml();
        fs::write(&tmp, body)
            .map_err(|e| ConfigError::IoError(format!("{}: {}", tmp.display(), e)))?;
        if let Err(err) = fs::rename(&tmp, path) {
            let _ = fs::remove_file(&tmp);
            return Err(ConfigError::IoError(format!("{}: {}", path.display(), err)));
        }
        Ok(())
    }
}

fn toml_string(value: &str) -> String {
    format!("\"{value}\"")
}

/// Labels like `;` and `.` and `space` are legal TOML bare keys? Only
/// alphanumerics, `-` and `_` are, so quote anything else.
fn toml_key(label: &str) -> String {
    let bare = label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !label.is_empty();
    if bare {
        label.to_string()
    } else {
        toml_string(label)
    }
}

pub fn action_name(action: Action) -> &'static str {
    match action {
        Action::MoveLeft => "move_left",
        Action::MoveRight => "move_right",
        Action::MoveUp => "move_up",
        Action::MoveDown => "move_down",
        Action::SpeedDown => "speed_down",
        Action::SpeedUp => "speed_up",
        Action::ClickLeft => "click_left",
        Action::ClickRight => "click_right",
        Action::ScrollUp => "scroll_up",
        Action::ScrollDown => "scroll_down",
        Action::EnterGrid => "enter_grid",
        Action::MoveTo(_, _) | Action::ClickAt(_, _) | Action::DragTo(_, _) | Action::DragEnd => {
            "move_left"
        }
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
