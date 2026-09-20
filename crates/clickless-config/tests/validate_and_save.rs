use clickless_config::{Config, ConfigError};
use std::fs;

/// Every acceptance gate a draft must pass before Apply, in one call.
#[test]
fn t01_validate_accepts_defaults_and_rejects_bad_speeds() {
    assert!(Config::default().validate().is_ok());

    let mut bad = Config::default();
    bad.settings.start_speed_px_s = 0;
    assert_eq!(
        bad.validate(),
        Err(ConfigError::InvalidStartSpeed(0)),
        "validate must gate zero start speed"
    );
}

#[test]
fn t02_validate_rejects_max_speed_below_start() {
    let mut bad = Config::default();
    bad.settings.max_speed_px_s = 100;
    bad.settings.start_speed_px_s = 500;
    assert_eq!(
        bad.validate(),
        Err(ConfigError::InvalidMaxSpeed(100, 500)),
        "validate must gate max < start"
    );
}

#[test]
fn t03_validate_rejects_empty_or_oversized_grids() {
    let mut bad = Config::default();
    bad.grid.rows = 0;
    assert!(bad.validate().is_err(), "rows = 0 must fail");

    let mut overflow = Config::default();
    overflow.grid.rows = u32::MAX;
    overflow.grid.cols = u32::MAX;
    assert!(overflow.validate().is_err(), "rows*cols overflow must fail");
}

#[test]
fn t04_validate_detects_duplicate_grid_keys() {
    let mut bad = Config::default();
    bad.grid.keys[1] = bad.grid.keys[0];
    assert_eq!(bad.validate(), Err(ConfigError::DuplicateGridKey));
}

/// Esc and Backspace are reserved by the engine; binding them to actions must
/// be a validation error, not a silent shadow.
#[test]
fn t05_validate_flags_reserved_keys_in_mouse_bindings() {
    let mut bad = Config::default();
    bad.mouse_bindings.insert(
        clickless_core::LogicalKey::Esc,
        clickless_core::Action::ClickLeft,
    );
    assert!(
        bad.validate().is_err(),
        "Esc binding must be rejected as reserved"
    );

    let mut bad2 = Config::default();
    bad2.mouse_bindings.insert(
        clickless_core::LogicalKey::Backspace,
        clickless_core::Action::ClickLeft,
    );
    assert!(
        bad2.validate().is_err(),
        "Backspace binding must be rejected as reserved"
    );
}

/// The leader must not also be bound to a mouse action: it would never fire.
#[test]
fn t06_validate_flags_leader_bound_as_mouse_action() {
    let mut bad = Config::default();
    bad.mouse_bindings
        .insert(bad.settings.leader, clickless_core::Action::ClickLeft);
    assert!(bad.validate().is_err(), "leader rebind must be rejected");
}

#[test]
fn t07_validate_flags_duplicate_keys_in_outer_label_banks() {
    let mut bad = Config::default();
    bad.grid.column_keys[1] = bad.grid.column_keys[0];
    assert!(bad.validate().is_err(), "duplicate column label must fail");

    let mut bad2 = Config::default();
    bad2.grid.row_keys[1] = bad2.grid.row_keys[0];
    assert!(bad2.validate().is_err(), "duplicate row label must fail");
}

#[test]
fn t08_to_toml_round_trips_through_parse() {
    let original = Config::parse(
        "[settings]\nleader = \"space\"\nstart_speed_px_s = 450\n\n[grid]\nnudge_step_px = 7\n",
    )
    .unwrap();
    let serialized = original.to_toml();
    let reparsed = Config::parse(&serialized).unwrap();
    assert_eq!(original, reparsed, "export -> import must be lossless");
}

#[test]
fn t09_save_to_file_writes_parsable_toml() {
    let dir = std::env::temp_dir().join("clickless-save-test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let config = Config::parse("[grid]\nnudge_step_px = 9\n").unwrap();
    config.save_to_file(&path).unwrap();
    let loaded = Config::load_from_file(&path).unwrap();
    assert_eq!(loaded, config);

    let _ = fs::remove_dir_all(&dir);
}

/// A failed write must leave the previous file untouched. On Windows a
/// directory at the target path makes the write fail.
#[test]
fn t10_failed_save_preserves_previous_file() {
    let dir = std::env::temp_dir().join("clickless-save-fail-test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");

    let original = Config::parse("[grid]\nnudge_step_px = 11\n").unwrap();
    original.save_to_file(&path).unwrap();

    // Block the replacement: a directory where the temp file must land.
    let tmp_sibling = dir.join("config.toml.tmp");
    fs::create_dir_all(&tmp_sibling).unwrap();

    let updated = Config::parse("[grid]\nnudge_step_px = 12\n").unwrap();
    assert!(
        updated.save_to_file(&path).is_err(),
        "save must fail when the temp path is blocked"
    );
    let after = Config::load_from_file(&path).unwrap();
    assert_eq!(after, original, "old file must survive a failed save");

    let _ = fs::remove_dir_all(&dir);
}

// --- appearance: the [theme] table ---
#[test]
fn t11_theme_parses_all_fields_and_defaults_absent_ones() {
    let cfg = Config::parse("[theme]\npanel = \"1A2B3C\"\npanel_opacity = 230\nlabel_size = 4\n")
        .unwrap();
    assert_eq!(cfg.theme.panel, (0x1A, 0x2B, 0x3C));
    assert_eq!(cfg.theme.panel_opacity, 230);
    assert_eq!(cfg.theme.label_size, 4);
    // Untouched fields keep their defaults.
    assert_eq!(
        cfg.theme.highlight,
        clickless_config::ThemeConfig::default().highlight
    );
}

#[test]
fn t12_theme_rejects_non_hex_and_out_of_range_values() {
    let err = Config::parse("[theme]\npanel = \"green\"\n").unwrap_err();
    assert!(
        matches!(err, clickless_config::ConfigError::InvalidTheme(_)),
        "non-hex colour must be InvalidTheme, got {err}"
    );
    let err = Config::parse("[theme]\npanel_opacity = 300\n").unwrap_err();
    assert!(matches!(
        err,
        clickless_config::ConfigError::InvalidTheme(_)
    ));
    let err = Config::parse("[theme]\nlabel_size = 0\n").unwrap_err();
    assert!(matches!(
        err,
        clickless_config::ConfigError::InvalidTheme(_)
    ));
    let err = Config::parse("[theme]\nlabel_size = 9\n").unwrap_err();
    assert!(matches!(
        err,
        clickless_config::ConfigError::InvalidTheme(_)
    ));
}

#[test]
fn t13_theme_round_trips_through_to_toml_and_validate() {
    let cfg =
        Config::parse("[theme]\npanel = \"101010\"\nhighlight = \"FFCC00\"\npanel_opacity = 140\n")
            .unwrap();
    cfg.validate().unwrap();
    let reparsed = Config::parse(&cfg.to_toml()).unwrap();
    assert_eq!(cfg, reparsed, "theme must survive export -> import");
}
