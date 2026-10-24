//! Native accessibility preferences. Queries run on the UI thread; no timers,
//! background work or artificial task progress is introduced by this module.
pub fn system_reduced() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        use std::ffi::{c_char, c_void};
        #[link(name = "AppKit", kind = "framework")]
        unsafe extern "C" {}
        #[link(name = "objc")]
        unsafe extern "C" {
            fn objc_getClass(name: *const c_char) -> *mut c_void;
            fn sel_registerName(name: *const c_char) -> *mut c_void;
            fn objc_msgSend();
        }
        let class = objc_getClass(c"NSWorkspace".as_ptr());
        if class.is_null() {
            return false;
        }
        let send_object: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let workspace = send_object(class, sel_registerName(c"sharedWorkspace".as_ptr()));
        if workspace.is_null() {
            return false;
        }
        let send_bool: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        return send_bool(
            workspace,
            sel_registerName(c"accessibilityDisplayShouldReduceMotion".as_ptr()),
        );
    }
    #[cfg(target_os = "windows")]
    unsafe {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn SystemParametersInfoW(action: u32, param: u32, value: *mut i32, flags: u32) -> i32;
        }
        let mut enabled = 1;
        return SystemParametersInfoW(0x1042, 0, &mut enabled, 0) != 0 && enabled == 0;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}
