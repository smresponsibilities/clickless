//! Shared Settings control values and descriptor-to-editor mapping.
//! Native and GPUI hosts retain raw text and use the same validated write path.

use super::editor::{Fields, SettingsEditor, fields_from_config};
use super::settings_model::{self, SettingsPage};
use std::collections::BTreeMap;

/// One settings card: model metadata plus the current draft value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRow {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub value: String,
}

/// Binds one page of the shared model to the editor draft. The WinUI host
/// renders these rows; `SettingsEditor` stays the only write path.
pub fn card_rows(editor: &SettingsEditor, page: SettingsPage) -> Vec<CardRow> {
    let fields = fields_from_config(editor.draft());
    settings_model::settings_for(page)
        .map(|descriptor| CardRow {
            key: descriptor.key,
            title: descriptor.title,
            description: descriptor.description,
            value: draft_value(&fields, descriptor.key),
        })
        .collect()
}

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.to_string()
}

/// The draft value for a model key. An unknown key yields an empty string so
/// the coverage test fails instead of rendering a stale example.
pub fn draft_value(fields: &Fields, key: &str) -> String {
    match key {
        "enabled" => on_off(fields.enabled),
        "scroll_step" => fields.scroll_step.clone(),
        "nested_keys" => fields.grid_keys.join(" "),
        "column_keys" => fields.column_keys.join(" "),
        "row_keys" => fields.row_keys.join(" "),
        "nudge_step" => fields.nudge_step_px.clone(),
        "drag_after_select" => on_off(fields.drag_after_select),
        "auto_free_mode" => on_off(fields.auto_free_mode),
        "panel_opacity" => fields.panel_opacity.clone(),
        "color_border" => fields.border.clone(),
        "border_px" => fields.border_px.clone(),
        "color_highlight" => fields.highlight.clone(),
        "highlight_opacity" => fields.highlight_opacity.clone(),
        "color_pointer" => fields.pointer.clone(),
        "label_size" => fields.label_size.clone(),

        "leader" => fields.leader.clone(),
        "hold_ms" => fields.hold_ms.clone(),
        "start_speed" => fields.start_speed_px_s.clone(),
        "max_speed" => fields.max_speed_px_s.clone(),
        "ramp_ms" => fields.ramp_ms.clone(),
        "layout" => fields.layout.clone(),
        "nested_size" => format!("{} x {}", fields.grid_rows, fields.grid_cols),
        "nudge_enabled" => on_off(fields.nudge_enabled),
        "mouse_bindings" => fields
            .mouse_bindings
            .iter()
            .map(|(key, action)| format!("{key} = {action}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "color_panel" => fields.panel.clone(),
        "color_label" => fields.label.clone(),
        "about" => format!("Clickless {}", env!("CARGO_PKG_VERSION")),
        _ => String::new(),
    }
}
/// One live card control, as read back from the WinUI host. The host folds a
/// map of these into the draft through `fields_with_draft`, so the write-back
/// path is testable without WinUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftValue {
    Flag(bool),
    Text(String),
    /// Index into `CHOICE_OPTIONS`.
    Choice(usize),
}

/// Labels and draft values behind every `SettingKind::Choice` control, in
/// display order. Index 0 is the `dense` layout that `draft_value` shows.
pub const CHOICE_OPTIONS: [(&str, &str); 2] = [("Dense", "dense"), ("Simple", "simple")];

/// Sidebar selection to page. An out-of-range index falls back to General, so
/// a programmatic selection change can never blank the content area.
pub fn page_for_index(index: i32) -> SettingsPage {
    SettingsPage::ALL
        .get(usize::try_from(index).unwrap_or(0))
        .copied()
        .unwrap_or(SettingsPage::General)
}

/// Folds live card values into a draft. `base` carries every field the host
/// has no control for yet, so unsupported settings keep their saved value.
/// A malformed value names its field and leaves the draft untouched, exactly
/// like `SettingsEditor::edit`.
pub fn fields_with_draft(
    base: &Fields,
    values: &BTreeMap<&'static str, DraftValue>,
) -> Result<Fields, String> {
    let mut fields = base.clone();
    for (key, value) in values {
        let key: &str = key;
        match (key, value) {
            ("enabled", DraftValue::Flag(on)) => fields.enabled = *on,
            ("scroll_step", DraftValue::Text(text)) => fields.scroll_step = text.trim().to_string(),
            ("nested_keys", DraftValue::Text(text)) => {
                fields.grid_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("column_keys", DraftValue::Text(text)) => {
                fields.column_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("row_keys", DraftValue::Text(text)) => {
                fields.row_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("nudge_step", DraftValue::Text(text)) => {
                fields.nudge_step_px = text.trim().to_string()
            }
            ("drag_after_select", DraftValue::Flag(on)) => fields.drag_after_select = *on,
            ("auto_free_mode", DraftValue::Flag(on)) => fields.auto_free_mode = *on,
            ("panel_opacity", DraftValue::Text(text)) => {
                fields.panel_opacity = text.trim().to_string()
            }
            ("color_border", DraftValue::Text(text)) => fields.border = text.trim().to_string(),
            ("border_px", DraftValue::Text(text)) => fields.border_px = text.trim().to_string(),
            ("color_highlight", DraftValue::Text(text)) => {
                fields.highlight = text.trim().to_string()
            }
            ("highlight_opacity", DraftValue::Text(text)) => {
                fields.highlight_opacity = text.trim().to_string()
            }
            ("color_pointer", DraftValue::Text(text)) => fields.pointer = text.trim().to_string(),
            ("label_size", DraftValue::Text(text)) => fields.label_size = text.trim().to_string(),

            ("nudge_enabled", DraftValue::Flag(on)) => fields.nudge_enabled = *on,
            ("leader", DraftValue::Text(text)) => fields.leader = text.trim().to_string(),
            ("hold_ms", DraftValue::Text(text)) => fields.hold_ms = text.trim().to_string(),
            ("start_speed", DraftValue::Text(text)) => {
                fields.start_speed_px_s = text.trim().to_string()
            }
            ("max_speed", DraftValue::Text(text)) => {
                fields.max_speed_px_s = text.trim().to_string()
            }
            ("ramp_ms", DraftValue::Text(text)) => fields.ramp_ms = text.trim().to_string(),
            ("color_panel", DraftValue::Text(text)) => fields.panel = text.trim().to_string(),
            ("color_label", DraftValue::Text(text)) => fields.label = text.trim().to_string(),
            ("mouse_bindings", DraftValue::Text(text)) => {
                fields.mouse_bindings = text
                    .split(['\r', '\n'])
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        line.split_once('=')
                            .map(|(key, action)| {
                                (key.trim().to_string(), action.trim().to_string())
                            })
                            .ok_or_else(|| format!("Shortcut must use key = action: {line}"))
                    })
                    .collect::<Result<_, _>>()?;
                let mut seen_keys = std::collections::HashSet::new();
                for (key, _) in &fields.mouse_bindings {
                    let k = key.to_ascii_lowercase();
                    if !seen_keys.insert(k) {
                        return Err(format!("Duplicate key '{key}' bound in mouse layer"));
                    }
                }
            }
            ("nested_size", DraftValue::Text(text)) => {
                let (rows, cols) = split_subgrid(text)?;
                fields.grid_rows = rows;
                fields.grid_cols = cols;
            }
            ("layout", DraftValue::Choice(index)) => {
                fields.layout = CHOICE_OPTIONS
                    .get(*index)
                    .ok_or_else(|| format!("Grid style: no option {index}"))?
                    .1
                    .to_string();
            }
            _ => {}
        }
    }
    Ok(fields)
}

/// Splits the `"3 x 10"` subgrid label that `draft_value` renders.
pub fn split_subgrid(text: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = text.split(['x', '×']).map(str::trim).collect();
    match parts.as_slice() {
        [rows, cols] if !rows.is_empty() && !cols.is_empty() => {
            Ok((rows.to_string(), cols.to_string()))
        }
        _ => Err(format!("Subgrid size: write \"3 x 10\", not \"{text}\"")),
    }
}

/// Applies a known keyboard preset to fields before review/apply.
pub fn apply_preset(fields: &mut Fields, preset: &str) -> Result<(), String> {
    match preset.to_ascii_lowercase().as_str() {
        "vim" | "vim_hjkl" | "default" => {
            fields.mouse_bindings = vec![
                ("h".into(), "move_left".into()),
                ("j".into(), "move_down".into()),
                ("k".into(), "move_up".into()),
                ("l".into(), "move_right".into()),
                ("f".into(), "click_left".into()),
                ("d".into(), "click_right".into()),
                ("w".into(), "scroll_up".into()),
                ("s".into(), "scroll_down".into()),
                ("u".into(), "speed_down".into()),
                ("o".into(), "speed_up".into()),
            ];
            fields.layout = "simple".into();
            Ok(())
        }
        "wasd" => {
            fields.mouse_bindings = vec![
                ("a".into(), "move_left".into()),
                ("s".into(), "move_down".into()),
                ("w".into(), "move_up".into()),
                ("d".into(), "move_right".into()),
                ("j".into(), "click_left".into()),
                ("k".into(), "click_right".into()),
                ("q".into(), "scroll_up".into()),
                ("e".into(), "scroll_down".into()),
                ("u".into(), "speed_down".into()),
                ("o".into(), "speed_up".into()),
            ];
            fields.layout = "simple".into();
            Ok(())
        }
        "simple_grid" | "grid" => {
            fields.layout = "simple".into();
            Ok(())
        }
        _ => Err(format!(
            "Unknown preset: {preset}. Choose vim, wasd, or simple_grid."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn presets_and_duplicate_binding_validation() {
        let mut fields = fields_from_config(&Config::default());
        apply_preset(&mut fields, "vim").expect("apply vim");
        assert_eq!(fields.layout, "simple");

        apply_preset(&mut fields, "wasd").expect("apply wasd");
        assert_eq!(
            fields
                .mouse_bindings
                .iter()
                .find(|(k, _)| k == "w")
                .map(|(_, a)| a.as_str()),
            Some("move_up")
        );

        // Duplicate binding error check in fields_with_draft
        let mut draft = BTreeMap::new();
        draft.insert(
            "mouse_bindings",
            DraftValue::Text("j = move_down\nj = click_left".into()),
        );
        let err = fields_with_draft(&fields, &draft).unwrap_err();
        assert!(err.contains("Duplicate key 'j' bound in mouse layer"));
    }
}
