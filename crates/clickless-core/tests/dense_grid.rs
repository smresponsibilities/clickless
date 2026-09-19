use clickless_core::{
    LogicalKey as K,
    grid::{GridConfig, GridNavAction, GridNavigator, Rect},
};

#[test]
fn dense_selection_keeps_context_and_inserts_keyboard_subgrid() {
    let mut nav = GridNavigator::new(1920, 1080, GridConfig::dense());
    nav.activate();
    assert_eq!(
        nav.overlay_frame().unwrap().cells[0].rect,
        Rect::new(0, 0, 192, 36)
    );
    nav.on_key_press(K::D);
    nav.on_key_release(K::D);
    assert_eq!(
        nav.on_key_press(K::G),
        Some(GridNavAction::MoveCursorTo(480, 522))
    );
    nav.on_key_release(K::G);
    let frame = nav.overlay_frame().unwrap();
    assert_eq!(
        frame.cells.iter().filter(|c| c.label.len() == 2).count(),
        299
    );
    let nested: Vec<_> = frame.cells.iter().filter(|c| c.label.len() == 1).collect();
    assert_eq!(nested.len(), 30);
    assert_eq!(nested[0].label, "q");
    assert_eq!(nested[0].rect, Rect::new(384, 504, 19, 12));
    assert_eq!(nested[29].label, "/");
    assert_eq!(
        nav.on_key_press(K::Q),
        Some(GridNavAction::MoveCursorTo(393, 510))
    );
    assert_eq!(
        nav.on_key_release(K::Q),
        Some(GridNavAction::ClickAt(393, 510))
    );
}

#[test]
fn space_click_and_undo_do_not_click_during_selection() {
    let mut nav = GridNavigator::new(1920, 1080, GridConfig::dense());
    nav.activate();
    nav.on_key_press(K::A);
    nav.on_key_press(K::A); // repeat ignored
    assert_eq!(nav.overlay_frame().unwrap().cells.len(), 30);
    nav.on_key_release(K::A);
    nav.on_key_press(K::Q);
    nav.on_key_release(K::Q);
    nav.on_key_press(K::Backspace);
    assert_eq!(nav.overlay_frame().unwrap().cells.len(), 30);
    nav.on_key_release(K::Backspace);
    nav.on_key_press(K::Q);
    assert_eq!(
        nav.on_key_press(K::Space),
        Some(GridNavAction::ClickAt(96, 18))
    );
    assert!(nav.overlay_frame().is_none());
}
