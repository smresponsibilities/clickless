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
