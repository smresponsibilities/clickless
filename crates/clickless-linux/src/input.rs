use super::LinuxHook;
use clickless_backend_api::OutputBackend;
use evdev::{EventType, InputEvent, Key, raw_stream::RawDevice, uinput::VirtualDeviceBuilder};
use std::{collections::HashMap, os::fd::AsRawFd, time::Instant};

const VIRTUAL_NAME: &str = "Clickless virtual keyboard";

fn is_wayland_session(session_type: Option<&str>, wayland_display: bool) -> bool {
    wayland_display || session_type.is_some_and(|session| session.eq_ignore_ascii_case("wayland"))
}

fn capabilities(device: &RawDevice) -> std::collections::HashSet<(u16, u16)> {
    let mut values = std::collections::HashSet::new();
    for kind in device.supported_events().iter() {
        values.insert((kind.0, u16::MAX));
    }
    if let Some(keys) = device.supported_keys() {
        for key in keys.iter() {
            values.insert((EventType::KEY.0, key.0));
        }
    }
    if let Some(items) = device.misc_properties() {
        for item in items.iter() {
            values.insert((EventType::MISC.0, item.0));
        }
    }
    if let Some(items) = device.supported_relative_axes() {
        for item in items.iter() {
            values.insert((EventType::RELATIVE.0, item.0));
        }
    }
    if let Some(items) = device.supported_switches() {
        for item in items.iter() {
            values.insert((EventType::SWITCH.0, item.0));
        }
    }
    values
}

fn discovery_due(last_scan: &mut Instant, now: Instant) -> bool {
    if now.duration_since(*last_scan).as_millis() < 1000 {
        return false;
    }
    *last_scan = now;
    true
}

fn prepare(devices: &[RawDevice]) -> Result<evdev::uinput::VirtualDevice, String> {
    let setup = || -> std::io::Result<_> {
        let mut keys = evdev::AttributeSet::<Key>::new();
        let mut misc = evdev::AttributeSet::<evdev::MiscType>::new();
        let mut axes = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
        let mut switches = evdev::AttributeSet::<evdev::SwitchType>::new();
        for dev in devices {
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
            for key in dev.supported_keys().expect("keyboard checked").iter() {
                keys.insert(key);
            }
            if let Some(values) = dev.misc_properties() {
                for value in values.iter() {
                    misc.insert(value);
                }
            }
            if let Some(values) = dev.supported_relative_axes() {
                for value in values.iter() {
                    axes.insert(value);
                }
            }
            if let Some(values) = dev.supported_switches() {
                for value in values.iter() {
                    switches.insert(value);
                }
            }
            let fd = dev.as_raw_fd();
            // SAFETY: fd belongs to a live device; fcntl does not retain or close it.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        let builder = VirtualDeviceBuilder::new()?
            .name(VIRTUAL_NAME)
            .input_id(evdev::InputId::new(evdev::BusType::BUS_VIRTUAL, 0, 0, 1))
            .with_keys(&keys)?
            .with_msc(&misc)?
            .with_relative_axes(&axes)?
            .with_switches(&switches)?;
        let mut output = builder.build()?;
        output.emit(&[])?;
        Ok(output)
    };
    setup().map_err(|error| format!("Keyboard forwarding setup failed before capture: {error}"))
}

// Apply ownership only when a whole physical frame can be forwarded.
fn forward_event<O: OutputBackend>(
    frame: &mut Vec<InputEvent>,
    state: &mut super::keyboard::KeyboardState,
    hook: &mut LinuxHook<O>,
    device: u64,
    event: InputEvent,
    now_ms: u64,
) -> Result<Option<Vec<InputEvent>>, String> {
    match (event.event_type(), event.code()) {
        (EventType::SYNCHRONIZATION, 3) => Err(
            "Keyboard events lost (SYN_DROPPED); capture stopped, restart with keys released"
                .into(),
        ),
        (EventType::SYNCHRONIZATION, 0) => {
            let mut output = Vec::new();
            for event in frame.drain(..) {
                if event.event_type() == EventType::KEY {
                    for (code, value) in
                        state.key(hook, device, event.code(), event.value(), now_ms)?
                    {
                        output.push(InputEvent::new(EventType::KEY, code, value));
                    }
                } else {
                    output.push(event);
                }
            }
            Ok(Some(output))
        }
        (EventType::LED | EventType::REPEAT, _) => Ok(None),
        _ => {
            if frame.len() >= 4096 {
                return Err("Keyboard frame exceeds 4096 events".into());
            }
            frame.push(event);
            Ok(None)
        }
    }
}

pub(super) fn run<O: OutputBackend>(
    mut hook: LinuxHook<O>,
    mut is_running: impl FnMut(&mut LinuxHook<O>) -> Result<bool, String>,
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

    let get_paths = || -> Result<Vec<std::path::PathBuf>, String> {
        let selected = std::env::var_os("CLICKLESS_INPUT_DEVICES");
        if let Some(value) = &selected {
            let paths: Vec<_> = std::env::split_paths(&value).collect();
            if paths.is_empty() || paths.iter().any(|path| !path.is_absolute()) {
                return Err("CLICKLESS_INPUT_DEVICES requires absolute device paths".into());
            }
            let mut unique = std::collections::HashSet::new();
            for path in &paths {
                let canonical = path
                    .canonicalize()
                    .map_err(|error| format!("Selected keyboard {}: {error}", path.display()))?;
                if !unique.insert(canonical) {
                    return Err("Selected keyboard paths refer to the same device".into());
                }
            }
            Ok(paths)
        } else {
            let entries = std::fs::read_dir("/dev/input")
                .map_err(|error| format!("Cannot enumerate /dev/input: {error}"))?;
            let mut paths = entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Cannot enumerate /dev/input: {error}"))?;
            paths.sort();
            Ok(paths)
        }
    };

    let try_open = |path: &std::path::Path, selected: bool| -> Result<Option<RawDevice>, String> {
        let dev = match RawDevice::open(path) {
            Ok(dev) => dev,
            Err(error) if selected => {
                return Err(format!("Selected keyboard {}: {error}", path.display()));
            }
            Err(_) => return Ok(None),
        };
        if dev.name() == Some(VIRTUAL_NAME)
            || !dev
                .supported_keys()
                .is_some_and(|keys| keys.contains(Key::KEY_CAPSLOCK) && keys.contains(Key::KEY_A))
        {
            if selected {
                return Err(format!(
                    "Selected device {} is not a supported physical keyboard",
                    path.display()
                ));
            }
            return Ok(None);
        }
        Ok(Some(dev))
    };

    let setup = || -> Result<_, String> {
        let paths = get_paths()?;
        let selected = std::env::var_os("CLICKLESS_INPUT_DEVICES").is_some();
        let mut devices = Vec::new();
        let mut active_paths = Vec::new();
        for path in paths {
            if let Some(dev) = try_open(&path, selected)? {
                if dev
                    .get_key_state()
                    .map_err(|error| error.to_string())?
                    .iter()
                    .next()
                    .is_some()
                {
                    return Err("Release all keyboard keys before starting Clickless".into());
                }
                devices.push(dev);
                active_paths.push(path);
            }
        }
        if devices.is_empty() {
            return Err("No accessible physical keyboard found in /dev/input".into());
        }
        let output = prepare(&devices)?;
        Ok((devices, active_paths, output))
    };
    let (devices, mut active_paths, mut output) = match setup() {
        Ok(t) => t,
        Err(error) => return hook.finish_input(Err(error), || Ok(())),
    };

    let allowed_capabilities: std::collections::HashSet<_> =
        devices.iter().flat_map(capabilities).collect();
    let mut grabbed: Vec<RawDevice> = Vec::new();

    for mut dev in devices {
        if let Err(error) = dev.grab() {
            let mut errors = vec![format!("Keyboard grab failed: {error}")];
            for mut g in grabbed {
                if let Err(error) = g.ungrab() {
                    errors.push(format!("Keyboard rollback ungrab failed: {error}"));
                }
            }
            return hook.finish_input(Err(errors.join("; ")), || Ok(()));
        }
        grabbed.push(dev);
    }
    let mut devices = grabbed;

    let mut state = super::keyboard::KeyboardState::default();
    let mut frames_by_device: HashMap<std::os::fd::RawFd, Vec<InputEvent>> = HashMap::new();
    let start = Instant::now();
    let mut last_tick = start;
    let mut last_scan = start;
    let result = (|| -> Result<(), String> {
        for dev in &mut devices {
            if dev
                .get_key_state()
                .map_err(|error| error.to_string())?
                .iter()
                .next()
                .is_some()
            {
                return Err(
                    "Keyboard changed during capture setup; restart with keys released".into(),
                );
            }
            frames_by_device.insert(dev.as_raw_fd(), Vec::new());
        }
        while is_running(&mut hook)? {
            let fds: Vec<_> = devices.iter().map(AsRawFd::as_raw_fd).collect();
            let readiness = crate::poll::poll_devices(&fds, 5)
                .map_err(|error| format!("Device poll error: {error}"))?;
            let mut removed = readiness.removed;
            let now = Instant::now();
            if discovery_due(&mut last_scan, now) {
                let paths = get_paths()?;
                let selected = std::env::var_os("CLICKLESS_INPUT_DEVICES").is_some();
                for path in paths {
                    if active_paths.contains(&path) {
                        continue;
                    }
                    let Some(mut dev) = try_open(&path, selected)? else {
                        continue;
                    };
                    if !capabilities(&dev).is_subset(&allowed_capabilities) {
                        return Err(format!(
                            "Replacement keyboard {} needs different forwarding capabilities; restart Clickless",
                            path.display()
                        ));
                    }
                    if dev
                        .get_key_state()
                        .map_err(|error| error.to_string())?
                        .iter()
                        .next()
                        .is_some()
                    {
                        continue;
                    }
                    let fd = dev.as_raw_fd();
                    // SAFETY: fd belongs to the live replacement device.
                    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                    if flags < 0
                        || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                    {
                        return Err(format!(
                            "Replacement keyboard {} nonblocking setup failed: {}",
                            path.display(),
                            std::io::Error::last_os_error()
                        ));
                    }
                    dev.grab().map_err(|error| {
                        format!(
                            "Replacement keyboard {} grab failed: {error}",
                            path.display()
                        )
                    })?;
                    // Register first so every subsequent failure rolls capture back.
                    devices.push(dev);
                    active_paths.push(path);
                    frames_by_device.insert(fd, Vec::new());
                    if devices
                        .last_mut()
                        .unwrap()
                        .get_key_state()
                        .map_err(|error| error.to_string())?
                        .iter()
                        .next()
                        .is_some()
                    {
                        return Err("Keyboard changed during replacement capture; restart with keys released".into());
                    }
                }
            }

            for dev in devices
                .iter_mut()
                .filter(|d| readiness.ready.contains(&d.as_raw_fd()))
            {
                let fd = dev.as_raw_fd();
                let fetched = match dev.fetch_events() {
                    Ok(events) => Ok(events.collect::<Vec<_>>()),
                    Err(error) => Err(error),
                };
                match fetched {
                    Ok(events) => {
                        for event in events {
                            let now_ms = start.elapsed().as_millis() as u64;
                            let frame = frames_by_device.get_mut(&fd).unwrap();
                            if event.event_type() == EventType::SYNCHRONIZATION && event.code() == 3
                            {
                                frame.clear();
                                let current_state = dev.get_key_state().map_err(|error| {
                                    format!("Query key state failed on SYN_DROPPED: {error}")
                                })?;
                                let held_keys: std::collections::HashSet<u16> =
                                    current_state.iter().map(|k| k.0).collect();
                                let releases = state
                                    .resync_device(&mut hook, fd as u64, &held_keys, now_ms)?;
                                let releases: Vec<_> = releases
                                    .into_iter()
                                    .map(|(code, value)| {
                                        InputEvent::new(EventType::KEY, code, value)
                                    })
                                    .collect();
                                if !releases.is_empty() {
                                    output.emit(&releases).map_err(|error| {
                                        format!(
                                            "Keyboard forwarding failed on SYN_DROPPED: {error}"
                                        )
                                    })?;
                                    for event in &releases {
                                        state.acknowledge_forwarded(event.code(), event.value());
                                    }
                                }
                                continue;
                            }
                            match forward_event(
                                frame, &mut state, &mut hook, fd as u64, event, now_ms,
                            ) {
                                Ok(Some(committed)) => {
                                    output.emit(&committed).map_err(|error| {
                                        format!("Keyboard forwarding failed: {error}")
                                    })?;
                                    for event in committed
                                        .iter()
                                        .filter(|event| event.event_type() == EventType::KEY)
                                    {
                                        state.acknowledge_forwarded(event.code(), event.value());
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) if error.raw_os_error() == Some(libc::ENODEV) => removed.push(fd),
                    Err(error) => return Err(format!("Device read error: {error}")),
                }
            }
            removed.sort_unstable();
            removed.dedup();
            for fd in removed {
                let releases =
                    state.remove(&mut hook, fd as u64, start.elapsed().as_millis() as u64)?;
                let releases: Vec<_> = releases
                    .into_iter()
                    .map(|(code, value)| InputEvent::new(EventType::KEY, code, value))
                    .collect();
                output
                    .emit(&releases)
                    .map_err(|error| format!("Removed keyboard release failed: {error}"))?;
                for event in &releases {
                    state.acknowledge_forwarded(event.code(), event.value());
                }
                if let Some(index) = devices.iter().position(|dev| dev.as_raw_fd() == fd) {
                    let mut dev = devices.remove(index);
                    active_paths.remove(index);
                    if let Err(error) = dev.ungrab()
                        && error.raw_os_error() != Some(libc::ENODEV)
                    {
                        return Err(format!("Removed keyboard ungrab failed: {error}"));
                    }
                }
                frames_by_device.remove(&fd);
                // Device loss cancels latched pointer output even without a held key.
                hook.release_capture()?;
            }
            let now = Instant::now();
            let dt = now.duration_since(last_tick).as_millis() as u64;
            if dt >= 10 {
                hook.sm_mut().poll(start.elapsed().as_millis() as u64);
                hook.sync_overlay()?;
                hook.tick(dt)?;
                last_tick = now;
            }
        }
        Ok(())
    })();
    hook.finish_input(result, || {
        let releases: Vec<_> = state
            .releases()
            .into_iter()
            .map(|(code, value)| InputEvent::new(EventType::KEY, code, value))
            .collect();
        let release = output
            .emit(&releases)
            .map_err(|error| format!("Virtual key release failed: {error}"));
        let mut ungrab_errs = Vec::new();
        for mut dev in devices {
            if let Err(error) = dev.ungrab() {
                ungrab_errs.push(format!("Keyboard ungrab failed: {error}"));
            }
        }
        match (release, ungrab_errs.is_empty()) {
            (Err(a), false) => Err(format!("{a}; {}", ungrab_errs.join(", "))),
            (Err(error), _) => Err(error),
            (_, false) => Err(ungrab_errs.join(", ")),
            _ => Ok(()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Output;
    impl OutputBackend for Output {
        fn move_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn button(
            &mut self,
            _: clickless_backend_api::Button,
            _: clickless_backend_api::Dir,
        ) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn discovery_deadline_survives_regular_runtime_ticks() {
        let start = Instant::now();
        let mut scan = start;
        for milliseconds in (10..1000).step_by(10) {
            assert!(!discovery_due(
                &mut scan,
                start + std::time::Duration::from_millis(milliseconds)
            ));
        }
        assert!(discovery_due(
            &mut scan,
            start + std::time::Duration::from_millis(1000)
        ));
        assert!(!discovery_due(
            &mut scan,
            start + std::time::Duration::from_millis(1010)
        ));
    }

    #[test]
    fn lost_events_stop_before_replaying_unknown_key_history() {
        let mut hook = LinuxHook::new(Output);
        let mut state = super::super::keyboard::KeyboardState::default();
        let mut frame = vec![InputEvent::new(EventType::KEY, 30, 1)];
        let error = forward_event(
            &mut frame,
            &mut state,
            &mut hook,
            1,
            InputEvent::new(EventType::SYNCHRONIZATION, 3, 0),
            0,
        )
        .unwrap_err();
        assert!(error.contains("restart with keys released"));
        assert!(state.releases().is_empty());
    }

    #[test]
    fn incomplete_frames_cannot_release_another_devices_committed_press() {
        let mut hook = LinuxHook::new(Output);
        let mut state = super::super::keyboard::KeyboardState::default();
        let mut a = Vec::new();
        let mut b = Vec::new();
        let key = |value| InputEvent::new(EventType::KEY, 30, value);
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        assert!(
            forward_event(&mut a, &mut state, &mut hook, 1, key(1), 0)
                .unwrap()
                .is_none()
        );
        assert!(
            forward_event(&mut a, &mut state, &mut hook, 1, key(0), 1)
                .unwrap()
                .is_none()
        );
        assert!(state.releases().is_empty());
        forward_event(&mut b, &mut state, &mut hook, 2, key(1), 2).unwrap();
        let output = forward_event(&mut b, &mut state, &mut hook, 2, syn, 3)
            .unwrap()
            .unwrap();
        assert_eq!(
            output
                .iter()
                .map(|e| (e.code(), e.value()))
                .collect::<Vec<_>>(),
            vec![(30, 1)]
        );
        assert!(
            forward_event(&mut a, &mut state, &mut hook, 1, syn, 4)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        forward_event(&mut b, &mut state, &mut hook, 2, key(0), 5).unwrap();
        let output = forward_event(&mut b, &mut state, &mut hook, 2, syn, 6)
            .unwrap()
            .unwrap();
        assert_eq!(
            output
                .iter()
                .map(|e| (e.code(), e.value()))
                .collect::<Vec<_>>(),
            vec![(30, 0)]
        );
    }

    #[test]
    fn wayland_session_is_rejected_even_with_xwayland_available() {
        assert!(is_wayland_session(Some("wayland"), false));
        assert!(is_wayland_session(Some("x11"), true));
        assert!(!is_wayland_session(Some("x11"), false));
        assert!(!is_wayland_session(None, false));
    }
}
