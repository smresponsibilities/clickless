use clickless_config::editor::{SettingsEditor, EditorResult, Fields};

pub struct SettingsAppModel {
    pub editor: SettingsEditor,
    pub raw_fields: Fields,
    pub apply_status: String,
}

impl SettingsAppModel {
    pub fn new() -> Self {
        let (editor, _rx) = SettingsEditor::new();
        let current_config = editor.current_config();
        
        let raw_fields = Fields {
            enabled: current_config.enabled,
            leader: current_config.leader.to_string(),
            start_speed_px_s: current_config.start_speed_px_s.to_string(),
            max_speed_px_s: current_config.max_speed_px_s.to_string(),
            ramp_ms: current_config.ramp_ms.to_string(),
            // Map the rest...
            grid_keys: current_config.grid_keys.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(""),
            grid_width: current_config.grid_width.to_string(),
            grid_height: current_config.grid_height.to_string(),
            layer1_key: current_config.layer1_key.to_string(),
            layer2_key: current_config.layer2_key.to_string(),
            bindings: std::collections::HashMap::new(), // omitted for sample
            theme: clickless_config::ThemeConfig::System,
            opacity: current_config.opacity.to_string(),
            reticle_color: current_config.reticle_color.to_string(),
        };

        Self {
            editor,
            raw_fields,
            apply_status: "Persisted".to_string(),
        }
    }

    pub fn apply(&mut self) {
        match self.editor.apply_fields(self.raw_fields.clone()) {
            EditorResult::Applied => {
                match self.editor.save() {
                    Ok(_) => self.apply_status = "Saved successfully".to_string(),
                    Err(e) => self.apply_status = format!("Save failed: {}", e),
                }
            }
            EditorResult::ValidationError(e) => {
                self.apply_status = format!("Validation error: {}", e);
            }
            EditorResult::Unchanged => {
                self.apply_status = "Unchanged".to_string();
            }
        }
    }
}
