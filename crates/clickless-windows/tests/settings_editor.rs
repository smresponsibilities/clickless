//! Prompt 3 red tests: pure editor semantics against the real Config.

use clickless_config::Config;
use clickless_core::LogicalKey;
use clickless_windows::settings_editor::{
    Fields, Section, SettingsEditor, apply_fields, fields_from_config,
};

fn default_fields() -> Fields {
    fields_from_config(&Config::default())
}

#[test]
fn t01_fields_round_trip_is_not_dirty() {
    let mut editor = SettingsEditor::new(Config::default());
    editor.edit(&default_fields()).unwrap();
    assert!(!editor.is_dirty());
    assert_eq!(editor.draft(), editor.applied());
}

#[test]
fn t02_bad_number_names_the_field() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "abc".into();
    let err = editor.edit(&fields).unwrap_err();
    assert!(err.contains("start"), "error must name the field: {err}");
    assert!(!editor.is_dirty());
}

#[test]
fn t03_invalid_edit_leaves_draft_and_applied_unchanged() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.max_speed_px_s = "100".into(); // below start speed 300
    // Validation is eager: the edit itself is rejected and names the rule.
    let err = editor.edit(&fields).unwrap_err();
    assert!(err.contains("max speed"), "error names the rule: {err}");
    assert!(!editor.is_dirty());
    assert_eq!(editor.applied(), &Config::default());
    // Apply with the untouched draft succeeds.
    editor.apply().unwrap();
    assert_eq!(editor.applied(), &Config::default());
}

#[test]
fn t04_valid_apply_updates_applied() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "400".into();
    editor.edit(&fields).unwrap();
    editor.apply().unwrap();
    assert_eq!(editor.applied().settings.start_speed_px_s, 400);
    assert!(!editor.is_dirty());
}

#[test]
fn t05_failed_save_preserves_old_file() {
    let dir = std::env::temp_dir().join("clickless-editor-test-9a4f");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.toml");
    std::fs::write(&path, Config::default().to_toml()).unwrap();

    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "777".into();
    editor.edit(&fields).unwrap();
    editor.apply().unwrap();

    // A parent that is a regular file makes the atomic save fail. A merely
    // missing directory is not an error: save creates the parent chain.
    let blocker = dir.join("blocker-file");
    std::fs::write(&blocker, b"x").unwrap();
    let bad = blocker.join("c.toml");
    assert!(editor.save(&bad).is_err());
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("start_speed_px_s = 300"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn t06_cancel_restores_last_applied_and_close_equals_cancel() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "400".into();
    editor.edit(&fields).unwrap();
    editor.apply().unwrap();

    let mut fields = default_fields();
    fields.start_speed_px_s = "900".into();
    editor.edit(&fields).unwrap();
    assert!(editor.is_dirty());

    editor.cancel();
    assert_eq!(editor.draft().settings.start_speed_px_s, 400);
    assert!(!editor.is_dirty());

    // Closing the window behaves like Cancel: a second cancel is a no-op.
    editor.cancel();
    assert_eq!(editor.draft().settings.start_speed_px_s, 400);
}

#[test]
fn t07_colors_parse_and_reject() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.panel = "1A2B3C".into();
    editor.edit(&fields).unwrap();
    assert_eq!(editor.draft().theme.panel, (0x1A, 0x2B, 0x3C));

    let mut bad = default_fields();
    bad.panel = "zzz".into();
    assert!(editor.edit(&bad).is_err());
}

#[test]
fn t08_leader_field_round_trips() {
    let fields = default_fields();
    assert_eq!(fields.leader, "capslock");

    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    // Space is taken by the enter_grid binding; c is free in the defaults.
    fields.leader = "c".into();
    editor.edit(&fields).unwrap();
    assert_eq!(editor.draft().settings.leader, LogicalKey::C);

    // A leader that collides with an existing binding is rejected.
    let mut bad = default_fields();
    bad.leader = "space".into();
    let err = editor.edit(&bad).unwrap_err();
    assert!(err.contains("reserved"), "error: {err}");
    assert_eq!(editor.draft().settings.leader, LogicalKey::C);
}

#[test]
fn t09_layout_switch_flips_dense_flag() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    assert_eq!(fields.layout, "dense");
    fields.layout = "simple".into();
    editor.edit(&fields).unwrap();
    assert!(!editor.draft().grid.dense);

    let mut fields = fields_from_config(editor.draft());
    fields.layout = "dense".into();
    editor.edit(&fields).unwrap();
    assert!(editor.draft().grid.dense);
    assert_eq!(editor.draft().grid.nudge_step_px, 1);
}

#[test]
fn t10_apply_fields_is_the_only_parser_used() {
    // apply_fields must accept exactly what Config::parse accepts for the
    // leader name, including the shorthand forms.
    let mut config = Config::default();
    let mut fields = default_fields();
    fields.leader = "caps".into();
    apply_fields(&mut config, &fields).unwrap();
    assert_eq!(config.settings.leader, LogicalKey::CapsLock);
}

#[test]
fn t11_promised_fields_round_trip_through_config() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.enabled = false;
    fields.highlight_opacity = "180".into();
    fields.border_px = "6".into();

    editor.edit(&fields).unwrap();
    editor.apply().unwrap();

    assert!(!editor.applied().enabled);
    assert_eq!(editor.applied().theme.highlight_opacity, 180);
    assert_eq!(editor.applied().theme.border_px, 6);
    let round_trip = fields_from_config(editor.applied());
    assert!(!round_trip.enabled);
    assert_eq!(round_trip.highlight_opacity, "180");
    assert_eq!(round_trip.border_px, "6");
}

#[test]
fn t12_runtime_apply_failure_keeps_applied_config_unchanged() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "777".into();
    editor.edit(&fields).unwrap();

    let err = editor
        .apply_to_runtime(|_| Err("runtime rejected config".into()))
        .unwrap_err();

    assert!(err.contains("runtime rejected"));
    assert_eq!(editor.applied().settings.start_speed_px_s, 300);
    assert!(editor.is_dirty());
}

#[test]
fn t13_save_does_not_persist_config_that_failed_runtime_apply() {
    let dir = std::env::temp_dir().join("clickless-editor-test-save-runtime");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.toml");
    std::fs::write(&path, Config::default().to_toml()).unwrap();

    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.start_speed_px_s = "888".into();
    editor.edit(&fields).unwrap();

    let result = editor.save_after_runtime_apply(&path, |_| Err("runtime failed".into()));

    assert!(result.is_err());
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("start_speed_px_s = 300"));
    assert!(!on_disk.contains("start_speed_px_s = 888"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn t14_bindings_and_grid_fields_round_trip_through_toml() {
    let mut fields = default_fields();
    fields.mouse_bindings[0] = ("g".into(), "click_left".into());
    fields.grid_rows = "2".into();
    fields.grid_cols = "2".into();
    fields.grid_keys = ["u", "i", "o", "j"].map(str::to_string).into();
    fields.column_keys[0] = "q".into();
    fields.row_keys[0] = "b".into();
    fields.row_keys[24] = "q".into();

    let mut editor = SettingsEditor::new(Config::default());
    editor.edit(&fields).unwrap();
    editor.apply().unwrap();

    let config = editor.applied();
    assert_eq!(
        config.mouse_bindings.get(&LogicalKey::G),
        Some(&clickless_core::Action::ClickLeft)
    );
    assert_eq!(config.grid.rows, 2);
    assert_eq!(config.grid.cols, 2);
    assert_eq!(
        config.grid.keys,
        [LogicalKey::U, LogicalKey::I, LogicalKey::O, LogicalKey::J]
    );
    assert_eq!(config.grid.column_keys[0], LogicalKey::Q);
    assert_eq!(config.grid.row_keys[0], LogicalKey::B);
    let parsed = Config::parse(&config.to_toml()).unwrap();
    assert_eq!(&parsed, config);
}

#[test]
fn t15_binding_errors_are_grouped_and_duplicate_key_is_not_lost() {
    let mut fields = default_fields();
    fields.mouse_bindings[0] = ("capslock".into(), "move_left".into());
    let err = apply_fields(&mut Config::default(), &fields).unwrap_err();
    assert!(err.starts_with("bindings:"), "{err}");
    assert!(err.contains("reserved"), "{err}");

    fields.mouse_bindings[0] = ("g".into(), "move_left".into());
    fields.mouse_bindings[1] = ("g".into(), "move_right".into());
    let err = apply_fields(&mut Config::default(), &fields).unwrap_err();
    assert!(err.starts_with("bindings:"), "{err}");
    assert!(err.contains("bound more than once"), "{err}");
}

#[test]
fn t16_grid_errors_are_grouped_for_the_grid_editor() {
    let mut fields = default_fields();
    fields.grid_rows = "2".into();
    fields.grid_cols = "2".into();
    fields.grid_keys = ["u", "u", "o", "j"].map(str::to_string).into();
    let err = apply_fields(&mut Config::default(), &fields).unwrap_err();
    assert!(err.starts_with("grid:"), "{err}");
    assert!(err.contains("key appears more than once"), "{err}");
}

#[test]
fn t17_bindings_and_grid_fields_survive_save_and_restart() {
    let mut fields = default_fields();
    fields.mouse_bindings[0] = ("g".into(), "click_left".into());
    fields.grid_rows = "2".into();
    fields.grid_cols = "2".into();
    fields.grid_keys = ["u", "i", "o", "j"].map(str::to_string).into();
    fields.column_keys[0] = "q".into();
    fields.row_keys[0] = "b".into();
    fields.row_keys[24] = "q".into();
    fields.auto_free_mode = true;

    let mut editor = SettingsEditor::new(Config::default());
    editor.edit(&fields).unwrap();

    let dir = std::env::temp_dir().join("clickless-editor-test-restart-024");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.toml");
    editor.save_after_runtime_apply(&path, |_| Ok(())).unwrap();

    let on_disk = std::fs::read_to_string(&path).unwrap();
    let restarted = Config::parse(&on_disk).unwrap();
    assert_eq!(
        restarted.mouse_bindings.get(&LogicalKey::G),
        Some(&clickless_core::Action::ClickLeft)
    );
    assert_eq!((restarted.grid.rows, restarted.grid.cols), (2, 2));
    assert_eq!(
        restarted.grid.keys,
        [LogicalKey::U, LogicalKey::I, LogicalKey::O, LogicalKey::J]
    );
    assert_eq!(restarted.grid.column_keys[0], LogicalKey::Q);
    assert_eq!(restarted.grid.row_keys[0], LogicalKey::B);
    assert!(restarted.grid.auto_free_mode_after_move);

    let round = fields_from_config(&restarted);
    assert_eq!(round.grid_rows, "2");
    assert_eq!(round.grid_cols, "2");
    assert!(
        round
            .mouse_bindings
            .contains(&("g".to_string(), "click_left".to_string())),
        "g binding survives restart: {:?}",
        round.mouse_bindings
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn t18_hold_ms_and_scroll_step_round_trip_and_reject_zero() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.hold_ms = "350".into();
    fields.scroll_step = "3".into();
    editor.edit(&fields).unwrap();
    editor.apply().unwrap();
    assert_eq!(editor.applied().settings.hold_ms, 350);
    assert_eq!(editor.applied().settings.scroll_step, 3);
    let round = fields_from_config(editor.applied());
    assert_eq!(round.hold_ms, "350");
    assert_eq!(round.scroll_step, "3");

    let mut bad = default_fields();
    bad.hold_ms = "0".into();
    let err = editor.edit(&bad).unwrap_err();
    assert!(err.contains("hold"), "error names the field: {err}");

    let mut bad = default_fields();
    bad.scroll_step = "0".into();
    let err = editor.edit(&bad).unwrap_err();
    assert!(err.contains("scroll"), "error names the field: {err}");
}

#[test]
fn t19_reset_all_restores_defaults_and_clears_dirt_when_applied_is_default() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.hold_ms = "350".into();
    fields.grid_rows = "2".into();
    fields.grid_cols = "2".into();
    fields.grid_keys = ["u", "i", "o", "j"].map(str::to_string).into();
    editor.edit(&fields).unwrap();
    assert!(editor.is_dirty());

    editor.reset_all();
    assert_eq!(editor.draft(), &Config::default());
    assert!(!editor.is_dirty());
}

#[test]
fn t20_reset_section_keeps_other_groups() {
    let mut editor = SettingsEditor::new(Config::default());
    let mut fields = default_fields();
    fields.hold_ms = "350".into();
    fields.grid_rows = "2".into();
    fields.grid_cols = "2".into();
    fields.grid_keys = ["u", "i", "o", "j"].map(str::to_string).into();
    editor.edit(&fields).unwrap();

    editor.reset_section(Section::Grid);
    assert_eq!(editor.draft().grid, Config::default().grid);
    assert_eq!(editor.draft().settings.hold_ms, 350);
    assert!(editor.is_dirty());

    editor.reset_section(Section::Settings);
    assert_eq!(editor.draft().settings, Config::default().settings);
    assert!(!editor.is_dirty());
}
