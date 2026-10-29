//! Driver-level keyboard remapping via Apple CoreHID & IOHIDServiceClient.
//! Discovered via Hyperkey reverse engineering (HidRemap.swift & KeyboardListener.swift).

use core_foundation::base::TCFType;
use std::ffi::{c_char, c_void};
use std::ptr::null_mut;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64_TYPE: isize = 4;
const K_HID_PAGE_GENERIC_DESKTOP: u32 = 1;
const K_HID_USAGE_KEYBOARD: u32 = 6;
const KEYBOARD_USAGE_PAGE_BASE: u64 = 0x7_0000_0000;
const USAGE_CAPS_LOCK: u64 = 0x39;
const USAGE_F18: u64 = 0x6D;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, define_class, msg_send, sel};
use objc2_foundation::{NSNotification, NSObject};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "ClicklessWakeObserver"]
    struct WakeObserver;

    impl WakeObserver {
        #[unsafe(method(receiveWakeNote:))]
        fn receive_wake_note(&self, _note: &NSNotification) {
            objc2::rc::autoreleasepool(|_| {
                apply_caps_lock_remap(true);
            });
        }
    }
);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: *mut c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *mut c_void;
    fn CFNumberCreate(alloc: *mut c_void, the_type: isize, value_ptr: *const c_void)
    -> *mut c_void;
    fn CFDictionaryCreate(
        alloc: *mut c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    fn CFArrayCreate(
        alloc: *mut c_void,
        values: *const *const c_void,
        num_values: isize,
        callbacks: *const c_void,
    ) -> *mut c_void;
    fn CFArrayGetCount(array: *mut c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *mut c_void, idx: isize) -> *mut c_void;
    fn CFRelease(cf: *mut c_void);

    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFTypeArrayCallBacks: c_void;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDEventSystemClientCreateSimpleClient(alloc: *mut c_void) -> *mut c_void;
    fn IOHIDEventSystemClientCopyServices(client: *mut c_void) -> *mut c_void;
    fn IOHIDServiceClientConformsTo(
        service: *mut c_void,
        usage_page: u32,
        usage: u32,
    ) -> libc::boolean_t;
    fn IOHIDServiceClientSetProperty(
        service: *mut c_void,
        key: *mut c_void,
        property: *mut c_void,
    ) -> libc::boolean_t;

    fn IOHIDManagerCreate(alloc: *mut c_void, options: u32) -> *mut c_void;
    fn IOHIDManagerSetDeviceMatching(manager: *mut c_void, matching: *mut c_void);
    fn IOHIDManagerRegisterDeviceMatchingCallback(
        manager: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void, i32, *mut c_void, *mut c_void),
        context: *mut c_void,
    );
    fn IOHIDManagerRegisterDeviceRemovalCallback(
        manager: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void, i32, *mut c_void, *mut c_void),
        context: *mut c_void,
    );
    fn IOHIDManagerScheduleWithRunLoop(
        manager: *mut c_void,
        runloop: *mut c_void,
        mode: core_foundation::string::CFStringRef,
    );
    fn IOHIDManagerUnscheduleFromRunLoop(
        manager: *mut c_void,
        runloop: *mut c_void,
        mode: core_foundation::string::CFStringRef,
    );
    fn IOHIDManagerOpen(manager: *mut c_void, options: u32) -> i32;
    fn IOHIDManagerClose(manager: *mut c_void, options: u32) -> i32;
}

unsafe extern "C" fn hotplug_device_removal_callback(
    _context: *mut c_void,
    _result: i32,
    _sender: *mut c_void,
    _device: *mut c_void,
) {
    // When a USB/Bluetooth keyboard is removed, re-apply the remap to remaining devices
    objc2::rc::autoreleasepool(|_| {
        apply_caps_lock_remap(true);
    });
}

unsafe extern "C" fn hotplug_device_matched_callback(
    _context: *mut c_void,
    _result: i32,
    _sender: *mut c_void,
    _device: *mut c_void,
) {
    // When a new USB/Bluetooth keyboard is plugged in, re-apply the remap
    objc2::rc::autoreleasepool(|_| {
        apply_caps_lock_remap(true);
    });
}

/// Applies or clears UserKeyMapping on all keyboards matching (GenericDesktop, Keyboard).
fn apply_caps_lock_remap(enable: bool) {
    unsafe {
        let client = IOHIDEventSystemClientCreateSimpleClient(null_mut());
        if client.is_null() {
            return;
        }

        let services = IOHIDEventSystemClientCopyServices(client);
        if services.is_null() {
            CFRelease(client);
            return;
        }

        let prop_key = CFStringCreateWithCString(
            null_mut(),
            c"UserKeyMapping".as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );

        let mapping_array = if enable {
            let src_key = CFStringCreateWithCString(
                null_mut(),
                c"HIDKeyboardModifierMappingSrc".as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            );
            let dst_key = CFStringCreateWithCString(
                null_mut(),
                c"HIDKeyboardModifierMappingDst".as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            );

            let src_val_u64 = (KEYBOARD_USAGE_PAGE_BASE | USAGE_CAPS_LOCK) as i64;
            let dst_val_u64 = (KEYBOARD_USAGE_PAGE_BASE | USAGE_F18) as i64;

            let src_val = CFNumberCreate(
                null_mut(),
                K_CF_NUMBER_SINT64_TYPE,
                (&src_val_u64 as *const i64).cast(),
            );
            let dst_val = CFNumberCreate(
                null_mut(),
                K_CF_NUMBER_SINT64_TYPE,
                (&dst_val_u64 as *const i64).cast(),
            );

            let keys = [src_key.cast_const(), dst_key.cast_const()];
            let vals = [src_val.cast_const(), dst_val.cast_const()];

            let dict = CFDictionaryCreate(
                null_mut(),
                keys.as_ptr(),
                vals.as_ptr(),
                2,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );

            let arr_items = [dict.cast_const()];
            let arr = CFArrayCreate(null_mut(), arr_items.as_ptr(), 1, &kCFTypeArrayCallBacks);

            CFRelease(src_key);
            CFRelease(dst_key);
            CFRelease(src_val);
            CFRelease(dst_val);
            CFRelease(dict);
            arr
        } else {
            // Empty array resets keyboard mapping to system defaults
            CFArrayCreate(null_mut(), null_mut(), 0, &kCFTypeArrayCallBacks)
        };

        let count = CFArrayGetCount(services);
        for i in 0..count {
            let service = CFArrayGetValueAtIndex(services, i);
            if IOHIDServiceClientConformsTo(
                service,
                K_HID_PAGE_GENERIC_DESKTOP,
                K_HID_USAGE_KEYBOARD,
            ) != 0
            {
                IOHIDServiceClientSetProperty(service, prop_key, mapping_array);
            }
        }

        CFRelease(mapping_array);
        CFRelease(prop_key);
        CFRelease(services);
        CFRelease(client);
    }
}

/// RAII guard that manages driver-level Caps Lock remapping and hotplug device callbacks.
pub struct HidRemapGuard {
    manager: *mut c_void,
    runloop: core_foundation::runloop::CFRunLoop,
    wake_observer: Option<Retained<WakeObserver>>,
    notification_center: Option<Retained<objc2_foundation::NSNotificationCenter>>,
    active: bool,
}

impl HidRemapGuard {
    pub fn install() -> Self {
        apply_caps_lock_remap(true);

        let (wake_observer, notification_center) = {
            let ws = objc2_app_kit::NSWorkspace::sharedWorkspace();
            let nc = ws.notificationCenter();
            let observer: Retained<WakeObserver> = unsafe { msg_send![WakeObserver::class(), new] };
            let obj: &AnyObject = &observer;
            unsafe {
                nc.addObserver_selector_name_object(
                    obj,
                    sel!(receiveWakeNote:),
                    Some(objc2_app_kit::NSWorkspaceDidWakeNotification),
                    None,
                );
            }
            (Some(observer), Some(nc))
        };

        let runloop = core_foundation::runloop::CFRunLoop::get_current();
        let manager = unsafe {
            let mgr = IOHIDManagerCreate(null_mut(), 0);
            if !mgr.is_null() {
                // Setup device matching dictionary for keyboards
                let page_key = CFStringCreateWithCString(
                    null_mut(),
                    c"DeviceUsagePage".as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                );
                let usage_key = CFStringCreateWithCString(
                    null_mut(),
                    c"DeviceUsage".as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                );

                let page_val_i64 = K_HID_PAGE_GENERIC_DESKTOP as i64;
                let usage_val_i64 = K_HID_USAGE_KEYBOARD as i64;
                let page_val = CFNumberCreate(
                    null_mut(),
                    K_CF_NUMBER_SINT64_TYPE,
                    (&page_val_i64 as *const i64).cast(),
                );
                let usage_val = CFNumberCreate(
                    null_mut(),
                    K_CF_NUMBER_SINT64_TYPE,
                    (&usage_val_i64 as *const i64).cast(),
                );

                let keys = [page_key.cast_const(), usage_key.cast_const()];
                let vals = [page_val.cast_const(), usage_val.cast_const()];
                let match_dict = CFDictionaryCreate(
                    null_mut(),
                    keys.as_ptr(),
                    vals.as_ptr(),
                    2,
                    &kCFTypeDictionaryKeyCallBacks,
                    &kCFTypeDictionaryValueCallBacks,
                );

                IOHIDManagerSetDeviceMatching(mgr, match_dict);
                IOHIDManagerRegisterDeviceMatchingCallback(
                    mgr,
                    hotplug_device_matched_callback,
                    null_mut(),
                );
                IOHIDManagerRegisterDeviceRemovalCallback(
                    mgr,
                    hotplug_device_removal_callback,
                    null_mut(),
                );

                CFRelease(page_key);
                CFRelease(usage_key);
                CFRelease(page_val);
                CFRelease(usage_val);
                CFRelease(match_dict);

                IOHIDManagerScheduleWithRunLoop(
                    mgr,
                    runloop.as_concrete_TypeRef().cast(),
                    core_foundation::runloop::kCFRunLoopCommonModes,
                );

                IOHIDManagerOpen(mgr, 0);
            }
            mgr
        };

        Self {
            manager,
            runloop,
            wake_observer,
            notification_center,
            active: true,
        }
    }
}

impl Drop for HidRemapGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;

        // 1. Remove notification observer first so wake events cannot fire during teardown
        if let (Some(observer), Some(nc)) = (&self.wake_observer, &self.notification_center) {
            let obj: &AnyObject = observer;
            unsafe {
                nc.removeObserver(obj);
            }
        }

        // 2. Unschedule manager from runloop and close so hotplug callbacks cannot fire
        if !self.manager.is_null() {
            unsafe {
                IOHIDManagerUnscheduleFromRunLoop(
                    self.manager,
                    self.runloop.as_concrete_TypeRef().cast(),
                    core_foundation::runloop::kCFRunLoopCommonModes,
                );
                IOHIDManagerClose(self.manager, 0);
                CFRelease(self.manager);
            }
        }

        // 3. Restore driver mapping back to defaults with all callbacks silenced
        apply_caps_lock_remap(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_observer_class_responds_to_wake_selector() {
        let cls = WakeObserver::class();
        assert!(cls.responds_to(sel!(receiveWakeNote:)));
    }
}
