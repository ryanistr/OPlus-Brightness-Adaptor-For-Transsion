use std::os::raw::{c_int, c_char, c_uchar};

// android ffi imports
#[allow(dead_code)]
extern "C" {
    pub(crate) fn __system_property_get(name: *const c_uchar, value: *mut c_uchar) -> c_int;
    pub(crate) fn __system_property_set(name: *const c_uchar, value: *const c_uchar) -> c_int;
    pub(crate) fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}
