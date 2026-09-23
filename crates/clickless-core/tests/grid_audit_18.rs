//! Prompt 18 red tests: grid commit contract and invalid-key containment.
//! Public StateMachine boundary only; no sleeps, no real pointer.

use clickless_core::{
    Action, KeyEvent, Layer, LogicalKey, MotionConfig, Phase, StateMachine, default_bindings,
    grid::GridConfig,
};

fn press(key: LogicalKey) -> KeyEvent {
    KeyEvent::new(key, Phase::Press)
}

fn release(key: LogicalKey) -> KeyEvent {
    KeyEvent::new(key, Phase::Release)
}

fn machine_with(config: GridConfig) -> StateMachine {
    let mut bindings = default_bindings();
    bindings.insert(LogicalKey::Space, Action::EnterGrid);
    let mut sm = StateMachine::with_config(LogicalKey::CapsLock, bindings, MotionConfig::default());
    sm.enable_grid(1920, 1080, config);
    sm
}

fn enter_grid(sm: &mut StateMachine) {
    sm.on_event(press(LogicalKey::CapsLock), 0);
    sm.poll(200);
    sm.on_event(press(LogicalKey::Space), 300);
    assert_eq!(sm.layer(), Layer::Grid);
}

#[test]
fn auto_free_without_nudge_leaves_no_grid_layer_behind() {
    let mut sm = machine_with(GridConfig {
        nudge_enabled: false,
        auto_free_mode_after_move: true,
        ..GridConfig::default()
    });
    enter_grid(&mut sm);
    // Level 1: K selects the center cell.
    sm.on_event(press(LogicalKey::K), 400);
    // Level 2: K again completes the selection.
    let outcome = sm.on_event_outcome(press(LogicalKey::K), 500);
    assert!(outcome.consumed);
    assert_eq!(outcome.action, Some(Action::MoveTo(959, 540)));
    // Atomic transition: navigator inactive implies core is Mouse, with no
    // overlay left behind.
    assert_eq!(sm.layer(), Layer::Mouse);
    assert!(sm.grid_overlay().is_none());
}

#[test]
fn invalid_printable_key_in_grid_is_consumed_and_keeps_state() {
    let mut sm = machine_with(GridConfig::default());
    enter_grid(&mut sm);
    let before = sm.grid_overlay().expect("level 1 overlay");
    // Z is not one of the nine simple-grid labels.
    let outcome = sm.on_event_outcome(press(LogicalKey::Z), 400);
    assert!(
        outcome.consumed,
        "invalid grid key must not leak to the app"
    );
    assert_eq!(outcome.action, None);
    let held = sm.on_event_outcome(release(LogicalKey::Z), 410);
    assert!(held.consumed, "release without press must stay balanced");
    assert_eq!(sm.layer(), Layer::Grid);
    assert_eq!(sm.grid_overlay(), Some(before));
}
