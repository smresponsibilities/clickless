//! App-rebuild 17: every shippable setting has help content, and gap rows
//! (no runtime effect) have none. Foundation-independent: any shell
//! (Win32 or Slint) renders these strings.

use clickless_windows::settings_help::{ALL, for_id};

const SHIPPABLE: [&str; 23] = [
    "enabled",
    "leader",
    "layout",
    "start_speed",
    "max_speed",
    "ramp_ms",
    "hold_ms",
    "scroll_step",
    "mouse_bindings",
    "column_keys",
    "row_keys",
    "nested_size",
    "nested_keys",
    "auto_free",
    "nudge_enabled",
    "nudge_step",
    "drag_after_select",
    "color_panel",
    "color_border",
    "color_label",
    "color_highlight",
    "color_pointer",
    "opacity",
];

const GAPS: [&str; 5] = [
    "multiplier_bounds",
    "theme_selector",
    "start_at_login",
    "initial_bindings",
    "animation",
];

const ENUM_TOKENS: [&str; 3] = ["click_left", "move_right", "enter_grid"];

#[test]
fn every_shippable_setting_has_help() {
    for id in SHIPPABLE {
        let help = for_id(id).unwrap_or_else(|| panic!("no help entry for {id}"));
        assert!(!help.label.is_empty(), "{id}: label empty");
        assert!(!help.short.is_empty(), "{id}: short help empty");
        assert!(!help.detail.is_empty(), "{id}: detail empty");
        assert!(!help.example.is_empty(), "{id}: example empty");
        assert!(
            !help.label.contains('_'),
            "{id}: label must be plain words, got {:?}",
            help.label
        );
        for token in ENUM_TOKENS {
            assert!(
                !help.label.contains(token),
                "{id}: label leaks an internal token ({token})"
            );
        }
    }
    assert_eq!(
        ALL.len(),
        SHIPPABLE.len(),
        "help table drifted from coverage"
    );
}

#[test]
fn gap_rows_have_no_help_entry() {
    // A control may not ship before its runtime effect exists; the help
    // table enforces the same rule so no shell can render a dead control.
    for id in GAPS {
        assert!(for_id(id).is_none(), "gap row {id} must have no entry");
    }
}

#[test]
fn audit_example_wording_survives() {
    // docs/settings-grid-ux-audit.md fixes these sentences; the table keeps
    // them so shells cannot drift.
    assert_eq!(
        for_id("start_speed").unwrap().short,
        "Pointer speed when movement begins."
    );
    assert_eq!(
        for_id("ramp_ms").unwrap().short,
        "Time taken to accelerate from start to maximum speed."
    );
    assert_eq!(for_id("auto_free").unwrap().label, "Return to pointer mode");
    assert_eq!(
        for_id("drag_after_select").unwrap().short,
        "Hold the mouse button after the final grid key. Release capture to end the drag."
    );
}
