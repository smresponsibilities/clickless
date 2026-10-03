use super::LinuxHook;
use clickless_backend_api::OutputBackend;
use evdev::{EventType, InputEvent, Key, raw_stream::RawDevice, uinput::VirtualDeviceBuilder};
use std::{
    collections::{HashMap, HashSet},
    os::fd::AsRawFd,
    time::Instant,
};

const VIRTUAL_NAME: &str = "Clickless virtual keyboard";

fn is_wayland_session(session_type: Option<&str>, wayland_display: bool) -> bool {
    wayland_display || session_type.is_some_and(|session| session.eq_ignore_ascii_case("wayland"))
}

fn shortcut_held(
    keys: &HashMap<u16, bool>,
    leader: clickless_core::LogicalKey,
    layer: clickless_core::Layer,
) -> bool {
    keys.iter().any(|(key, forwarded)| {
        *forwarded
            && matches!(*key, 29 | 97 | 56 | 100 | 125 | 126)
            && !(*key == 29
                && leader == clickless_core::LogicalKey::ControlLeft
                && layer != clickless_core::Layer::Initial)
    })
}

fn prepare(dev: &RawDevice) -> Result<evdev::uinput::VirtualDevice, String> {
    let setup = || -> std::io::Result<_> {
        // Do not grab hybrid devices whose events cannot be reproduced.
        if dev.supported_events().iter().any(|kind| {
            !matches!(
                kind,
                EventType::SYNCHRONIZATION
                    | EventType::KEY
                    | EventType::MISC
                    | EventType::RELATIVE
                    | EventType::SWITCH
                    | EventType::LED
                    | EventType::REPEAT
            )
        }) {
            return Err(std::io::Error::other(
                "unsupported keyboard event capabilities",
            ));
        }
        let mut builder = VirtualDeviceBuilder::new()?
            .name(VIRTUAL_NAME)
            .input_id(evdev::InputId::new(evdev::BusType::BUS_VIRTUAL, 0, 0, 1))
            .with_keys(dev.supported_keys().expect("keyboard checked"))?;
        if let Some(misc) = dev.misc_properties() {
            builder = builder.with_msc(misc)?;
        }
        if let Some(axes) = dev.supported_relative_axes() {
            builder = builder.with_relative_axes(axes)?;
        }
        if let Some(switches) = dev.supported_switches() {
            builder = builder.with_switches(switches)?;
        }
        let mut output = builder.build()?;
        output.emit(&[])?;
        let fd = dev.as_raw_fd();
        // SAFETY: fd belongs to the live device; fcntl neither retains nor closes it.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(output)
    };
    setup().map_err(|error| format!("Keyboard forwarding setup failed before capture: {error}"))
}

pub(super) fn run<O: OutputBackend>(
    mut hook: LinuxHook<O>,
    mut is_running: impl FnMut() -> bool,
) -> Result<(), String> {
    if is_wayland_session(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
    ) {
        return hook.finish_input(
            Err("Wayland keyboard capture is unsupported; use an X11 desktop session".into()),
            || Ok(()),
        );
    }
    let setup = || -> Result<_, String> {
        let entries = std::fs::read_dir("/dev/input")
            .map_err(|error| format!("Cannot enumerate /dev/input: {error}"))?;
        for entry in entries.flatten() {
            let Ok(dev) = RawDevice::open(entry.path()) else {
                continue;
            };
            if dev.name() == Some(VIRTUAL_NAME)
                || !dev.supported_keys().is_some_and(|keys| {
                    keys.contains(Key::KEY_CAPSLOCK) && keys.contains(Key::KEY_A)
                })
            {
                continue;
            }
            if dev
                .get_key_state()
                .map_err(|error| error.to_string())?
                .iter()
                .next()
                .is_some()
            {
                return Err("Release all keyboard keys before starting Clickless".into());
            }
            let output = prepare(&dev)?;
            return Ok((dev, output));
        }
        Err("No accessible physical keyboard found in /dev/input".into())
    };
    let (mut dev, mut output) = match setup() {
        Ok(pair) => pair,
        Err(error) => return hook.finish_input(Err(error), || Ok(())),
    };
    if let Err(error) = dev.grab() {
        return hook.finish_input(Err(format!("Keyboard grab failed: {error}")), || Ok(()));
    }
    // Each press owns its repeats and release, even if capture changes mid-hold.
    let mut keys: HashMap<u16, bool> = HashMap::new();
    // Include keys from failed/partial writes, whose delivery is uncertain.
    let mut forwarded_keys = HashSet::new();
    let mut frame = Vec::new();
    let start = Instant::now();
    let mut last_tick = start;
    let mut leader_tap = None;
    let result = (|| -> Result<(), String> {
        if dev
            .get_key_state()
            .map_err(|error| error.to_string())?
            .iter()
            .next()
            .is_some()
        {
            return Err("Keyboard changed during capture setup; restart with keys released".into());
        }
        while is_running() {
            match dev.fetch_events() {
                Ok(events) => {
                    for event in events {
                        let now_ms = start.elapsed().as_millis() as u64;
                        match event.event_type() {
                            EventType::SYNCHRONIZATION if event.code() == 3 => {
                                return Err(
                                    "Keyboard events lost (SYN_DROPPED); capture stopped".into()
                                );
                            }
                            EventType::SYNCHRONIZATION if event.code() == 0 => {
                                output.emit(&frame).map_err(|error| {
                                    format!("Keyboard forwarding failed: {error}")
                                })?;
                                frame.clear();
                            }
                            EventType::KEY => {
                                let code = event.code();
                                let value = event.value();
                                if !matches!(value, 0..=2) {
                                    return Err(format!("Invalid keyboard value: {value}"));
                                }
                                let logical = super::key_code::evdev_to_logical(code);
                                let is_leader = logical == Some(hook.sm().leader_key());
                                let existing = keys.get(&code).copied();
                                if value == 1 && existing.is_none() {
                                    if is_leader
                                        && !hook.sm().is_paused()
                                        && !matches!(
                                            hook.sm().leader_key(),
                                            clickless_core::LogicalKey::ShiftLeft
                                                | clickless_core::LogicalKey::ControlLeft
                                        )
                                        && hook.sm().layer() == clickless_core::Layer::Initial
                                    {
                                        leader_tap = Some(now_ms);
                                    } else {
                                        leader_tap = None;
                                    }
                                }
                                // Alt, Super and either Ctrl preserve application chords.
                                let chord =
                                    shortcut_held(&keys, hook.sm().leader_key(), hook.sm().layer());
                                let forward = if value == 2 || (value == 1 && existing.is_some()) {
                                    existing.unwrap_or(false)
                                } else {
                                    let engine_modifier = matches!(code, 29 | 42) || is_leader;
                                    let pass = (existing == Some(true) && !engine_modifier)
                                        || (chord && value == 1)
                                        || (code == 29
                                            && !is_leader
                                            && hook.sm().layer() == clickless_core::Layer::Grid);
                                    let outcome = if pass {
                                        if value == 1 {
                                            hook.sm_mut().interrupt_pending_taps();
                                        }
                                        clickless_core::Outcome::PASS
                                    } else {
                                        hook.process_key_outcome(code, value == 1, now_ms)?
                                    };
                                    if value == 1 {
                                        keys.insert(code, !outcome.consumed);
                                        !outcome.consumed
                                    } else {
                                        keys.remove(&code).unwrap_or(false)
                                    }
                                };
                                if value == 0 && is_leader {
                                    if !forward
                                        && leader_tap.take().is_some_and(|pressed| {
                                            now_ms.saturating_sub(pressed) < hook.sm().hold_ms()
                                        })
                                    {
                                        forwarded_keys.insert(code);
                                        frame.push(InputEvent::new(EventType::KEY, code, 1));
                                        frame.push(event);
                                    }
                                    leader_tap = None;
                                }
                                if forward {
                                    forwarded_keys.insert(code);
                                    frame.push(event);
                                }
                            }
                            EventType::LED | EventType::REPEAT => {}
                            _ => frame.push(event),
                        }
                        if frame.len() > 4096 {
                            return Err("Keyboard frame exceeds 4096 events".into());
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(format!("Device read error: {error}")),
            }
            let now = Instant::now();
            let dt = now.duration_since(last_tick).as_millis() as u64;
            if dt >= 10 {
                hook.sm_mut().poll(start.elapsed().as_millis() as u64);
                hook.sync_overlay()?;
                hook.tick(dt)?;
                last_tick = now;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    })();
    hook.finish_input(result, || {
        let releases: Vec<_> = forwarded_keys
            .into_iter()
            .map(|code| InputEvent::new(EventType::KEY, code, 0))
            .collect();
        let release = output
            .emit(&releases)
            .map_err(|error| format!("Virtual key release failed: {error}"));
        let ungrab = dev
            .ungrab()
            .map_err(|error| format!("Keyboard ungrab failed: {error}"));
        match (release, ungrab) {
            (Err(a), Err(b)) => Err(format!("{a}; {b}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
            _ => Ok(()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::{Layer, LogicalKey};

    #[test]
    fn wayland_session_is_rejected_even_with_xwayland_available() {
        assert!(is_wayland_session(Some("wayland"), false));
        assert!(is_wayland_session(Some("x11"), true));
        assert!(!is_wayland_session(Some("x11"), false));
        assert!(!is_wayland_session(None, false));
    }

    #[test]
    fn control_leader_owns_active_layer_but_preserves_other_shortcuts() {
        let mut keys = HashMap::from([(29, true)]);
        assert!(shortcut_held(
            &keys,
            LogicalKey::ControlLeft,
            Layer::Initial
        ));
        assert!(!shortcut_held(&keys, LogicalKey::ControlLeft, Layer::Mouse));
        assert!(!shortcut_held(&keys, LogicalKey::ControlLeft, Layer::Grid));
        assert!(shortcut_held(&keys, LogicalKey::CapsLock, Layer::Grid));
        keys.insert(97, true);
        assert!(shortcut_held(&keys, LogicalKey::ControlLeft, Layer::Grid));
    }
}
