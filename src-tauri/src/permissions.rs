//! macOS permission checks & requests (HID access / Accessibility).
//!
//! The app talks to the mouse over the IOKit HID feature-report interface.
//! macOS 14+ may gate this behind the Input Monitoring permission, so we
//! check/request it on first run and surface the state in the UI.

#[cfg(target_os = "macos")]
mod imp {
    use std::os::raw::{c_int, c_void};

    #[allow(non_upper_case_globals)]
    const kIOHIDRequestTypeListenEvent: c_int = 0;
    #[allow(non_upper_case_globals)]
    const kIOHIDRequestTypePostEvent: c_int = 1;

    unsafe fn dlsym<T: Copy + Sized>(name: &str) -> T {
        let cname = std::ffi::CString::new(name).unwrap();
        let sym = libc::dlsym(libc::RTLD_DEFAULT, cname.as_ptr());
        assert!(!sym.is_null(), "dlsym({}) failed", name);
        std::mem::transmute_copy::<*mut c_void, T>(&sym)
    }

    type IOHIDCheckAccessFn = unsafe extern "C" fn(c_int) -> u8;
    type IOHIDRequestAccessFn = unsafe extern "C" fn(c_int) -> u8;
    type AXIsProcessTrustedFn = unsafe extern "C" fn() -> u8;
    type AXIsProcessTrustedWithOptionsFn = unsafe extern "C" fn(*const c_void) -> u8;
    type CFStringCreateWithCStringFn =
        unsafe extern "C" fn(*const c_void, *const std::os::raw::c_char, u32) -> *const c_void;
    type CFDictionaryCreateMutableFn = unsafe extern "C" fn(
        *const c_void,
        isize,
        *const c_void,
        *const c_void,
    ) -> *const c_void;
    type CFDictionarySetValueFn = unsafe extern "C" fn(*const c_void, *const c_void, *const c_void);
    type CFReleaseFn = unsafe extern "C" fn(*const c_void);

    /// Input Monitoring access for HID event listening.
    pub fn check_input_monitoring() -> bool {
        unsafe {
            let f: IOHIDCheckAccessFn = dlsym("IOHIDCheckAccess");
            f(kIOHIDRequestTypeListenEvent) != 0
        }
    }

    /// Request Input Monitoring (shows the system permission prompt).
    /// Returns true if access was already granted or the prompt was accepted.
    pub fn request_input_monitoring() -> bool {
        unsafe {
            let f: IOHIDRequestAccessFn = dlsym("IOHIDRequestAccess");
            let ok = f(kIOHIDRequestTypeListenEvent) != 0;
            // also request post-event access for completeness
            let f2: IOHIDRequestAccessFn = dlsym("IOHIDRequestAccess");
            let _ = f2(kIOHIDRequestTypePostEvent);
            ok || check_input_monitoring()
        }
    }

    /// Accessibility trust (needed by some HID-related APIs on newer macOS).
    pub fn check_accessibility() -> bool {
        unsafe {
            let f: AXIsProcessTrustedFn = dlsym("AXIsProcessTrusted");
            f() != 0
        }
    }

    /// Request Accessibility trust with the system prompt.
    pub fn request_accessibility() -> bool {
        unsafe {
            let cf_str: CFStringCreateWithCStringFn = dlsym("CFStringCreateWithCString");
            let cf_dict: CFDictionaryCreateMutableFn = dlsym("CFDictionaryCreateMutable");
            let cf_set: CFDictionarySetValueFn = dlsym("CFDictionarySetValue");
            let cf_release: CFReleaseFn = dlsym("CFRelease");

            let key = cf_str(std::ptr::null(), c"AXTrustedCheckOptionPrompt".as_ptr(), 0x08000100);
            let val = cf_str(std::ptr::null(), c"true".as_ptr(), 0x08000100);
            let opts = cf_dict(std::ptr::null(), 1, std::ptr::null(), std::ptr::null());
            cf_set(opts, key, val);
            let f: AXIsProcessTrustedWithOptionsFn = dlsym("AXIsProcessTrustedWithOptions");
            let granted = f(opts) != 0;
            cf_release(opts);
            cf_release(key);
            cf_release(val);
            granted || check_accessibility()
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn check_input_monitoring() -> bool {
        true
    }
    pub fn request_input_monitoring() -> bool {
        true
    }
    pub fn check_accessibility() -> bool {
        true
    }
    pub fn request_accessibility() -> bool {
        true
    }
}

pub use imp::*;

/// Full permission report for the UI.
pub fn status() -> serde_json::Value {
    serde_json::json!({
        "input_monitoring": check_input_monitoring(),
        "accessibility": check_accessibility(),
    })
}

/// Request every permission the app may need; returns the new status.
pub fn request_all() -> serde_json::Value {
    let _ = request_input_monitoring();
    let _ = request_accessibility();
    status()
}
