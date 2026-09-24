//! Ticket 030: first-run practice over the real StateMachine.
//! No output backend exists in this path, so practice can never click.

use clickless_core::{KeyEvent, LogicalKey, Phase};
use clickless_windows::practice::{PRACTICE_VERSION, Practice, Step};

fn press(practice: &mut Practice, key: LogicalKey, now: u64) {
    practice.key(KeyEvent::new(key, Phase::Press), now);
}

#[test]
fn practice_advances_through_all_steps_on_real_engine() {
    assert_eq!(PRACTICE_VERSION, 2);
    let mut practice = Practice::new();
    assert_eq!(practice.step(), Step::HoldLeader);

    press(&mut practice, LogicalKey::CapsLock, 0);
    press(&mut practice, LogicalKey::Space, 1000);
    assert_eq!(practice.step(), Step::GridPick);

    press(&mut practice, LogicalKey::U, 1100);
    press(&mut practice, LogicalKey::U, 1200);
    assert_eq!(practice.step(), Step::Done);
}

#[test]
fn practice_esc_cancels_from_any_step() {
    let mut practice = Practice::new();
    press(&mut practice, LogicalKey::CapsLock, 0);
    press(&mut practice, LogicalKey::Space, 500);
    press(&mut practice, LogicalKey::Esc, 400);
    assert!(practice.cancelled());
}

#[test]
fn practice_capslock_closes_grid_on_press_or_release() {
    let mut practice = Practice::new();
    press(&mut practice, LogicalKey::CapsLock, 0);
    press(&mut practice, LogicalKey::Space, 1000);
    assert_eq!(practice.step(), Step::GridPick);
    press(&mut practice, LogicalKey::CapsLock, 1100);
    assert!(practice.grid_overlay().is_none());
}
