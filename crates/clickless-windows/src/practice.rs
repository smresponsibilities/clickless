//! First-run practice (Ticket 030): a pure flow over the real StateMachine.
//!
//! No output backend exists in this module, so practice can never move the
//! real pointer, click, or scroll. The practice dialog feeds it dialog-local
//! keys while global capture is suspended, and reports step changes as text.

use clickless_core::grid::GridConfig;
use clickless_core::{
    Action, KeyEvent, Layer, LogicalKey, MotionConfig, Phase, StateMachine, default_bindings,
};

/// Completion value stored in `practice_completed_version`. Bump when the
/// steps change so owners re-run once.
pub const PRACTICE_VERSION: u32 = 6;

const PRACTICE_KEYS: [LogicalKey; 3] = [
    LogicalKey::Space,
    LogicalKey::ControlLeft,
    LogicalKey::ShiftLeft,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    HoldLeader,
    GridPick,
    Done,
}

pub struct Practice {
    sm: StateMachine,
    step: Step,
    key_index: usize,
    opening_pressed_at: Option<u64>,
    cancelled: bool,
}

impl Default for Practice {
    fn default() -> Self {
        Self::new()
    }
}

impl Practice {
    pub fn new() -> Self {
        Self {
            sm: new_machine(),
            step: Step::HoldLeader,
            key_index: 0,
            opening_pressed_at: None,
            cancelled: false,
        }
    }

    pub fn opening_key(&self) -> LogicalKey {
        PRACTICE_KEYS[self.key_index]
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
                Step::HoldLeader => {
                    if self.sm.layer() == Layer::Grid {
                        self.step = Step::GridPick;
                    }
                }
                Step::GridPick => {
                    if matches!(action, Some(Action::ClickAt(_, _))) {
                        self.key_index += 1;
                        if self.key_index == PRACTICE_KEYS.len() {
                            self.step = Step::Done;
                        } else {
                            self.sm = new_machine();
                            self.opening_pressed_at = None;
                            self.step = Step::HoldLeader;
                        }
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

fn new_machine() -> StateMachine {
    let mut sm = StateMachine::with_config(
        LogicalKey::CapsLock,
        default_bindings(),
        MotionConfig::default(),
    );
    sm.enable_grid(1920, 1080, GridConfig::simple());
    sm
}
