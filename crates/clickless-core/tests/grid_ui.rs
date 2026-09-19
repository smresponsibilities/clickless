use clickless_core::{
    LogicalKey,
    grid::{GridConfig, GridNavAction, GridNavigator},
};

#[test]
fn selected_subcell_is_highlighted_and_escape_cancels_pending_click() {
    let mut nav = GridNavigator::new(
        900,
        900,
        GridConfig {
            auto_free_mode_after_move: false,
            ..Default::default()
        },
    );
    nav.activate();
    nav.on_key_press(LogicalKey::K);
    nav.on_key_press(LogicalKey::K);
    let frame = nav.overlay_frame().unwrap();
    assert_eq!(frame.highlight, Some(frame.cells[4].rect));
    assert_eq!(frame.pointer, Some((450, 450)));
    assert_eq!(
        nav.on_key_press(LogicalKey::Esc),
        Some(GridNavAction::HideOverlay)
    );
    assert_eq!(nav.on_key_release(LogicalKey::K), None);
    assert!(nav.overlay_frame().is_none());
}
