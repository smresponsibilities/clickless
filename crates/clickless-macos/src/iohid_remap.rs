//! Scoped Caps Lock remapping. Preserve mappings owned by the user or other tools.

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, CFTypeRef, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    runloop::{CFRunLoop, kCFRunLoopCommonModes},
    string::{CFString, CFStringRef},
};
use std::{
    cell::RefCell,
    ffi::c_void,
    ptr::null_mut,
    time::{Duration, Instant},
};

const CAPS_LOCK: i64 = 0x7_0000_0039;
const F18: i64 = 0x7_0000_006d;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDEventSystemClientCreateSimpleClient(alloc: *mut c_void) -> CFTypeRef;
    fn IOHIDEventSystemClientCopyServices(client: CFTypeRef) -> CFArrayRef;
    fn IOHIDServiceClientConformsTo(service: CFTypeRef, page: u32, usage: u32) -> bool;
    fn IOHIDServiceClientCopyProperty(service: CFTypeRef, key: CFStringRef) -> CFTypeRef;
    fn IOHIDServiceClientSetProperty(
        service: CFTypeRef,
        key: CFStringRef,
        value: CFTypeRef,
    ) -> bool;
    fn IOHIDManagerCreate(alloc: *mut c_void, options: u32) -> CFTypeRef;
    fn IOHIDManagerSetDeviceMatching(manager: CFTypeRef, matching: CFTypeRef);
    fn IOHIDManagerRegisterDeviceMatchingCallback(
        manager: CFTypeRef,
        callback: unsafe extern "C" fn(*mut c_void, i32, *mut c_void, *mut c_void),
        context: *mut c_void,
    );
    fn IOHIDManagerScheduleWithRunLoop(manager: CFTypeRef, runloop: *mut c_void, mode: CFStringRef);
    fn IOHIDManagerUnscheduleFromRunLoop(
        manager: CFTypeRef,
        runloop: *mut c_void,
        mode: CFStringRef,
    );
    fn IOHIDManagerOpen(manager: CFTypeRef, options: u32) -> i32;
    fn IOHIDManagerClose(manager: CFTypeRef, options: u32) -> i32;
}

fn property(service: &CFType) -> Option<CFType> {
    // SAFETY: service is retained; CopyProperty returns an owned CF object or null.
    let value = unsafe {
        IOHIDServiceClientCopyProperty(
            service.as_CFTypeRef(),
            CFString::new("UserKeyMapping").as_concrete_TypeRef(),
        )
    };
    (!value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

fn set_property(service: &CFType, value: &CFType) -> Result<(), String> {
    // SAFETY: all CF objects remain retained for this call.
    if unsafe {
        IOHIDServiceClientSetProperty(
            service.as_CFTypeRef(),
            CFString::new("UserKeyMapping").as_concrete_TypeRef(),
            value.as_CFTypeRef(),
        )
    } {
        Ok(())
    } else {
        Err("Could not update keyboard UserKeyMapping; capture stopped".into())
    }
}

fn mapping(source: i64, destination: i64) -> CFType {
    CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("HIDKeyboardModifierMappingSrc"),
            CFNumber::from(source),
        ),
        (
            CFString::new("HIDKeyboardModifierMappingDst"),
            CFNumber::from(destination),
        ),
    ])
    .as_CFType()
}

fn merged_mapping(previous: Option<&CFType>) -> Result<CFType, String> {
    let mut entries = Vec::new();
    if let Some(previous) = previous {
        let array = previous
            .downcast::<CFArray>()
            .ok_or("Keyboard UserKeyMapping is not an array")?;
        for entry in array.iter() {
            // SAFETY: UserKeyMapping is a CF property array. Validate each object's type.
            let entry = unsafe { CFType::wrap_under_get_rule(*entry) };
            let dict = entry
                .downcast::<CFDictionary>()
                .ok_or("Invalid keyboard mapping entry")?;
            let key = CFString::new("HIDKeyboardModifierMappingSrc");
            let source = dict
                .find(key.as_CFTypeRef())
                .ok_or("Keyboard mapping has no source")?;
            let source = unsafe { CFType::wrap_under_get_rule(*source) };
            let source = source
                .downcast::<CFNumber>()
                .and_then(|n| n.to_i64())
                .ok_or("Invalid keyboard mapping source")?;
            if source != CAPS_LOCK {
                entries.push(entry);
            }
        }
    }
    entries.push(mapping(CAPS_LOCK, F18));
    Ok(CFArray::from_CFTypes(&entries).as_CFType())
}

struct SavedMapping {
    service: CFType,
    previous: Option<CFType>,
    installed: CFType,
}

#[derive(Default)]
struct Remaps {
    saved: Vec<SavedMapping>,
    error: Option<String>,
}

impl Remaps {
    fn refresh(&mut self) -> Result<(), String> {
        // SAFETY: Create/Copy objects are checked for null and retained under create rule.
        let client = unsafe { IOHIDEventSystemClientCreateSimpleClient(null_mut()) };
        if client.is_null() {
            return Err("Could not create keyboard remapping client".into());
        }
        let client = unsafe { CFType::wrap_under_create_rule(client) };
        let services = unsafe { IOHIDEventSystemClientCopyServices(client.as_CFTypeRef()) };
        if services.is_null() {
            return Err("Could not enumerate keyboard remapping services".into());
        }
        let services = unsafe { CFArray::<CFTypeRef>::wrap_under_create_rule(services) };
        // Drop detached services without restoring a different device that reused an ID.
        self.saved
            .retain(|saved| services.iter().any(|s| *s == saved.service.as_CFTypeRef()));
        for service in services.iter() {
            if !unsafe { IOHIDServiceClientConformsTo(*service, 1, 6) } {
                continue;
            }
            let service = unsafe { CFType::wrap_under_get_rule(*service) };
            let current = property(&service);
            if let Some(saved) = self
                .saved
                .iter()
                .find(|s| s.service.as_CFTypeRef() == service.as_CFTypeRef())
            {
                if current.as_ref() == Some(&saved.installed) {
                    continue;
                }
                // Wake can reset mappings. Never overwrite another tool's newer edits.
                if current != saved.previous {
                    return Err(
                        "Keyboard mapping changed outside Clickless; capture stopped".into(),
                    );
                }
                set_property(&service, &saved.installed)?;
            } else {
                let installed = merged_mapping(current.as_ref())?;
                set_property(&service, &installed)?;
                self.saved.push(SavedMapping {
                    service,
                    previous: current,
                    installed,
                });
            }
        }
        Ok(())
    }
}

fn restored_mapping(current: &CFType, previous: Option<&CFType>) -> Result<Option<CFType>, String> {
    let current = current
        .downcast::<CFArray>()
        .ok_or("Keyboard UserKeyMapping is not an array")?;
    let owned = mapping(CAPS_LOCK, F18);
    let mut entries = Vec::new();
    let mut found = false;
    for entry in current.iter() {
        let entry = unsafe { CFType::wrap_under_get_rule(*entry) };
        if entry == owned {
            found = true;
        } else {
            entries.push(entry);
        }
    }
    if !found {
        return Ok(None);
    }
    if let Some(previous) = previous {
        let previous = previous
            .downcast::<CFArray>()
            .ok_or("Invalid saved keyboard mapping")?;
        for entry in previous.iter() {
            let entry = unsafe { CFType::wrap_under_get_rule(*entry) };
            let dict = entry
                .downcast::<CFDictionary>()
                .ok_or("Invalid saved mapping entry")?;
            let key = CFString::new("HIDKeyboardModifierMappingSrc");
            if let Some(source) = dict.find(key.as_CFTypeRef()) {
                let source = unsafe { CFType::wrap_under_get_rule(*source) };
                if source.downcast::<CFNumber>().and_then(|n| n.to_i64()) == Some(CAPS_LOCK) {
                    entries.push(entry);
                }
            }
        }
    }
    Ok(Some(CFArray::from_CFTypes(&entries).as_CFType()))
}

impl Remaps {
    fn restore(&mut self) -> Result<(), String> {
        restore_mappings(&self.saved, property, set_property)
    }
}

fn restore_mappings(
    saved: &[SavedMapping],
    mut read: impl FnMut(&CFType) -> Option<CFType>,
    mut write: impl FnMut(&CFType, &CFType) -> Result<(), String>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for saved in saved {
        // Preserve later unrelated edits, removing only our Caps Lock entry.
        let Some(current) = read(&saved.service) else {
            continue;
        };
        let result =
            restored_mapping(&current, saved.previous.as_ref()).and_then(
                |restored| match restored {
                    Some(restored) => write(&saved.service, &restored),
                    None => Ok(()),
                },
            );
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Keyboard mapping restore failed: {}",
            errors.join("; ")
        ))
    }
}

impl Drop for Remaps {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("{error}; restore Caps Lock in keyboard settings");
        }
    }
}

unsafe extern "C" fn keyboard_connected(
    context: *mut c_void,
    result: i32,
    _: *mut c_void,
    _: *mut c_void,
) {
    if result != 0 {
        return;
    }
    // SAFETY: boxed state outlives the manager; callbacks run on its runloop thread.
    let state = unsafe { &*context.cast::<RefCell<Remaps>>() };
    let Ok(mut state) = state.try_borrow_mut() else {
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.refresh()));
    let error = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error,
        Err(_) => "Keyboard remapping callback panicked; capture stopped".into(),
    };
    state.error = Some(error);
}

pub struct HidRemapGuard {
    manager: CFType,
    runloop: CFRunLoop,
    state: Box<RefCell<Remaps>>,
    last_refresh: Instant,
    listening: bool,
}

impl HidRemapGuard {
    pub fn install() -> Result<Self, String> {
        let state = Box::new(RefCell::new(Remaps::default()));
        state.borrow_mut().refresh()?;
        let manager = unsafe { IOHIDManagerCreate(null_mut(), 0) };
        if manager.is_null() {
            return Err("Could not create keyboard hotplug manager".into());
        }
        let manager = unsafe { CFType::wrap_under_create_rule(manager) };
        let runloop = CFRunLoop::get_current();
        let mut guard = Self {
            manager,
            runloop,
            state,
            last_refresh: Instant::now(),
            listening: true,
        };
        let matching = CFDictionary::from_CFType_pairs(&[
            (CFString::new("DeviceUsagePage"), CFNumber::from(1_i64)),
            (CFString::new("DeviceUsage"), CFNumber::from(6_i64)),
        ]);
        // SAFETY: manager/state are owned by guard; teardown unschedules before state drops.
        unsafe {
            IOHIDManagerSetDeviceMatching(guard.manager.as_CFTypeRef(), matching.as_CFTypeRef());
            IOHIDManagerRegisterDeviceMatchingCallback(
                guard.manager.as_CFTypeRef(),
                keyboard_connected,
                (&mut *guard.state as *mut RefCell<Remaps>).cast(),
            );
            IOHIDManagerScheduleWithRunLoop(
                guard.manager.as_CFTypeRef(),
                guard.runloop.as_concrete_TypeRef().cast(),
                kCFRunLoopCommonModes,
            );
            if IOHIDManagerOpen(guard.manager.as_CFTypeRef(), 0) != 0 {
                return Err("Could not open keyboard hotplug manager".into());
            }
        }
        Ok(guard)
    }

    pub fn finish(&mut self) -> Result<(), String> {
        let close = if self.listening {
            self.listening = false;
            unsafe {
                IOHIDManagerUnscheduleFromRunLoop(
                    self.manager.as_CFTypeRef(),
                    self.runloop.as_concrete_TypeRef().cast(),
                    kCFRunLoopCommonModes,
                );
                IOHIDManagerClose(self.manager.as_CFTypeRef(), 0)
            }
        } else {
            0
        };
        let restore = self.state.borrow_mut().restore();
        match (close, restore) {
            (0, result) => result,
            (_, Ok(())) => Err("Could not close keyboard hotplug manager".into()),
            (_, Err(error)) => Err(format!("Could not close keyboard hotplug manager; {error}")),
        }
    }

    pub fn check(&mut self) -> Result<(), String> {
        if let Some(error) = self.state.borrow_mut().error.take() {
            return Err(error);
        }
        // Existing input loop resumes after wake. Reconcile driver resets there.
        if self.last_refresh.elapsed() >= Duration::from_secs(1) {
            self.state.borrow_mut().refresh()?;
            self.last_refresh = Instant::now();
        }
        Ok(())
    }
}

impl Drop for HidRemapGuard {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("{error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_restore_write_is_reported_and_other_keyboards_are_attempted() {
        let installed = merged_mapping(None).unwrap();
        let mut saved = Vec::new();
        for name in ["first", "second"] {
            saved.push(SavedMapping {
                service: CFString::new(name).as_CFType(),
                previous: None,
                installed: installed.clone(),
            });
        }
        let mut attempts = 0;
        let result = restore_mappings(
            &saved,
            |_| Some(installed.clone()),
            |_, _| {
                attempts += 1;
                Err("injected write failure".into())
            },
        );
        assert!(result.unwrap_err().contains("injected write failure"));
        assert_eq!(attempts, 2);
        // SavedMapping drops only CF references; fake services never enter native I/O.
    }

    #[test]
    fn restore_preserves_later_remaps_and_respects_new_caps_owner() {
        let previous_caps = mapping(CAPS_LOCK, 0x7_0000_0029);
        let previous = CFArray::from_CFTypes(std::slice::from_ref(&previous_caps)).as_CFType();
        let later = mapping(0x7_0000_00e2, 0x7_0000_00e3);
        let current = CFArray::from_CFTypes(&[mapping(CAPS_LOCK, F18), later.clone()]).as_CFType();
        assert_eq!(
            restored_mapping(&current, Some(&previous))
                .unwrap()
                .unwrap(),
            CFArray::from_CFTypes(&[later.clone(), previous_caps]).as_CFType()
        );
        let changed_caps =
            CFArray::from_CFTypes(&[mapping(CAPS_LOCK, 0x7_0000_002a), later]).as_CFType();
        assert!(
            restored_mapping(&changed_caps, Some(&previous))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            restored_mapping(&current, None).unwrap().unwrap(),
            CFArray::from_CFTypes(&[mapping(0x7_0000_00e2, 0x7_0000_00e3)]).as_CFType()
        );
    }

    #[test]
    fn caps_remap_preserves_unrelated_mapping_and_original_snapshot() {
        let other = mapping(0x7_0000_00e2, 0x7_0000_00e3);
        let original =
            CFArray::from_CFTypes(&[other.clone(), mapping(CAPS_LOCK, 0x7_0000_0029)]).as_CFType();
        let updated = merged_mapping(Some(&original)).unwrap();
        let expected = CFArray::from_CFTypes(&[other.clone(), mapping(CAPS_LOCK, F18)]).as_CFType();
        assert_eq!(updated, expected);
        assert_eq!(
            original,
            CFArray::from_CFTypes(&[other, mapping(CAPS_LOCK, 0x7_0000_0029)]).as_CFType()
        );
        assert_eq!(merged_mapping(Some(&updated)).unwrap(), updated);
        assert!(merged_mapping(Some(&CFString::new("invalid").as_CFType())).is_err());
    }
}
