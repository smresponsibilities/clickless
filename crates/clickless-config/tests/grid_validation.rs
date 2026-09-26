use clickless_config::Config;

#[test]
fn invalid_grid_settings_are_rejected_without_panicking() {
    for input in [
        "[grid]\nrows = 2\ncols = 2",
        "[grid]\nrows = 65536\ncols = 65536\nkeys = []",
        "[grid]\nrows = 1\ncols = 2\nkeys = ['caps', 'capslock']",
        "[grid]\nnudge_step_px = 0",
        "[grid]\nnudge_step_px = -1",
    ] {
        assert!(Config::parse(input).is_err(), "accepted {input}");
    }
}

#[test]
fn application_defaults_to_dense_but_simple_and_custom_keys_are_available() {
    assert!(Config::default().grid.dense);
    assert!(
        !Config::parse("[grid]\nlayout = 'simple'")
            .unwrap()
            .grid
            .dense
    );
    let custom = Config::parse(
        "[grid]\nlayout = 'dense'\ncolumn_keys = ['a','s']\nrow_keys = ['q','w','e']",
    )
    .unwrap();
    assert_eq!(custom.grid.column_keys.len(), 2);
    assert_eq!(custom.grid.row_keys.len(), 3);
    for invalid in [
        "layout = 'other'",
        "row_keys = []",
        "column_keys = ['a','a']",
        "row_keys = ['space']",
    ] {
        assert!(Config::parse(&format!("[grid]\n{invalid}")).is_err());
    }
}
