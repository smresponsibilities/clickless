use super::LinuxHook;
use clickless_backend_api::OutputBackend;
use clickless_core::{Layer, LogicalKey};
use std::collections::{HashMap, HashSet};

/// Physical ownership belongs to a device; engine and virtual keys belong to
/// the aggregate. A second keyboard cannot release the first keyboard's key.
#[derive(Default)]
pub(super) struct KeyboardState {
    devices: HashMap<u64, HashMap<u16, bool>>,
    leader_tap: Option<u64>,
    // An output write can fail after forwarding a press but before its release.
    forwarded: HashSet<u16>,
}

impl KeyboardState {
    pub(super) fn key<O: OutputBackend>(
        &mut self,
        hook: &mut LinuxHook<O>,
        device: u64,
        code: u16,
        value: i32,
        now_ms: u64,
    ) -> Result<Vec<(u16, i32)>, String> {
        if !matches!(value, 0..=2) {
            return Err(format!("Invalid keyboard value: {value}"));
        }
        let own = self
            .devices
            .get(&device)
            .and_then(|keys| keys.get(&code))
            .copied();
        let other = self
            .devices
            .iter()
            .filter(|(id, _)| **id != device)
            .find_map(|(_, keys)| keys.get(&code))
            .copied();
        if value == 2 || (value == 1 && own.is_some()) {
            return Ok(if own == Some(true) {
                vec![(code, 2)]
            } else {
                vec![]
            });
        }
        if value == 1 {
            if let Some(forward) = other {
                self.devices
                    .entry(device)
                    .or_default()
                    .insert(code, forward);
                return Ok(vec![]);
            }
        } else {
            let Some(forward) = own else {
                return Ok(vec![]);
            };
            self.devices.get_mut(&device).unwrap().remove(&code);
            if other.is_some() {
                return Ok(vec![]);
            }
            let leader = super::key_code::evdev_to_logical(code) == Some(hook.sm().leader_key());
            if !forward || matches!(code, 29 | 42) || leader {
                hook.process_key_outcome(code, false, now_ms)?;
            }
            if leader {
                let tap = self
                    .leader_tap
                    .take()
                    .is_some_and(|start| now_ms.saturating_sub(start) < hook.sm().hold_ms());
                if !forward && tap {
                    self.forwarded.insert(code);
                    return Ok(vec![(code, 1), (code, 0)]);
                }
            }
            return Ok(if forward { vec![(code, 0)] } else { vec![] });
        }
        let leader = super::key_code::evdev_to_logical(code) == Some(hook.sm().leader_key());
        self.leader_tap = if leader
            && !hook.sm().is_paused()
            && !matches!(
                hook.sm().leader_key(),
                LogicalKey::ShiftLeft | LogicalKey::ControlLeft
            )
            && hook.sm().layer() == Layer::Initial
        {
            Some(now_ms)
        } else {
            None
        };
        let chord = self.devices.values().any(|keys| {
            keys.iter().any(|(key, forward)| {
                *forward
                    && matches!(*key, 29 | 97 | 56 | 100 | 125 | 126)
                    && !(*key == 29
                        && hook.sm().leader_key() == LogicalKey::ControlLeft
                        && hook.sm().layer() != Layer::Initial)
            })
        });
        let outcome = if chord || (code == 29 && !leader && hook.sm().layer() == Layer::Grid) {
            hook.sm_mut().interrupt_pending_taps();
            clickless_core::Outcome::PASS
        } else {
            hook.process_key_outcome(code, true, now_ms)?
        };
        let forward = !outcome.consumed;
        if forward {
            self.forwarded.insert(code);
        }
        self.devices
            .entry(device)
            .or_default()
            .insert(code, forward);
        Ok(if forward { vec![(code, 1)] } else { vec![] })
    }

    pub(super) fn remove<O: OutputBackend>(
        &mut self,
        hook: &mut LinuxHook<O>,
        device: u64,
        now_ms: u64,
    ) -> Result<Vec<(u16, i32)>, String> {
        // Removal must not replay an unused leader tap.
        self.leader_tap = None;
        let codes: Vec<_> = self
            .devices
            .get(&device)
            .map(|keys| keys.keys().copied().collect())
            .unwrap_or_default();
        let mut releases = Vec::new();
        for code in codes {
            releases.extend(self.key(hook, device, code, 0, now_ms)?);
        }
        self.devices.remove(&device);
        Ok(releases)
    }

    pub(super) fn resync_device<O: OutputBackend>(
        &mut self,
        hook: &mut LinuxHook<O>,
        device: u64,
        held_keys: &HashSet<u16>,
        now_ms: u64,
    ) -> Result<Vec<(u16, i32)>, String> {
        self.leader_tap = None;
        let tracked: Vec<_> = self
            .devices
            .get(&device)
            .map(|keys| keys.keys().copied().collect())
            .unwrap_or_default();
        let mut releases = Vec::new();
        for code in tracked {
            if !held_keys.contains(&code) {
                releases.extend(self.key(hook, device, code, 0, now_ms)?);
            }
        }
        for &code in held_keys {
            let already = self
                .devices
                .get(&device)
                .and_then(|keys| keys.get(&code))
                .is_some();
            if !already {
                self.devices.entry(device).or_default().insert(code, false);
            }
        }
        Ok(releases)
    }

    pub(super) fn releases(&self) -> Vec<(u16, i32)> {
        let mut codes: Vec<_> = self.forwarded.iter().copied().collect();
        codes.sort_unstable();
        codes.into_iter().map(|code| (code, 0)).collect()
    }

    pub(super) fn acknowledge_forwarded(&mut self, code: u16, value: i32) {
        if value == 0 {
            self.forwarded.remove(&code);
        } else if value == 1 {
            self.forwarded.insert(code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_backend_api::{Button, Dir, OutputBackend};

    #[derive(Default)]
    struct RecordingOutput;

    impl OutputBackend for RecordingOutput {
        fn move_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _: Button, _: Dir) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn overlapping_forwarded_keys_release_only_after_last_device() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        assert_eq!(state.key(&mut hook, 1, 30, 1, 0).unwrap(), vec![(30, 1)]);
        assert!(state.key(&mut hook, 2, 30, 1, 1).unwrap().is_empty());
        assert!(state.key(&mut hook, 1, 30, 0, 2).unwrap().is_empty());
        assert_eq!(state.key(&mut hook, 2, 30, 0, 3).unwrap(), vec![(30, 0)]);
    }

    #[test]
    fn right_control_on_another_device_preserves_chord() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        state.key(&mut hook, 1, 58, 1, 0).unwrap();
        hook.sm_mut().poll(200);
        state.key(&mut hook, 1, 97, 1, 201).unwrap();
        assert_eq!(state.key(&mut hook, 2, 35, 1, 202).unwrap(), vec![(35, 1)]);
        assert_eq!(state.key(&mut hook, 2, 35, 0, 203).unwrap(), vec![(35, 0)]);
    }

    #[test]
    fn removal_keeps_other_devices_key_owned() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        state.key(&mut hook, 1, 30, 1, 0).unwrap();
        state.key(&mut hook, 2, 30, 1, 1).unwrap();
        assert!(state.remove(&mut hook, 1, 2).unwrap().is_empty());
        assert_eq!(state.remove(&mut hook, 2, 3).unwrap(), vec![(30, 0)]);
        state.acknowledge_forwarded(30, 0);
        assert!(state.releases().is_empty());
    }

    #[test]
    fn invalid_and_unowned_events_do_not_change_ownership() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        assert!(state.key(&mut hook, 1, 30, 3, 0).is_err());
        assert!(state.key(&mut hook, 1, 30, 0, 1).unwrap().is_empty());
        assert!(state.key(&mut hook, 1, 30, 2, 2).unwrap().is_empty());
        assert!(state.releases().is_empty());
    }

    #[test]
    fn cleanup_keeps_a_release_until_output_acknowledges_it() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        state.key(&mut hook, 1, 30, 1, 0).unwrap();
        assert_eq!(state.key(&mut hook, 1, 30, 0, 1).unwrap(), vec![(30, 0)]);
        assert_eq!(state.releases(), vec![(30, 0)]);
        state.acknowledge_forwarded(30, 0);
        assert!(state.releases().is_empty());
    }

    #[test]
    fn resync_device_releases_dropped_keys_while_preserving_other_devices() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        state.key(&mut hook, 1, 30, 1, 0).unwrap();
        state.key(&mut hook, 2, 35, 1, 1).unwrap();
        let current_held = HashSet::new();
        let releases = state.resync_device(&mut hook, 1, &current_held, 2).unwrap();
        assert_eq!(releases, vec![(30, 0)]);
        assert_eq!(state.key(&mut hook, 2, 35, 0, 3).unwrap(), vec![(35, 0)]);
    }

    #[test]
    fn resync_device_does_not_guess_untracked_presses() {
        let mut state = KeyboardState::default();
        let mut hook = crate::LinuxHook::new(RecordingOutput);
        let mut current_held = HashSet::new();
        current_held.insert(45);
        let releases = state.resync_device(&mut hook, 1, &current_held, 0).unwrap();
        assert!(releases.is_empty());
        assert!(state.key(&mut hook, 1, 45, 0, 1).unwrap().is_empty());
    }
}
