use std::ffi::CString;
use std::io;
use crate::logging::{log_d, log_e};

// brightness write function
pub(crate) fn write_brightness(fd: i32, val: i32, last_val: &mut i32, dbg: bool) {
    if *last_val == val {
        return;
    }
    if dbg { log_d(&format!("[DisplayAdaptor] Writing brightness: {} -> {}", *last_val, val)); }

    let s = val.to_string();
    let c_str = match CString::new(s.as_bytes()) { Ok(c) => c, Err(_) => { log_e("[DisplayAdaptor] Failed to create CString"); return; } };
    let bytes = c_str.as_bytes_with_nul();

    let result = unsafe { libc::write(fd, bytes.as_ptr() as *const _, bytes.len()) };
    if result < 0 {
        if dbg { log_e(&format!("[DisplayAdaptor] Write failed for value {}: {}", val, io::Error::last_os_error())); }
    } else {
        *last_val = val;
    }
}
