use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn IsSecureEventInputEnabled() -> bool;
}

pub fn check_accessibility_permissions(prompt: bool) -> bool {
    if prompt {
        use core_foundation::number::kCFBooleanTrue;

        unsafe {
            // macOS expects a CFDictionary mapping kAXTrustedCheckOptionPrompt to kCFBooleanTrue
            let keys = [kAXTrustedCheckOptionPrompt.cast::<c_void>()];
            let values = [kCFBooleanTrue.cast::<c_void>()];
            let dict = core_foundation::dictionary::CFDictionaryCreate(
                core_foundation::base::kCFAllocatorDefault,
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &core_foundation::dictionary::kCFTypeDictionaryKeyCallBacks,
                &core_foundation::dictionary::kCFTypeDictionaryValueCallBacks,
            );

            let trusted = AXIsProcessTrustedWithOptions(dict.cast());
            core_foundation::base::CFRelease(dict.cast());
            trusted
        }
    } else {
        unsafe { AXIsProcessTrusted() }
    }
}

pub fn is_secure_input_enabled() -> bool {
    unsafe { IsSecureEventInputEnabled() }
}
