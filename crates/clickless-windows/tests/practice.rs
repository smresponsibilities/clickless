//! Ticket 030: first-run practice over the real StateMachine.
//! No output backend exists in this path, so practice can never click.

use clickless_core::{KeyEvent, LogicalKey, Phase};
use clickless_windows::practice::{PRACTICE_VERSION, Practice, Step};

fn press(practice: &mut Practice, key: LogicalKey, now: u64) {
    practice.key(KeyEvent::new(key, Phase::Press), now);
}

fn release(practice: &mut Practice, key: LogicalKey, now: u64) {
    practice.key(KeyEvent::new(key, Phase::Release), now);
}

#[test]
fn practice_advances_through_all_steps_on_real_engine() {
    assert_eq!(PRACTICE_VERSION, 1);
    let mut practice = Practice::new();
    assert_eq!(practice.step(), Step::HoldLeader);

    press(&mut practice, LogicalKey::CapsLock, 0);
    press(&mut practice, LogicalKey::J, 300);
    assert_eq!(practice.step(), Step::GridPick);

    press(&mut practice, LogicalKey::Space, 500);
    press(&mut practice, LogicalKey::D, 600);
    release(&mut practice, LogicalKey::D, 650);
    press(&mut practice, LogicalKey::G, 700);
    assert_eq!(practice.step(), Step::Done);
    assert_eq!(practice.nested_count(), 30);
}

#[test]
fn practice_esc_cancels_from_any_step() {
    let mut practice = Practice::new();
    press(&mut practice, LogicalKey::CapsLock, 0);
    press(&mut practice, LogicalKey::J, 300);
    press(&mut practice, LogicalKey::Esc, 400);
    assert!(practice.cancelled());
}
