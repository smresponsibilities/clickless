//! First-run practice (Ticket 030): a pure flow over the real StateMachine.
//!
//! No output backend exists in this module, so practice can never move the
//! real pointer, click, or scroll. The practice dialog feeds it dialog-local
//! keys while global capture is suspended, and reports step changes as text.

use clickless_core::grid::GridConfig;
use clickless_core::{Action, KeyEvent, Layer, LogicalKey, Phase, StateMachine};

/// Completion value stored in `practice_completed_version`. Bump when the
/// steps change so owners re-run once.
pub const PRACTICE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    HoldLeader,
    MovePointer,
    GridPick,
    Done,
}

pub struct Practice {
    sm: StateMachine,
    step: Step,
    cancelled: bool,
}

impl Default for Practice {
    fn default() -> Self {
        Self::new()
    }
}

impl Practice {
    pub fn new() -> Self {
        let mut sm = StateMachine::new();
        sm.enable_grid(640, 360, GridConfig::dense());
        Self {
            sm,
            step: Step::HoldLeader,
            cancelled: false,
        }
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

    /// Feeds one dialog-local key to the real engine and advances the step.
    /// Esc cancels from any step; keys after Done or cancel are ignored.
    pub fn key(&mut self, event: KeyEvent, now_ms: u64) {
        if self.cancelled || self.step == Step::Done {
            return;
        }
        if event.key == LogicalKey::Esc && event.phase == Phase::Press {
            self.cancelled = true;
            return;
        }
        let action = self.sm.on_event(event, now_ms);
        // Re-check step conditions in a loop so that a single key event
        // can advance through multiple steps (e.g. HoldLeader→MovePointer→GridPick
        // when CapsLock is held long enough for the layer change AND a movement
        // action occurs on the same key event).
        for _ in 0..2 {
            match self.step {
                Step::HoldLeader => {
                    if self.sm.layer() == Layer::Mouse {
                        self.step = Step::MovePointer;
                    }
                }
                Step::MovePointer => {
                    if matches!(
                        action,
                        Some(Action::MoveLeft | Action::MoveRight | Action::MoveUp | Action::MoveDown)
                    ) {
                        self.step = Step::GridPick;
                    }
                }
                Step::GridPick => {
                    if self.nested_count() == 30 {
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
