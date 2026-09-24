//! First-run practice (Ticket 030): a pure flow over the real StateMachine.
//!
//! No output backend exists in this module, so practice can never move the
//! real pointer, click, or scroll. The practice dialog feeds it dialog-local
//! keys while global capture is suspended, and reports step changes as text.

use clickless_core::grid::GridConfig;
use clickless_core::{KeyEvent, Layer, LogicalKey, Phase, StateMachine};

/// Completion value stored in `practice_completed_version`. Bump when the
/// steps change so owners re-run once.
pub const PRACTICE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    HoldLeader,
    GridPick,
    Done,
}

pub struct Practice {
    sm: StateMachine,
    step: Step,
    cancelled: bool,
    leader_down: bool,
}

impl Default for Practice {
    fn default() -> Self {
        Self::new()
    }
}

impl Practice {
    pub fn new() -> Self {
        let mut sm = StateMachine::new();
        sm.enable_grid(1920, 1080, GridConfig::simple());
        Self {
            sm,
            step: Step::HoldLeader,
            cancelled: false,
            leader_down: false,
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

    pub fn grid_overlay(&self) -> Option<clickless_core::grid::OverlayFrame> {
        self.sm.grid_overlay()
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
        if event.key == LogicalKey::CapsLock {
            self.leader_down = event.phase == Phase::Press;
            if self.step == Step::GridPick && event.phase == Phase::Release {
                self.sm
                    .on_event(KeyEvent::new(LogicalKey::CapsLock, Phase::Release), now_ms);
                return;
            }
        }
        if self.step == Step::GridPick && event.key == LogicalKey::CapsLock {
            // Practice treats CapsLock as an explicit grid-close command.
            // This also handles a second press when the OS emits no release
            // between repeated key presses.
            self.sm
                .on_event(KeyEvent::new(LogicalKey::CapsLock, Phase::Release), now_ms);
            return;
        }
        if self.step == Step::HoldLeader
            && event.key == LogicalKey::Space
            && event.phase == Phase::Press
            && self.leader_down
        {
            self.sm.show_grid();
        } else {
            self.sm.on_event(event, now_ms);
        }
        // Re-check after every event so CapsLock can enter the grid flow
        // without requiring an unrelated pointer-movement exercise.
        for _ in 0..2 {
            match self.step {
                Step::HoldLeader => {
                    if matches!(self.sm.layer(), Layer::Mouse | Layer::Grid) {
                        self.step = Step::GridPick;
                    }
                }
                Step::GridPick => {
                    if self.sm.grid_overlay().is_none()
                        && event.key != LogicalKey::Space
                        && event.key != LogicalKey::CapsLock
                    {
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
