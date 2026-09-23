//! Prompt 3 red tests: theme fields, key-name helper, default config path.

use clickless_backend_api::overlay::{BORDER_PX, HIGHLIGHT_ALPHA};
use clickless_config::{Config, ThemeConfig, logical_key_name, parse_logical_key};

#[test]
fn t01_theme_highlight_opacity_and_border_px_round_trip() {
    let mut config = Config::default();
    config.theme.highlight_opacity = 200;
    config.theme.border_px = 3;
    let text = config.to_toml();
    let parsed = Config::parse(&text).unwrap();
    assert_eq!(parsed.theme.highlight_opacity, 200);
    assert_eq!(parsed.theme.border_px, 3);
}

#[test]
fn t02_theme_field_validation_bounds() {
    let mut config = Config::default();
    config.theme.highlight_opacity = 255;
    config.theme.border_px = 16;
    assert!(config.validate().is_ok());

    config.theme.highlight_opacity = 0;
    assert!(config.validate().is_ok());

    config.theme.border_px = 0;
    assert!(config.validate().is_err());
    config.theme.border_px = 17;
    assert!(config.validate().is_err());
}

#[test]
fn t03_to_overlay_theme_maps_the_new_fields() {
    let mut theme = ThemeConfig::default();
    assert_eq!(theme.to_overlay_theme().highlight_alpha, HIGHLIGHT_ALPHA);
    assert_eq!(theme.to_overlay_theme().border_px, BORDER_PX);

    theme.highlight_opacity = 64;
    theme.border_px = 5;
    let overlay = theme.to_overlay_theme();
    assert_eq!(overlay.highlight_alpha, 64);
    assert_eq!(overlay.border_px, 5);
}

#[test]
fn t04_logical_key_name_round_trips_through_parse() {
    for name in ["capslock", "a", "h", "j", "space", "esc", "backspace"] {
        let key = parse_logical_key(name).unwrap();
        assert_eq!(logical_key_name(key), name);
    }
    assert_eq!(logical_key_name(parse_logical_key(";").unwrap()), ";");
    assert_eq!(logical_key_name(parse_logical_key(",").unwrap()), ",");
    assert_eq!(logical_key_name(parse_logical_key(".").unwrap()), ".");
}

#[test]
fn t05_default_config_path_prefers_the_clickless_directory() {
    // SAFETY: single-threaded test process; env mutation is the point.
    #[cfg(windows)]
    unsafe {
        std::env::set_var("APPDATA", "C:\\fake-appdata-for-test");
        let path = clickless_config::default_config_path().unwrap();
        assert!(path.to_string_lossy().contains("clickless"));
        assert!(path.to_string_lossy().ends_with("clickless.toml"));
    }
    #[cfg(not(windows))]
    unsafe {
        std::env::set_var("HOME", "/fake-home-for-test");
        let path = clickless_config::default_config_path().unwrap();
        assert!(path.to_string_lossy().contains("clickless"));
        assert!(path.to_string_lossy().ends_with("clickless.toml"));
    }
}
