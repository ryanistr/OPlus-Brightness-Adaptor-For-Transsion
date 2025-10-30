use std::ffi::CString;
use std::os::raw::c_int;
use crate::ffi::__android_log_print;
use crate::paths::log_tag;
use crate::constants::{LOG_DEBUG, LOG_ERROR};

// logging utilities
pub(crate) fn log_write(level: c_int, msg: &str) {
    let tag = CString::new(log_tag()).unwrap();
    let fmt = CString::new("%s").unwrap();
    let c_msg = CString::new(msg).unwrap();
    unsafe { __android_log_print(level, tag.as_ptr(), fmt.as_ptr(), c_msg.as_ptr()) };
}
pub(crate) fn log_d(msg: &str) { log_write(LOG_DEBUG, msg); }
pub(crate) fn log_e(msg: &str) { log_write(LOG_ERROR, msg); }
