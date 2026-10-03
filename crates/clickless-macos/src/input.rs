use super::MacosHook;
use clickless_backend_api::OutputBackend;
use clickless_core::{Layer, LogicalKey, Outcome};
use core_foundation::{
    base::TCFType,
    mach_port::{CFMachPort, CFMachPortInvalidate, CFMachPortRef},
    runloop::{CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes, kCFRunLoopDefaultMode},
};
use core_graphics::{
    event::{CGEventFlags, CGEventType, EventField},
    sys::CGEventRef,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr::null_mut,
    time::Instant,
};

// core-graphics 0.24's callback wrapper returns the original event for None.
// Use the public C API so an engine-owned event can return a null pointer.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        location: u32,
        placement: u32,
        options: u32,
        mask: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
}

struct Tap {
    port: CFMachPort,
    source: Option<CFRunLoopSource>,
    runloop: CFRunLoop,
}

impl Drop for Tap {
    fn drop(&mut self) {
        // SAFETY: port/source remain owned and teardown runs on their run-loop thread.
        unsafe {
            CGEventTapEnable(self.port.as_concrete_TypeRef(), false);
            if let Some(source) = &self.source {
                self.runloop.remove_source(source, kCFRunLoopCommonModes);
            }
            CFMachPortInvalidate(self.port.as_concrete_TypeRef());
        }
    }
}

struct State<O: OutputBackend> {
    hook: MacosHook<O>,
    start: Instant,
    // True denotes a press forwarded to the application, false engine ownership.
    keys: HashMap<u16, bool>,
    modifiers: HashMap<u16, bool>,
    error: Option<String>,
}

struct Callback<O: OutputBackend> {
    state: RefCell<State<O>>,
    reentered: Cell<bool>,
}

fn modifier_flag(code: u16) -> Option<CGEventFlags> {
    match code {
        0x38 | 0x3c => Some(CGEventFlags::CGEventFlagShift),
        0x3b | 0x3e => Some(CGEventFlags::CGEventFlagControl),
        0x3a | 0x3d => Some(CGEventFlags::CGEventFlagAlternate),
        0x37 | 0x36 => Some(CGEventFlags::CGEventFlagCommand),
        0x3f => Some(CGEventFlags::CGEventFlagSecondaryFn),
        _ => None,
    }
}

impl<O: OutputBackend> State<O> {
    fn event(&mut self, kind: u32, code: u16, flags: CGEventFlags, repeat: bool) -> bool {
        if kind == CGEventType::FlagsChanged as u32 && code == 0x39 {
            // AlphaShift reports lock state, not a physical key's held interval.
            return true;
        }
        let down = if kind == CGEventType::FlagsChanged as u32 {
            let Some(flag) = modifier_flag(code) else {
                return true;
            };
            // When the other side remains held, the aggregate flag stays set.
            // This key's previous state distinguishes its press from release.
            let previous = self.modifiers.get(&code).copied().unwrap_or(false);
            let down = flags.contains(flag) && !previous;
            self.modifiers.insert(code, down);
            down
        } else if kind == CGEventType::KeyDown as u32 {
            true
        } else if kind == CGEventType::KeyUp as u32 {
            false
        } else {
            return true;
        };
        let existing = self.keys.get(&code).copied();
        if repeat || (down && existing.is_some()) {
            return existing.unwrap_or(true);
        }
        let is_modifier = modifier_flag(code).is_some();
        let other_side_held = is_modifier
            && self.modifiers.iter().any(|(other, held)| {
                *held && *other != code && modifier_flag(*other) == modifier_flag(code)
            });
        let is_leader = super::key_code::cg_to_logical(code) == Some(self.hook.sm().leader_key());
        let leader_owns_control = self.hook.sm().leader_key() == LogicalKey::ControlLeft
            && self.hook.sm().layer() != Layer::Initial
            && !self.modifiers.get(&0x3e).copied().unwrap_or(false);
        let chord = flags
            .intersects(CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagAlternate)
            || (flags.contains(CGEventFlags::CGEventFlagControl) && !leader_owns_control);
        let pass = self.error.is_some()
            || (down && other_side_held)
            || (existing == Some(true) && !is_modifier && !is_leader)
            || (down && chord && code != 0x3b)
            || (down
                && code == 0x3b
                && flags.intersects(
                    CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagAlternate,
                ))
            || (code == 0x3b && !is_leader && self.hook.sm().layer() == Layer::Grid);
        let outcome = if pass {
            if down {
                self.hook.sm_mut().interrupt_pending_taps();
            }
            Outcome::PASS
        } else {
            let (outcome, result) =
                self.hook
                    .process_key_outcome(code, down, self.start.elapsed().as_millis() as u64);
            if let Err(error) = result {
                self.error = Some(error);
            }
            outcome
        };
        // Modifier events always retain OS ownership, including grid shortcuts.
        if down {
            let forward = is_modifier || !outcome.consumed;
            self.keys.insert(code, forward);
            forward
        } else {
            self.keys.remove(&code).unwrap_or(true)
        }
    }
}

unsafe extern "C" fn callback<O: OutputBackend>(
    _proxy: *mut c_void,
    kind: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    // SAFETY: user_info points to Box<Callback<O>>, retained until Tap is invalidated.
    let context = unsafe { &*user_info.cast::<Callback<O>>() };
    let Ok(mut state) = context.state.try_borrow_mut() else {
        context.reentered.set(true);
        return event;
    };
    if kind == CGEventType::TapDisabledByTimeout as u32
        || kind == CGEventType::TapDisabledByUserInput as u32
    {
        state.error = Some("macOS disabled keyboard capture; restart after checking permissions and input restrictions".into());
        return event;
    }
    if event.is_null() {
        return event;
    }
    // SAFETY: ordinary callbacks supply a valid borrowed CGEvent for this call.
    let pid =
        unsafe { CGEventGetIntegerValueField(event, EventField::EVENT_SOURCE_UNIX_PROCESS_ID) };
    if pid == i64::from(std::process::id()) {
        return event;
    }
    let processed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: all access is read-only and event remains live throughout callback.
        let code =
            unsafe { CGEventGetIntegerValueField(event, EventField::KEYBOARD_EVENT_KEYCODE) };
        let code = u16::try_from(code).map_err(|_| "Invalid macOS keyboard code".to_string())?;
        let flags = CGEventFlags::from_bits_truncate(unsafe { CGEventGetFlags(event) });
        let repeat = kind == CGEventType::KeyDown as u32
            && unsafe { CGEventGetIntegerValueField(event, EventField::KEYBOARD_EVENT_AUTOREPEAT) }
                != 0;
        Ok::<_, String>(state.event(kind, code, flags, repeat))
    }));
    match processed {
        Ok(Ok(false)) => null_mut(),
        Ok(Ok(true)) => event,
        Ok(Err(error)) => {
            state.error = Some(error);
            event
        }
        Err(_) => {
            state.error = Some("Keyboard callback panicked; capture stopped".into());
            event
        }
    }
}

pub(super) fn run<O: OutputBackend>(
    mut hook: MacosHook<O>,
    mut running: impl FnMut() -> bool,
) -> Result<(), String> {
    if !super::permissions::check_accessibility_permissions(true) {
        let error = "macOS Accessibility permission is required for Clickless to intercept keyboard shortcuts. Please grant it in System Settings -> Privacy & Security -> Accessibility and restart the application.";
        return match hook.release_capture() {
            Ok(()) => Err(error.into()),
            Err(cleanup) => Err(format!("{error}; {cleanup}")),
        };
    }
    if super::permissions::is_secure_input_enabled() {
        let error =
            "macOS Secure Input is enabled; leave the protected field and restart Clickless";
        return match hook.release_capture() {
            Ok(()) => Err(error.into()),
            Err(cleanup) => Err(format!("{error}; {cleanup}")),
        };
    }

    if hook.sm().leader_key() == LogicalKey::CapsLock {
        let error = "CapsLock is a toggle on macOS, not a verified hold leader. Set leader to ShiftLeft or ControlLeft before starting Clickless";
        return match hook.release_capture() {
            Ok(()) => Err(error.into()),
            Err(cleanup) => Err(format!("{error}; {cleanup}")),
        };
    }
    let mut modifiers = HashMap::new();
    for code in [0x38, 0x3c, 0x3b, 0x3e, 0x3a, 0x3d, 0x37, 0x36, 0x3f] {
        // SAFETY: public API reads HID modifier state without retaining pointers.
        modifiers.insert(code, unsafe { CGEventSourceKeyState(1, code) });
    }
    let state = Box::new(Callback {
        state: RefCell::new(State {
            hook,
            start: Instant::now(),
            keys: HashMap::new(),
            modifiers,
            error: None,
        }),
        reentered: Cell::new(false),
    });
    let result = (|| -> Result<(), String> {
        let mask = (1 << CGEventType::KeyDown as u32)
            | (1 << CGEventType::KeyUp as u32)
            | (1 << CGEventType::FlagsChanged as u32);
        // SAFETY: callback's concrete type matches state; Box outlives Tap on every path.
        let port = unsafe {
            CGEventTapCreate(
                0,
                0,
                0,
                mask,
                callback::<O>,
                (&*state as *const Callback<O>).cast_mut().cast(),
            )
        };
        if port.is_null() {
            return Err("Failed to create keyboard event tap; check Accessibility/Input Monitoring permissions".into());
        }
        // SAFETY: non-null port was returned under the Core Foundation create rule.
        let port = unsafe { CFMachPort::wrap_under_create_rule(port) };
        let mut tap = Tap {
            port,
            source: None,
            runloop: CFRunLoop::get_current(),
        };
        tap.source = Some(
            tap.port
                .create_runloop_source(0)
                .map_err(|()| "Failed to create keyboard run-loop source".to_string())?,
        );
        // SAFETY: source and port are owned by tap and runloop is current thread's.
        unsafe {
            tap.runloop.add_source(
                tap.source.as_ref().expect("source created"),
                kCFRunLoopCommonModes,
            );
            CGEventTapEnable(tap.port.as_concrete_TypeRef(), true);
        }
        let mut last_tick = Instant::now();
        while running() {
            if super::permissions::is_secure_input_enabled() {
                return Err("macOS Secure Input enabled; capture stopped. Leave the protected field and restart Clickless".into());
            }
            if !super::permissions::check_accessibility_permissions(false) {
                return Err("macOS Accessibility permission revoked; capture stopped".into());
            }
            // SAFETY: no state borrow is held while callbacks are dispatched.
            unsafe {
                CFRunLoop::run_in_mode(
                    kCFRunLoopDefaultMode,
                    std::time::Duration::from_millis(5),
                    true,
                );
            }
            if state.reentered.get() {
                return Err("Reentrant keyboard callback; capture stopped".into());
            }
            let mut state = state.state.borrow_mut();
            if let Some(error) = state.error.take() {
                return Err(error);
            }
            let now = Instant::now();
            let dt = now.duration_since(last_tick).as_millis() as u64;
            if dt >= 10 {
                let now_ms = state.start.elapsed().as_millis() as u64;
                state.hook.sm_mut().poll(now_ms);
                state.hook.sync_overlay()?;
                state.hook.tick(dt)?;
                last_tick = now;
            }
        }
        Ok(())
    })(); // Tap invalidated before cleanup and before callback state can be freed.
    let cleanup = state.state.borrow_mut().hook.release_capture();
    match (result, cleanup) {
        (Err(a), Err(b)) => Err(format!("{a}; {b}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_backend_api::{Button, Dir};

    struct Output;
    impl OutputBackend for Output {
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
    fn control_leader_consumes_movement_but_command_chords_pass() {
        let mut hook = MacosHook::with_config(
            Output,
            LogicalKey::ControlLeft,
            clickless_core::default_bindings(),
            Default::default(),
        );
        hook.process_key(0x3b, true, 0).unwrap();
        hook.sm_mut().poll(200);
        assert_eq!(hook.sm().layer(), Layer::Mouse);
        let mut state = State {
            hook,
            start: Instant::now(),
            keys: HashMap::new(),
            modifiers: HashMap::from([(0x3b, true)]),
            error: None,
        };
        assert!(!state.event(
            CGEventType::KeyDown as u32,
            0x04,
            CGEventFlags::CGEventFlagControl,
            false
        ));
        assert!(state.event(
            CGEventType::KeyDown as u32,
            0x26,
            CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagCommand,
            false
        ));
        state.modifiers.insert(0x3e, true);
        assert!(state.event(
            CGEventType::KeyDown as u32,
            0x28,
            CGEventFlags::CGEventFlagControl,
            false
        ));
    }
}
