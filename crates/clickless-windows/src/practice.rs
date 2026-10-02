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

    /// Readable name of an activation key. Lives here so the lesson copy and
    /// the home screen can never spell the same key differently.
    pub fn key_name(key: LogicalKey) -> &'static str {
        clickless_core::home::key_name(key)
    }

    /// The keys the user can press right now, in the order they are shown.
    /// Read from the live overlay, so the list is never a guess: it is exactly
    /// the labels on screen. Empty before the grid opens.
    pub fn visible_keys(&self) -> Vec<String> {
        self.sm
            .grid_overlay()
            .map(|frame| frame.cells.iter().map(|cell| cell.label.clone()).collect())
            .unwrap_or_default()
    }

    /// The exact instruction for the current step. Each step names the key to
    /// press next and how to recover, so a user never has to guess.
    pub fn instruction(&self) -> String {
        if self.cancelled {
            return "Practice cancelled. Start begins again; Esc closes.".to_string();
        }
        match self.step {
            Step::SelectActivationKey => format!(
                "Step 1 of 3: choose your activation key.\n\n\
                 Press the key you want to hold to open the grid: CapsLock, Space, \
                 Left Ctrl or Left Shift. Now press {}.\n\
                 Esc cancels.",
                Self::key_name(self.chosen_leader)
            ),
            Step::HoldLeader => format!(
                "Step 2 of 3: hold {} to open the grid.\n\n\
                 In this safe lesson, hold for 200 ms, then release to keep the grid open. Outside practice, releasing the activation key closes its grid.\n\
                 Esc cancels.",
                Self::key_name(self.opening_key())
            ),
            Step::GridPick => {
                let keys = self.visible_keys();
                if keys.is_empty() {
                    return "Step 3 of 3: the grid is closed.\n\n\
                            Hold the activation key again to reopen it.\n\
                            Esc cancels."
                        .to_string();
                }
                let listed = keys
                    .iter()
                    .take(12)
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" ");
                let more = keys.len().saturating_sub(12);
                let tail = if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                };
                format!(
                    "Step 3 of 3: choose a target.\n\n\
                     Press one of these labels: {listed}{tail}.\n\
                     The first press moves the pointer. Press the same label again to click it.\n\
                     Esc cancels the grid; Backspace goes back one level."
                )
            }
            Step::Done => "Done. You moved, chose a target and clicked it.\n\n\
                           Finish saves completion and closes. Practice again is in Settings."
                .to_string(),
        }
    }

    /// Short status line: the live overlay progress, or the step number.
    pub fn status(&self) -> String {
        if self.cancelled {
            return "Start begins again; Esc closes.".to_string();
        }
        match self.step {
            Step::SelectActivationKey => format!("Now: {}", Self::key_name(self.chosen_leader)),
            Step::HoldLeader => format!("Now: hold {}", Self::key_name(self.opening_key())),
            Step::GridPick => {
                let open = self.sm.grid_overlay().is_some();
                let count = self.visible_keys().len();
                if open {
                    format!("Grid open. {count} labels on screen. Pick one.")
                } else {
                    format!("Nested targets so far: {}", self.nested_count())
                }
            }
            Step::Done => "Lesson complete.".to_string(),
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
                    self.opening_pressed_at.get_or_insert(now_ms);
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
        press(practice, key, now + 220);
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

    // Ticket 050: every step names the exact key to press next.

    #[test]
    fn t05_each_step_names_the_exact_key_and_a_recovery_path() {
        let mut practice = Practice::new();
        // Step 1 names the key to press and the alternatives.
        let step1 = practice.instruction();
        assert!(step1.contains("CapsLock"), "{step1}");
        assert!(step1.contains("Space"), "{step1}");
        assert!(step1.contains("Esc"), "every step must offer a way out");
        assert!(
            practice.status().contains("CapsLock"),
            "{}",
            practice.status()
        );

        // Step 2 names the saved key to hold.
        press(&mut practice, LogicalKey::Space, 0);
        let step2 = practice.instruction();
        assert!(
            step2.contains("Space"),
            "step 2 must name the chosen key: {step2}"
        );
        assert!(
            !step2.contains("CapsLock"),
            "a Space user must not be told CapsLock"
        );
        assert!(
            practice.status().contains("hold Space"),
            "{}",
            practice.status()
        );

        // Step 3 names the labels that are actually on screen.
        hold(&mut practice, LogicalKey::Space, 100);
        assert_eq!(practice.step(), Step::GridPick);
        let step3 = practice.instruction();
        let visible = practice.visible_keys();
        assert!(!visible.is_empty(), "the grid must be open");
        for key in visible.iter().take(12) {
            assert!(
                step3.contains(key.as_str()),
                "step 3 must offer the on-screen key {key}: {step3}"
            );
        }
        assert!(step3.contains("Backspace"), "recovery: {step3}");
        assert!(step3.contains("Esc"), "recovery: {step3}");
    }

    #[test]
    fn t06_step_three_lists_labels_from_the_overlay_not_a_fixed_list() {
        // Simple grid: 9 labels. Dense: many more, summarised with a count.
        let mut simple = Practice::with_setup(LogicalKey::CapsLock, false);
        hold(&mut simple, LogicalKey::CapsLock, 0);
        assert_eq!(simple.visible_keys().len(), 9);
        assert!(!simple.instruction().contains("more"));

        let mut dense = Practice::with_setup(LogicalKey::CapsLock, true);
        hold(&mut dense, LogicalKey::CapsLock, 0);
        let keys = dense.visible_keys();
        assert!(keys.len() > 12, "the dense lesson must have many labels");
        let text = dense.instruction();
        assert!(
            text.contains(&format!("{} more", keys.len() - 12)),
            "the tail must report the real remaining count: {text}"
        );
    }

    #[test]
    fn t07_done_and_cancelled_steps_say_what_happens_next() {
        let mut practice = Practice::with_setup(LogicalKey::CapsLock, false);
        hold(&mut practice, LogicalKey::CapsLock, 0);
        press(&mut practice, LogicalKey::U, 400);
        press(&mut practice, LogicalKey::U, 500);
        assert_eq!(practice.step(), Step::Done);
        let done = practice.instruction();
        assert!(done.contains("Finish"), "{done}");
        assert!(
            done.contains("moved"),
            "the lesson confirms what was learned: {done}"
        );

        let mut cancelled = Practice::new();
        cancelled.cancel();
        assert!(cancelled.instruction().contains("cancelled"));
        assert!(cancelled.status().contains("Start begins again"));
    }

    #[test]
    fn t08_key_names_match_the_home_screen() {
        // One spelling per key, shared with the home screen.
        for key in ACTIVATION_KEYS {
            assert_eq!(
                Practice::key_name(key),
                clickless_core::home::key_name(key),
                "the lesson and the home screen must spell {:?} the same way",
                key
            );
        }
    }
}
