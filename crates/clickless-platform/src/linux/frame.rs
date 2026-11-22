//! evdev discards events after SYN_DROPPED through the next SYN_REPORT.
#[derive(Default)]
pub(super) struct FrameRecovery(bool);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecoveryAction {
    Forward,
    Discard,
    Resync,
}

impl FrameRecovery {
    pub(super) fn event(&mut self, kind: u16, code: u16) -> RecoveryAction {
        if kind == 0 && code == 3 {
            self.0 = true;
            RecoveryAction::Discard
        } else if self.0 {
            if kind == 0 && code == 0 {
                self.0 = false;
                RecoveryAction::Resync
            } else {
                RecoveryAction::Discard
            }
        } else {
            RecoveryAction::Forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_discards_stale_events_through_report_then_resyncs_once() {
        let mut recovery = FrameRecovery::default();
        assert_eq!(recovery.event(1, 30), RecoveryAction::Forward);
        assert_eq!(recovery.event(0, 3), RecoveryAction::Discard);
        assert_eq!(recovery.event(1, 30), RecoveryAction::Discard);
        assert_eq!(recovery.event(0, 3), RecoveryAction::Discard);
        assert_eq!(recovery.event(0, 0), RecoveryAction::Resync);
        assert_eq!(recovery.event(1, 31), RecoveryAction::Forward);
        assert_eq!(recovery.event(0, 0), RecoveryAction::Forward);
        let mut other_keyboard = FrameRecovery::default();
        recovery.event(0, 3);
        assert_eq!(other_keyboard.event(1, 30), RecoveryAction::Forward);
    }
}
