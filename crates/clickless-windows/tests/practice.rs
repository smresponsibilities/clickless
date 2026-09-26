//! Ticket 030: first-run practice over the real StateMachine.
//! No output backend exists in this path, so practice can never click.

#![cfg(windows)]

use clickless_core::{KeyEvent, LogicalKey, Phase};
use clickless_windows::practice::{PRACTICE_VERSION, Practice, Step};

fn press(practice: &mut Practice, key: LogicalKey, now: u64) {
    practice.key(KeyEvent::new(key, Phase::Press), now);
}

fn hold(practice: &mut Practice, key: LogicalKey, now: u64) {
    press(practice, key, now);
    practice.key(KeyEvent::new(key, Phase::Release), now + 250);
}

/// First run chooses the default CapsLock key before the pointer lesson.
fn choose_capslock(practice: &mut Practice) {
    press(practice, LogicalKey::CapsLock, 0);
    assert_eq!(practice.step(), Step::HoldLeader);
}

#[test]
fn practice_advances_from_hold_to_click_on_real_engine() {
    assert_eq!(PRACTICE_VERSION, 8);
    let mut practice = Practice::new();
    assert_eq!(practice.step(), Step::SelectActivationKey);
    choose_capslock(&mut practice);

    // Hold the chosen leader to open the grid.
    hold(&mut practice, LogicalKey::CapsLock, 100);
    assert_eq!(practice.step(), Step::GridPick);

    // Outer label then inner label performs the click and finishes.
    press(&mut practice, LogicalKey::U, 400);
    press(&mut practice, LogicalKey::U, 500);
    assert_eq!(practice.step(), Step::Done);
}

#[test]
fn practice_esc_cancels_from_any_step() {
    let mut practice = Practice::new();
    choose_capslock(&mut practice);
    press(&mut practice, LogicalKey::CapsLock, 100);
    press(&mut practice, LogicalKey::Esc, 400);
    assert!(practice.cancelled());
}

#[test]
fn practice_space_hold_opens_grid() {
    let mut practice = Practice::new();
    choose_capslock(&mut practice);
    hold(&mut practice, LogicalKey::CapsLock, 0);
    assert_eq!(practice.step(), Step::GridPick);
    assert!(practice.grid_overlay().is_some());
}

#[test]
fn practice_offers_activation_key_choice_first() {
    let mut practice = Practice::new();
    assert_eq!(practice.step(), Step::SelectActivationKey);
    assert_eq!(practice.chosen_leader(), LogicalKey::CapsLock);

    // A non-offered key does nothing; an offered key advances the flow.
    press(&mut practice, LogicalKey::J, 0);
    assert_eq!(practice.step(), Step::SelectActivationKey);
    press(&mut practice, LogicalKey::ControlLeft, 100);
    assert_eq!(practice.chosen_leader(), LogicalKey::ControlLeft);
    assert_eq!(practice.opening_key(), LogicalKey::ControlLeft);
    assert_eq!(practice.step(), Step::HoldLeader);
}

/// Ticket 045: a returning user is taught the saved key, not CapsLock.
#[test]
fn practice_uses_saved_leader_and_grid_style() {
    let mut practice = Practice::with_setup(LogicalKey::Space, true);
    assert_eq!(practice.step(), Step::HoldLeader);
    assert_eq!(practice.opening_key(), LogicalKey::Space);
    assert!(practice.dense());
    assert_ne!(practice.opening_key(), LogicalKey::CapsLock);

    hold(&mut practice, LogicalKey::Space, 0);
    assert_eq!(practice.step(), Step::GridPick);
    let overlay = practice.grid_overlay().expect("dense grid open");
    assert!(!overlay.cells.is_empty());
}
