//! Ticket 030: first-run practice over the real StateMachine.
//! No output backend exists in this path, so practice can never click.

use clickless_core::{KeyEvent, LogicalKey, Phase};
use clickless_windows::practice::{PRACTICE_VERSION, Practice, Step};

fn press(practice: &mut Practice, key: LogicalKey, now: u64) {
    practice.key(KeyEvent::new(key, Phase::Press), now);
}

fn hold(practice: &mut Practice, key: LogicalKey, now: u64) {
    press(practice, key, now);
    practice.key(KeyEvent::new(key, Phase::Release), now + 250);
}

#[test]
fn practice_advances_through_all_steps_on_real_engine() {
    assert_eq!(PRACTICE_VERSION, 6);
    let mut practice = Practice::new();
    assert_eq!(practice.step(), Step::HoldLeader);

    for (key, start) in [
        (LogicalKey::Space, 0),
        (LogicalKey::ControlLeft, 1000),
        (LogicalKey::ShiftLeft, 2000),
    ] {
        assert_eq!(practice.opening_key(), key);
        hold(&mut practice, key, start);
        assert_eq!(practice.step(), Step::GridPick);
        press(&mut practice, LogicalKey::U, start + 300);
        press(&mut practice, LogicalKey::U, start + 400);
        if start < 2000 {
            assert_eq!(practice.step(), Step::HoldLeader);
        }
    }
    assert_eq!(practice.step(), Step::Done);
}

#[test]
fn practice_esc_cancels_from_any_step() {
    let mut practice = Practice::new();
    press(&mut practice, LogicalKey::Space, 0);
    press(&mut practice, LogicalKey::Esc, 400);
    assert!(practice.cancelled());
}

#[test]
fn practice_space_hold_opens_grid() {
    let mut practice = Practice::new();
    hold(&mut practice, LogicalKey::Space, 0);
    assert_eq!(practice.step(), Step::GridPick);
    assert!(practice.grid_overlay().is_some());
}
