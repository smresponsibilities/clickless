//! First-run practice (Ticket 030): a pure flow over the real StateMachine.
//!
//! No output backend exists in this module, so practice can never move the
//! real pointer, click, or scroll. The practice dialog feeds it dialog-local
//! keys while global capture is suspended, and reports step changes as text.
//!
//! Ticket 045: practice teaches the leader key and grid style that are
//! actually saved. `new` is the first-run chooser; `with_setup` is the
//! returning flow, so a CapsLock-only lesson is never shown to a user who
//! saved a different activation key.

use clickless_core::grid::GridConfig;
use clickless_core::{
    Action, KeyEvent, Layer, LogicalKey, MotionConfig, Phase, StateMachine, default_bindings,
};

/// Completion value stored in `practice_completed_version`. Bump when the
/// steps change so owners re-run once.
pub const PRACTICE_VERSION: u32 = 8;

/// Activation keys a new user can pick from. CapsLock stays the default so an
/// owner who is happy with it keeps the current setup; the modifiers are
/// non-toggle keys that never flip a lock state.
pub const ACTIVATION_KEYS: [LogicalKey; 4] = [
    LogicalKey::CapsLock,
    LogicalKey::Space,
    LogicalKey::ControlLeft,
    LogicalKey::ShiftLeft,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    SelectActivationKey,
    HoldLeader,
    GridPick,
    Done,
}

pub struct Practice {
    sm: StateMachine,
    step: Step,
    chosen_leader: LogicalKey,
    dense: bool,
    opening_pressed_at: Option<u64>,
    cancelled: bool,
}

impl Default for Practice {
    fn default() -> Self {
        Self::new()
    }
}

impl Practice {
    /// First run: start by choosing the activation key.
    pub fn new() -> Self {
        Self {
            sm: machine(LogicalKey::CapsLock, false),
            step: Step::SelectActivationKey,
            chosen_leader: LogicalKey::CapsLock,
            dense: false,
            opening_pressed_at: None,
            cancelled: false,
        }
    }

    /// Returning flow: teach the leader key and grid style that are saved.
    pub fn with_setup(leader: LogicalKey, dense: bool) -> Self {
        Self {
            sm: machine(leader, dense),
            step: Step::HoldLeader,
            chosen_leader: leader,
            dense,
            opening_pressed_at: None,
            cancelled: false,
        }
    }

    /// The activation key the lesson teaches (the saved key, or the choice).
    pub fn opening_key(&self) -> LogicalKey {
        self.chosen_leader
    }

    /// The activation key the user picked (or the CapsLock default).
    pub fn chosen_leader(&self) -> LogicalKey {
        self.chosen_leader
    }

    /// True when the lesson uses the dense grid style.
    pub fn dense(&self) -> bool {
        self.dense
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Nested one-char label count in the current overlay, for UI progress.
    pub fn nested_count(&self) -> usize {
        self.sm
            .grid_overlay()
            .map(|frame| {
                frame
                    .cells
                    .iter()
                    .filter(|cell| cell.label.chars().count() == 1)
                    .count()
            })
            .unwrap_or(0)
    }

    pub fn grid_overlay(&self) -> Option<clickless_core::grid::OverlayFrame> {
        self.sm.grid_overlay()
    }

    /// Feeds one dialog-local key to the real engine and advances the step.
    /// Esc cancels from any step; keys after Done or cancel are ignored.
    ///
    /// In `SelectActivationKey` the user presses the key they want to use, so
    /// no config token names are needed. Only offered keys advance the flow.
    pub fn key(&mut self, event: KeyEvent, now_ms: u64) {
        if self.cancelled || self.step == Step::Done {
            return;
        }
        if event.key == LogicalKey::Esc && event.phase == Phase::Press {
            self.cancelled = true;
            return;
        }
        if self.step == Step::SelectActivationKey {
            if event.phase == Phase::Press && ACTIVATION_KEYS.contains(&event.key) {
                self.chosen_leader = event.key;
                self.sm = machine(self.chosen_leader, self.dense);
                self.step = Step::HoldLeader;
            }
            return;
        }
        let key = self.opening_key();
        let opens_grid = if self.step == Step::HoldLeader && event.key == key {
            match event.phase {
                Phase::Press => {
                    self.opening_pressed_at = Some(now_ms);
                    false
                }
                Phase::Release => self
                    .opening_pressed_at
                    .take()
                    .is_some_and(|start| now_ms.saturating_sub(start) >= 200),
            }
        } else {
            false
        };
        let action = if opens_grid {
            self.sm.show_grid()
        } else if self.step == Step::HoldLeader && event.key == key {
            None
        } else {
            self.sm.on_event(event, now_ms)
        };
        for _ in 0..2 {
            match self.step {
                Step::SelectActivationKey => {}
                Step::HoldLeader => {
                    if self.sm.layer() == Layer::Grid {
                        self.step = Step::GridPick;
                    }
                }
                Step::GridPick => {
                    if matches!(action, Some(Action::ClickAt(_, _))) {
                        self.step = Step::Done;
                    }
                }
                Step::Done => {}
            }
        }
    }

    /// Cancels practice (Esc key, focus loss, Skip). Never touches output.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

fn machine(leader: LogicalKey, dense: bool) -> StateMachine {
    let mut sm = StateMachine::with_config(leader, default_bindings(), MotionConfig::default());
    let grid = if dense {
        GridConfig::dense()
    } else {
        GridConfig::simple()
    };
    sm.enable_grid(1920, 1080, grid);
    sm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(practice: &mut Practice, key: LogicalKey, now: u64) {
        practice.key(KeyEvent::new(key, Phase::Press), now);
    }

    fn hold(practice: &mut Practice, key: LogicalKey, now: u64) {
        press(practice, key, now);
        practice.key(KeyEvent::new(key, Phase::Release), now + 250);
    }

    #[test]
    fn new_practice_starts_at_activation_key_choice() {
        let practice = Practice::new();
        assert_eq!(practice.step(), Step::SelectActivationKey);
        assert_eq!(practice.chosen_leader(), LogicalKey::CapsLock);
    }

    #[test]
    fn offered_key_selects_leader_and_starts_practice() {
        let mut practice = Practice::new();
        press(&mut practice, LogicalKey::Space, 0);
        assert_eq!(practice.chosen_leader(), LogicalKey::Space);
        assert_eq!(practice.opening_key(), LogicalKey::Space);
        assert_eq!(practice.step(), Step::HoldLeader);
    }

    #[test]
    fn unrelated_key_does_not_advance_selection() {
        let mut practice = Practice::new();
        press(&mut practice, LogicalKey::J, 0);
        assert_eq!(practice.step(), Step::SelectActivationKey);
    }

    #[test]
    fn returning_setup_skips_the_chooser_and_teaches_saved_key() {
        let practice = Practice::with_setup(LogicalKey::Space, true);
        assert_eq!(practice.step(), Step::HoldLeader);
        assert_eq!(practice.opening_key(), LogicalKey::Space);
        assert!(practice.dense());
    }

    #[test]
    fn saved_leader_hold_opens_the_grid() {
        let mut practice = Practice::with_setup(LogicalKey::ControlLeft, false);
        hold(&mut practice, LogicalKey::ControlLeft, 0);
        assert_eq!(practice.step(), Step::GridPick);
        assert!(practice.grid_overlay().is_some());
    }
}
