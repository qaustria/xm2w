//! Drift Guard — cancels the XM2w phantom (-1,-1) report offset.
//!
//! The dongle's *pointing* interface (usage page 1, usage 2) emits a
//! constant (-1,-1) delta on every report at ~1 kHz — a stuck motion state
//! in the mouse firmware/hardware (verified: CPI-independent, survives
//! factory reset, present in the raw report stream as
//! `01 00 ff ff ff ff 00 00`). We seize that interface exclusively, drop
//! reports at or below the dead zone, and re-inject real movement and
//! button presses through CGEventPost, so the OS and games only ever see
//! intentional input.
//!
//! Only the pointing interface is seized: the keyboard interface (button
//! binds) and the consumer/wheel interface stay native, so binds and
//! scrolling keep working untouched.

use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU8, AtomicPtr, Ordering};
use std::sync::Mutex;

#[allow(non_upper_case_globals)]
const kIOHIDOptionsTypeNone: u32 = 0;
#[allow(non_upper_case_globals)]
const kIOHIDOptionsTypeSeizeDevice: u32 = 1;
#[allow(non_upper_case_globals)]
const kCFNumberSInt32Type: u32 = 3;
#[allow(non_upper_case_globals)]
const kCFStringEncodingUTF8: u32 = 0x08000100;

// CGEvent types / fields / tap locations
#[allow(non_upper_case_globals)]
const K_CG_MOUSE_MOVED: u32 = 5;
#[allow(non_upper_case_globals)]
const K_CG_LEFT_DOWN: u32 = 1;
#[allow(non_upper_case_globals)]
const K_CG_LEFT_UP: u32 = 2;
#[allow(non_upper_case_globals)]
const K_CG_RIGHT_DOWN: u32 = 3;
#[allow(non_upper_case_globals)]
const K_CG_RIGHT_UP: u32 = 4;
#[allow(non_upper_case_globals)]
const K_CG_OTHER_DOWN: u32 = 25;
#[allow(non_upper_case_globals)]
const K_CG_OTHER_UP: u32 = 26;
#[allow(non_upper_case_globals)]
const K_CG_FIELD_DELTA_X: u32 = 11;
#[allow(non_upper_case_globals)]
const K_CG_FIELD_DELTA_Y: u32 = 12;
#[allow(non_upper_case_globals)]
const K_CG_FIELD_BUTTON: u32 = 3;
#[allow(non_upper_case_globals)]
const K_CG_HID_TAP: u32 = 0;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static DEADZONE: AtomicI16 = AtomicI16::new(1);
static PREV_BUTTONS: AtomicU8 = AtomicU8::new(0);
static RUNLOOP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

#[derive(Clone, Copy)]
#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}

// ---------------------------------------------------------------------------
// dlsym plumbing (same style as macos.rs)
// ---------------------------------------------------------------------------

unsafe fn dlsym<T: Copy + Sized>(name: &str) -> T {
    let cname = std::ffi::CString::new(name).unwrap();
    let sym = libc::dlsym(libc::RTLD_DEFAULT, cname.as_ptr());
    assert!(!sym.is_null(), "dlsym({}) failed", name);
    std::mem::transmute_copy::<*mut c_void, T>(&sym)
}

type CFStringCreateWithCStringFn = unsafe extern "C" fn(*const c_void, *const c_char, u32) -> *mut c_void;
type CFNumberCreateFn = unsafe extern "C" fn(*const c_void, u32, *const c_void) -> *mut c_void;
type CFDictionaryCreateMutableFn = unsafe extern "C" fn(*const c_void, isize, *const c_void, *const c_void) -> *mut c_void;
type CFDictionarySetValueFn = unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void);
type CFReleaseFn = unsafe extern "C" fn(*const c_void);
type IOHIDManagerCreateFn = unsafe extern "C" fn(*const c_void, u32) -> *mut c_void;
type IOHIDManagerSetDeviceMatchingFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
type IOHIDManagerOpenFn = unsafe extern "C" fn(*mut c_void, u32) -> c_int;
type IOHIDManagerRegisterInputReportCallbackFn = unsafe extern "C" fn(
    *mut c_void,
    Option<unsafe extern "C" fn(*mut c_void, c_int, *mut c_void, c_int, u32, *const u8, isize)>,
    *mut c_void,
);
type IOHIDManagerScheduleWithRunLoopFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void);
type IOHIDManagerUnscheduleFromRunLoopFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void);
type IOHIDManagerCloseFn = unsafe extern "C" fn(*mut c_void, u32);
type CFRunLoopGetCurrentFn = unsafe extern "C" fn() -> *mut c_void;
type CFRunLoopRunFn = unsafe extern "C" fn();
type CFRunLoopStopFn = unsafe extern "C" fn(*mut c_void);
type CGEventCreateMouseEventFn = unsafe extern "C" fn(*const c_void, u32, CGPoint, u32) -> *mut c_void;
type CGEventSetIntegerValueFieldFn = unsafe extern "C" fn(*mut c_void, u32, i64);
type CGEventPostFn = unsafe extern "C" fn(u32, *mut c_void);
type IOHIDRequestAccessFn = unsafe extern "C" fn(c_int) -> u8;

struct CgFns {
    cf_release: CFReleaseFn,
    cg_create_mouse: CGEventCreateMouseEventFn,
    cg_set_field: CGEventSetIntegerValueFieldFn,
    cg_post: CGEventPostFn,
}

unsafe fn load_cg_fns() -> CgFns {
    CgFns {
        cf_release: dlsym("CFRelease"),
        cg_create_mouse: dlsym("CGEventCreateMouseEvent"),
        cg_set_field: dlsym("CGEventSetIntegerValueField"),
        cg_post: dlsym("CGEventPost"),
    }
}

// ---------------------------------------------------------------------------
// Filter logic (pure, unit-tested)
// ---------------------------------------------------------------------------

/// Drop reports at or below the dead zone (the phantom is exactly (-1,-1)
/// on every report; real flicks are far larger).
pub fn filter_report(deadzone: i16, dx: i16, dy: i16) -> (i16, i16) {
    if dx.abs() <= deadzone && dy.abs() <= deadzone {
        (0, 0)
    } else {
        (dx, dy)
    }
}

/// Map the report's button bits (usage page 9: 1=Left, 2=Right, 3=Middle,
/// 4=Back, 5=Forward, 6..8=extra) to CGEvent mouse buttons and produce
/// (event_type, cg_button) pairs for changed buttons. prev==new → empty.
fn button_changes(prev: u8, new: u8) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let changed = prev ^ new;
    for bit in 0..8 {
        if changed & (1 << bit) != 0 {
            let down = new & (1 << bit) != 0;
            let kind = if bit == 0 {
                if down { K_CG_LEFT_DOWN } else { K_CG_LEFT_UP }
            } else if bit == 1 {
                if down { K_CG_RIGHT_DOWN } else { K_CG_RIGHT_UP }
            } else {
                if down { K_CG_OTHER_DOWN } else { K_CG_OTHER_UP }
            };
            out.push((kind, bit as u32));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Callback (runs on the guard thread's run loop)
// ---------------------------------------------------------------------------

unsafe extern "C" fn on_report(
    ctx: *mut c_void,
    _result: c_int,
    _sender: *mut c_void,
    _rtype: c_int,
    _report_id: u32,
    report: *const u8,
    len: isize,
) {
    if ctx.is_null() || report.is_null() || len < 6 {
        return;
    }
    let r = std::slice::from_raw_parts(report, len as usize);
    if r[0] != 0x01 {
        return; // only the movement report (ID 1)
    }
    let fns = &*(ctx as *const CgFns);
    let buttons = r[1];
    let raw_dx = i16::from_le_bytes([r[2], r[3]]);
    let raw_dy = i16::from_le_bytes([r[4], r[5]]);
    let dz = DEADZONE.load(Ordering::Relaxed);
    let (dx, dy) = filter_report(dz, raw_dx, raw_dy);

    let zero = CGPoint { x: 0.0, y: 0.0 };
    if dx != 0 || dy != 0 {
        let ev = (fns.cg_create_mouse)(std::ptr::null(), K_CG_MOUSE_MOVED, zero, 0);
        if !ev.is_null() {
            (fns.cg_set_field)(ev, K_CG_FIELD_DELTA_X, dx as i64);
            (fns.cg_set_field)(ev, K_CG_FIELD_DELTA_Y, dy as i64);
            (fns.cg_post)(K_CG_HID_TAP, ev);
            (fns.cf_release)(ev);
        }
    }

    let prev = PREV_BUTTONS.swap(buttons, Ordering::Relaxed);
    if prev != buttons {
        for (kind, btn) in button_changes(prev, buttons) {
            let ev = (fns.cg_create_mouse)(std::ptr::null(), kind, zero, btn);
            if !ev.is_null() {
                (fns.cg_set_field)(ev, K_CG_FIELD_BUTTON, btn as i64);
                (fns.cg_post)(K_CG_HID_TAP, ev);
                (fns.cf_release)(ev);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

pub fn set_deadzone(n: i16) {
    DEADZONE.store(n.max(0).min(32), Ordering::Relaxed);
}

pub fn deadzone() -> i16 {
    DEADZONE.load(Ordering::Relaxed)
}

/// Start the guard: seize the pointing interface, filter + re-inject.
/// Requires macOS + Accessibility permission (for CGEventPost).
pub fn start() -> Result<(), String> {
    if is_active() {
        return Ok(());
    }
    if crate::xm2w::emu::emu_enabled() {
        return Err("Drift Guard is unavailable in emulator mode".into());
    }
    #[cfg(target_os = "macos")]
    {
        if !crate::permissions::check_accessibility() {
            let _ = crate::permissions::request_accessibility();
            return Err(
                "Drift Guard needs the Accessibility permission — enable it in System Settings → Privacy & Security → Accessibility, then toggle Drift Guard again"
                    .into(),
            );
        }
        let handle = std::thread::Builder::new()
            .name("drift-guard".into())
            .spawn(guard_thread_main)
            .map_err(|e| format!("failed to start thread: {e}"))?;
        *THREAD.lock().unwrap() = Some(handle);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Drift Guard is only supported on macOS".into())
    }
}

/// Stop the guard and release the device.
pub fn stop() {
    if !is_active() {
        return;
    }
    let rl = RUNLOOP.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !rl.is_null() {
        unsafe {
            let stop: CFRunLoopStopFn = dlsym("CFRunLoopStop");
            (stop)(rl);
        }
    }
    if let Some(h) = THREAD.lock().unwrap().take() {
        let _ = h.join();
    }
}

#[cfg(target_os = "macos")]
fn guard_thread_main() {
    unsafe {
        {
            let cf_string_create: CFStringCreateWithCStringFn = dlsym("CFStringCreateWithCString");
            let cf_number_create: CFNumberCreateFn = dlsym("CFNumberCreate");
            let cf_dict_create_mutable: CFDictionaryCreateMutableFn = dlsym("CFDictionaryCreateMutable");
            let cf_dict_set_value: CFDictionarySetValueFn = dlsym("CFDictionarySetValue");
            let _cf_release: CFReleaseFn = dlsym("CFRelease");
            let iohid_manager_create: IOHIDManagerCreateFn = dlsym("IOHIDManagerCreate");
            let iohid_manager_set_matching: IOHIDManagerSetDeviceMatchingFn = dlsym("IOHIDManagerSetDeviceMatching");
            let iohid_manager_open: IOHIDManagerOpenFn = dlsym("IOHIDManagerOpen");

            let cfstr = |s: &str| {
                let c = std::ffi::CString::new(s).unwrap();
                (cf_string_create)(std::ptr::null(), c.as_ptr(), kCFStringEncodingUTF8)
            };
            let cfnum = |v: i32| {
                (cf_number_create)(std::ptr::null(), kCFNumberSInt32Type, &v as *const i32 as *const c_void)
            };

            let mgr = (iohid_manager_create)(std::ptr::null(), 0);
            if mgr.is_null() {
                eprintln!("[driftguard] IOHIDManagerCreate failed");
                return;
            }
            let dict = (cf_dict_create_mutable)(std::ptr::null(), 3, std::ptr::null(), std::ptr::null());
            (cf_dict_set_value)(dict, cfstr("VendorID"), cfnum(0x3367));
            (cf_dict_set_value)(dict, cfstr("PrimaryUsagePage"), cfnum(1));
            (cf_dict_set_value)(dict, cfstr("PrimaryUsage"), cfnum(2));
            (iohid_manager_set_matching)(mgr, dict);

            let r = (iohid_manager_open)(mgr, kIOHIDOptionsTypeSeizeDevice);
            if r != 0 {
                // Input Monitoring may be missing after reinstall — ask for it.
                let req: IOHIDRequestAccessFn = dlsym("IOHIDRequestAccess");
                let _ = req(0);
                eprintln!("[driftguard] seize failed: 0x{:08x}", r as u32);
                return;
            }

            let cg = load_cg_fns();
            let register: IOHIDManagerRegisterInputReportCallbackFn =
                dlsym("IOHIDManagerRegisterInputReportCallback");
            let schedule: IOHIDManagerScheduleWithRunLoopFn = dlsym("IOHIDManagerScheduleWithRunLoop");
            let runloop_get_current: CFRunLoopGetCurrentFn = dlsym("CFRunLoopGetCurrent");
            let runloop_run: CFRunLoopRunFn = dlsym("CFRunLoopRun");

            let ctx = Box::into_raw(Box::new(cg)) as *mut c_void;
            (register)(mgr, Some(on_report), ctx);
            let rl = (runloop_get_current)();
            RUNLOOP.store(rl, Ordering::SeqCst);
            (schedule)(mgr, rl, std::ptr::null());
            ACTIVE.store(true, Ordering::SeqCst);
            eprintln!("[driftguard] ACTIVE (deadzone={})", DEADZONE.load(Ordering::Relaxed));

            (runloop_run)(); // blocks until CFRunLoopStop

            ACTIVE.store(false, Ordering::SeqCst);
            let unschedule: IOHIDManagerUnscheduleFromRunLoopFn = dlsym("IOHIDManagerUnscheduleFromRunLoop");
            let close: IOHIDManagerCloseFn = dlsym("IOHIDManagerClose");
            (unschedule)(mgr, rl, std::ptr::null());
            (close)(mgr, kIOHIDOptionsTypeNone);
            drop(Box::from_raw(ctx as *mut CgFns));
            eprintln!("[driftguard] stopped");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn guard_thread_main() {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phantom_is_dropped() {
        assert_eq!(filter_report(1, -1, -1), (0, 0));
        assert_eq!(filter_report(1, 1, -1), (0, 0));
        assert_eq!(filter_report(1, 0, 0), (0, 0));
        assert_eq!(filter_report(2, -2, 2), (0, 0));
    }

    #[test]
    fn real_movement_passes() {
        assert_eq!(filter_report(1, 5, -3), (5, -3));
        assert_eq!(filter_report(1, -2, 0), (-2, 0));
        assert_eq!(filter_report(1, 0, 9), (0, 9));
    }

    #[test]
    fn button_edges() {
        assert_eq!(button_changes(0, 0), vec![]);
        assert_eq!(button_changes(0, 0x01), vec![(K_CG_LEFT_DOWN, 0)]);
        assert_eq!(button_changes(0x01, 0), vec![(K_CG_LEFT_UP, 0)]);
        assert_eq!(button_changes(0, 0x02), vec![(K_CG_RIGHT_DOWN, 1)]);
        assert_eq!(button_changes(0, 0x04), vec![(K_CG_OTHER_DOWN, 2)]);
        assert_eq!(button_changes(0, 0x08), vec![(K_CG_OTHER_DOWN, 3)]); // back
        assert_eq!(button_changes(0, 0x10), vec![(K_CG_OTHER_DOWN, 4)]); // forward
        let both = button_changes(0x01 | 0x04, 0x02 | 0x08);
        assert!(both.contains(&(K_CG_LEFT_UP, 0)));
        assert!(both.contains(&(K_CG_OTHER_UP, 2)));
        assert!(both.contains(&(K_CG_RIGHT_DOWN, 1)));
        assert!(both.contains(&(K_CG_OTHER_DOWN, 3)));
    }

    #[test]
    fn deadzone_clamped() {
        set_deadzone(999);
        assert_eq!(deadzone(), 32);
        set_deadzone(-5);
        assert_eq!(deadzone(), 0);
        set_deadzone(1);
        assert_eq!(deadzone(), 1);
    }
}
