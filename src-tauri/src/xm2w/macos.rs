//! macOS transport: IOKit HID via raw C calls (dlsym).
//!
//! hidapi on macOS defaults to exclusive (Seize) opens which require root;
//! plain `IOHIDDeviceOpen(dev, kIOHIDOptionsTypeNone)` works as a normal user,
//! so we talk to IOKit directly - no TCC permission needed.

use super::Transport;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type CFStringRef = *mut c_void;
type CFDictionaryRef = *mut c_void;
type CFNumberRef = *mut c_void;
type CFSetRef = *mut c_void;

#[allow(non_upper_case_globals)]
const kIOHIDReportTypeFeature: c_int = 2;
#[allow(non_upper_case_globals)]
const kIOHIDOptionsTypeNone: u32 = 0;
#[allow(non_upper_case_globals)]
const kCFNumberSInt32Type: u32 = 3;
#[allow(non_upper_case_globals)]
const kCFStringEncodingUTF8: u32 = 0x08000100;

unsafe fn dlsym<T: Copy + Sized>(name: &str) -> T {
    let cname = CString::new(name).unwrap();
    let sym = libc::dlsym(libc::RTLD_DEFAULT, cname.as_ptr());
    assert!(!sym.is_null(), "dlsym({}) failed", name);
    std::mem::transmute_copy::<*mut c_void, T>(&sym)
}

type CFStringCreateWithCStringFn = unsafe extern "C" fn(*const c_void, *const c_char, u32) -> CFStringRef;
type CFNumberCreateFn = unsafe extern "C" fn(*const c_void, u32, *const c_void) -> CFNumberRef;
type CFDictionaryCreateMutableFn = unsafe extern "C" fn(*const c_void, isize, *const c_void, *const c_void) -> CFDictionaryRef;
type CFDictionarySetValueFn = unsafe extern "C" fn(CFDictionaryRef, *const c_void, *const c_void);
type CFSetGetCountFn = unsafe extern "C" fn(CFSetRef) -> isize;
type CFSetGetValuesFn = unsafe extern "C" fn(CFSetRef, *mut *const c_void);
type CFNumberGetValueFn = unsafe extern "C" fn(CFNumberRef, u32, *mut c_void) -> c_int;
type IOHIDManagerCreateFn = unsafe extern "C" fn(*const c_void, u32) -> IOHIDManagerRef;
type IOHIDManagerSetDeviceMatchingFn = unsafe extern "C" fn(IOHIDManagerRef, CFDictionaryRef);
type IOHIDManagerOpenFn = unsafe extern "C" fn(IOHIDManagerRef, u32) -> c_int;
type IOHIDManagerCopyDevicesFn = unsafe extern "C" fn(IOHIDManagerRef) -> CFSetRef;
type IOHIDDeviceGetPropertyFn = unsafe extern "C" fn(IOHIDDeviceRef, CFStringRef) -> *const c_void;
type IOHIDDeviceOpenFn = unsafe extern "C" fn(IOHIDDeviceRef, u32) -> c_int;
type IOHIDDeviceCloseFn = unsafe extern "C" fn(IOHIDDeviceRef, u32);
type IOHIDRequestAccessFn = unsafe extern "C" fn(c_int) -> u8;
type IOHIDDeviceSetReportFn = unsafe extern "C" fn(IOHIDDeviceRef, c_int, c_int, *const c_void, isize) -> c_int;
type IOHIDDeviceGetReportFn = unsafe extern "C" fn(IOHIDDeviceRef, c_int, c_int, *mut c_void, *mut isize) -> c_int;

struct Fns {
    cf_string_create: CFStringCreateWithCStringFn,
    cf_number_create: CFNumberCreateFn,
    cf_dict_create_mutable: CFDictionaryCreateMutableFn,
    cf_dict_set_value: CFDictionarySetValueFn,
    cf_set_get_count: CFSetGetCountFn,
    cf_set_get_values: CFSetGetValuesFn,
    cf_number_get_value: CFNumberGetValueFn,
    iohid_manager_create: IOHIDManagerCreateFn,
    iohid_manager_set_matching: IOHIDManagerSetDeviceMatchingFn,
    iohid_manager_open: IOHIDManagerOpenFn,
    iohid_manager_copy_devices: IOHIDManagerCopyDevicesFn,
    iohid_device_get_property: IOHIDDeviceGetPropertyFn,
    iohid_device_open: IOHIDDeviceOpenFn,
    iohid_device_close: IOHIDDeviceCloseFn,
    iohid_device_set_report: IOHIDDeviceSetReportFn,
    iohid_device_get_report: IOHIDDeviceGetReportFn,
}

unsafe fn load_fns() -> Fns {
    Fns {
        cf_string_create: dlsym("CFStringCreateWithCString"),
        cf_number_create: dlsym("CFNumberCreate"),
        cf_dict_create_mutable: dlsym("CFDictionaryCreateMutable"),
        cf_dict_set_value: dlsym("CFDictionarySetValue"),
        cf_set_get_count: dlsym("CFSetGetCount"),
        cf_set_get_values: dlsym("CFSetGetValues"),
        cf_number_get_value: dlsym("CFNumberGetValue"),
        iohid_manager_create: dlsym("IOHIDManagerCreate"),
        iohid_manager_set_matching: dlsym("IOHIDManagerSetDeviceMatching"),
        iohid_manager_open: dlsym("IOHIDManagerOpen"),
        iohid_manager_copy_devices: dlsym("IOHIDManagerCopyDevices"),
        iohid_device_get_property: dlsym("IOHIDDeviceGetProperty"),
        iohid_device_open: dlsym("IOHIDDeviceOpen"),
        iohid_device_close: dlsym("IOHIDDeviceClose"),
        iohid_device_set_report: dlsym("IOHIDDeviceSetReport"),
        iohid_device_get_report: dlsym("IOHIDDeviceGetReport"),
    }
}

unsafe fn cfstr(f: &Fns, s: &str) -> CFStringRef {
    let c = CString::new(s).unwrap();
    (f.cf_string_create)(std::ptr::null(), c.as_ptr(), kCFStringEncodingUTF8)
}

unsafe fn cfnum(f: &Fns, v: i32) -> CFNumberRef {
    (f.cf_number_create)(std::ptr::null(), kCFNumberSInt32Type, &v as *const i32 as *const c_void)
}

pub struct MacTransport {
    fns: Fns,
    dev: IOHIDDeviceRef,
}

impl MacTransport {
    pub fn open(pid: u16) -> Result<Self, String> {
        unsafe {
            let fns = load_fns();
            let mgr = (fns.iohid_manager_create)(std::ptr::null(), 0);
            if mgr.is_null() {
                return Err("IOHIDManagerCreate failed".into());
            }
            let match_dict = (fns.cf_dict_create_mutable)(std::ptr::null(), 2, std::ptr::null(), std::ptr::null());
            (fns.cf_dict_set_value)(match_dict, cfstr(&fns, "VendorID"), cfnum(&fns, 0x3367));
            (fns.cf_dict_set_value)(match_dict, cfstr(&fns, "ProductID"), cfnum(&fns, pid as i32));
            (fns.iohid_manager_set_matching)(mgr, match_dict);
            let mor = (fns.iohid_manager_open)(mgr, 0);
            if mor != 0 {
                // kIOReturnNotPermitted (0xe00002e2): Input Monitoring not granted.
                // Ask for it so the user sees the system permission prompt instead
                // of a silent failure (the grant is lost on every reinstall).
                let req: IOHIDRequestAccessFn = dlsym("IOHIDRequestAccess");
                let _ = req(0); // kIOHIDRequestTypeListenEvent
                let _ = req(1); // kIOHIDRequestTypePostEvent
                return Err(format!("IOHIDManagerOpen failed: 0x{:08x} (requested Input Monitoring — grant it in System Settings → Privacy & Security)", mor as u32));
            }
            let devs = (fns.iohid_manager_copy_devices)(mgr);
            if devs.is_null() {
                return Err("device not found - is the mouse plugged in?".into());
            }
            let n = (fns.cf_set_get_count)(devs);
            if n <= 0 {
                return Err("device not found - is the mouse plugged in?".into());
            }
            let mut arr: Vec<*const c_void> = vec![std::ptr::null(); n as usize];
            (fns.cf_set_get_values)(devs, arr.as_mut_ptr());
            // prefer the keyboard/vendor interface (primary usage 1/6)
            let mut dev: IOHIDDeviceRef = std::ptr::null_mut();
            for &cand in &arr {
                let pp = (fns.iohid_device_get_property)(cand as IOHIDDeviceRef, cfstr(&fns, "PrimaryUsagePage"));
                let pu = (fns.iohid_device_get_property)(cand as IOHIDDeviceRef, cfstr(&fns, "PrimaryUsage"));
                if pp.is_null() || pu.is_null() {
                    continue;
                }
                let mut vp: i32 = 0;
                let mut vu: i32 = 0;
                (fns.cf_number_get_value)(pp as CFNumberRef, kCFNumberSInt32Type, &mut vp as *mut i32 as *mut c_void);
                (fns.cf_number_get_value)(pu as CFNumberRef, kCFNumberSInt32Type, &mut vu as *mut i32 as *mut c_void);
                if vp == 1 && vu == 6 {
                    dev = cand as IOHIDDeviceRef;
                    break;
                }
            }
            if dev.is_null() {
                dev = arr[0] as IOHIDDeviceRef;
            }
            if (fns.iohid_device_open)(dev, kIOHIDOptionsTypeNone) != 0 {
                return Err("IOHIDDeviceOpen failed".into());
            }
            Ok(MacTransport { fns, dev })
        }
    }
}

impl Drop for MacTransport {
    fn drop(&mut self) {
        unsafe {
            (self.fns.iohid_device_close)(self.dev, kIOHIDOptionsTypeNone);
        }
    }
}

impl Transport for MacTransport {
    fn set_feature(&mut self, report_id: u8, payload: &[u8]) -> Result<(), String> {
        unsafe {
            let mut buf = vec![0u8; payload.len() + 1];
            buf[0] = report_id;
            buf[1..].copy_from_slice(payload);
            let r = (self.fns.iohid_device_set_report)(
                self.dev,
                kIOHIDReportTypeFeature,
                report_id as c_int,
                buf.as_ptr() as *const c_void,
                buf.len() as isize,
            );
            if r != 0 {
                return Err(format!("SetReport(0x{report_id:02x}) failed: 0x{:08x}", r as u32));
            }
            Ok(())
        }
    }

    fn get_feature(&mut self, report_id: u8, size: usize) -> Result<Vec<u8>, String> {
        unsafe {
            let mut buf = vec![0u8; size];
            buf[0] = report_id;
            let mut n = size as isize;
            let r = (self.fns.iohid_device_get_report)(
                self.dev,
                kIOHIDReportTypeFeature,
                report_id as c_int,
                buf.as_mut_ptr() as *mut c_void,
                &mut n as *mut isize,
            );
            if r != 0 {
                return Err(format!("GetReport(0x{report_id:02x}) failed: 0x{:08x}", r as u32));
            }
            buf.truncate(n as usize);
            Ok(buf)
        }
    }
}
