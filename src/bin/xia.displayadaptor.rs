use std::ffi::{CString, CStr};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use std::{thread::sleep};
use std::os::raw::{c_int, c_char, c_uchar};
use base64::{engine::general_purpose, Engine};

// import android stuff
unsafe extern "C" {
    fn __system_property_get(name: *const c_uchar, value: *mut c_uchar) -> c_int;
    fn __system_property_set(name: *const c_uchar, value: *const c_uchar) -> c_int;
    fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

// logging utils
const LG_D: c_int = 3; // Debug
const LG_E: c_int = 6; // Error

// fixed fallback OS brightness value
const FB_MAX: i32 = 8191; // Maximum fallback brightness
const FB_MIN: i32 = 222;  // Minimum fallback brightness
const FB_OFF: i32 = 0;    // Screen off

// brightness paths and properties
fn k_min_path() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvbWluX2JyaWdodG5lc3M=") }
fn k_max_path() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvbWF4X2h3X2JyaWdodG5lc3M=") }
fn k_bri_path() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvYnJpZ2h0bmVzcw==") }
fn k_prop_cur() -> String { dx("c3lzLm9wbHVzLm11bHRpYnJpZ2h0bmVzcw==") }
fn k_prop_min() -> String { dx("c3lzLm9wbHVzLm11bHRpYnJpZ2h0bmVzcy5taW4=") }
fn k_prop_pmax() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEubXVsdGlicmlnaHRuZXNzLm1heA==") }
fn k_prop_pmin() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEubXVsdGlicmlnaHRuZXNzLm1pbg==") }
fn k_log_tag() -> String { dx("REFkYXB0") } // "DAdpt"
fn k_cmdline() -> String { dx("L3Byb2MvY21kbGluZQ==") }
fn k_dbg_flag() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEuZGlzcGxheS1kZWJ1Zw==") }

fn dx(s: &str) -> String {
    let bytes = general_purpose::STANDARD
        .decode(s)
        .unwrap_or_else(|_| general_purpose::STANDARD.decode(format!("{}==", s)).unwrap());
    String::from_utf8(bytes).unwrap()
}

// logging
fn lg(l: c_int, m: &str) {
    let t = CString::new(k_log_tag()).unwrap();
    let f = CString::new("%s").unwrap();
    let c = CString::new(m).unwrap();
    unsafe { __android_log_print(l, t.as_ptr(), f.as_ptr(), c.as_ptr()) };
}
fn l_d(m: &str) { lg(LG_D, m); }
fn l_e(m: &str) { lg(LG_E, m); }

// system props looker
fn gp(k: &str) -> Option<String> {
    const PROP_VALUE_MAX: usize = 92;
    let ck = CString::new(k).ok()?;
    let mut b = vec![0u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(ck.as_ptr() as *const u8, b.as_mut_ptr() as *mut u8) };
    if len > 0 {
        let cs = unsafe { CStr::from_ptr(b.as_ptr() as *const c_char) };
        Some(cs.to_string_lossy().into_owned())
    } else {
        None
    }
}
fn gp_i(k: &str) -> Option<i32> { gp(k)?.parse::<i32>().ok() }
fn sp(k: &str, v: &str) -> bool {
    let ck = CString::new(k).ok()?;
    let cv = CString::new(v).ok()?;
    unsafe { __system_property_set(ck.as_ptr(), cv.as_ptr()) == 0 }
}

// read file
fn rf(p: &str) -> Option<i32> { std::fs::read_to_string(p).ok()?.trim().parse().ok() }

// get current os brightness
fn gb() -> i32 { gp("debug.tracing.screen_brightness").and_then(|v| v.split('.').next()?.parse::<i32>().ok()).unwrap_or(FB_MIN) }
fn gs() -> i32 { gp("debug.tracing.screen_state").and_then(|v| v.parse::<i32>().ok()).unwrap_or(2) }

// scale the brightness from os to hw
fn sb(v: i32, h1: i32, h2: i32, i1: i32, i2: i32) -> i32 {
    if h1 >= h2 { return h1.max(0); }
    let i1 = i1.min(i2 - 1);
    let i2 = i2.max(i1 + 1);
    if v <= i1 { return h1; }
    if v >= i2 { return h2; }
    let p = (v - i1) * 100 / (i2 - i1);
    let pv = match p {
        0..=70 => 1 + (149 * p / 70),
        71..=90 => 150 + (104 * (p - 70) / 20),
        91..=100 => 254 + (257 * (p - 90) / 10),
        _ => 511,
    };
    (h1 + (pv * (h2 - h1) / 511)).clamp(h1, h2)
}

// debug mode
fn dbg_on() -> bool {
    let k = CString::new(k_dbg_flag()).unwrap();
    let mut b = [0i8; 92];
    let r = unsafe { __system_property_get(k.as_ptr() as *const u8, b.as_mut_ptr() as *mut u8) };
    if r <= 0 { return false; }
    let cs = unsafe { CStr::from_ptr(b.as_ptr() as *const c_char) };
    cs.to_str().unwrap_or("") == "true"
}

// brightness min and max range
#[derive(Clone, Copy, Debug)]
struct IR { mn: i32, mx: i32, l: bool }
impl IR {
    fn init() -> Self {
        match (gp_i(&k_prop_pmin()), gp_i(&k_prop_pmax())) {
            (Some(a), Some(b)) if a < b => Self { mn: a, mx: b, l: false },
            _ => Self { mn: FB_MIN, mx: FB_MAX, l: false },
        }
    }
    fn rf(&mut self) {
        if self.l { return; }
        if let (Some(rm), Some(rx)) = (gp_i(&k_prop_min()), gp_i(&k_prop_cur())) {
            if rm < rx {
                self.mn = rm;
                self.mx = rx;
                self.l = true;
            }
        }
    }
}

// write scaled brightness
fn wb(fd: i32, v: i32, last: &mut i32) {
    if *last == v { return; }
    let s = v.to_string();
    if let Ok(c) = CString::new(s.as_bytes()) {
        let bytes = c.as_bytes_with_nul();
        let result = unsafe { libc::write(fd, bytes.as_ptr() as *const _, bytes.len()) };
        if result >= 0 { *last = v; }
    }
}

// brightness interface
fn main() {
    let min_p = k_min_path();
    let max_p = k_max_path();
    let bri_p = k_bri_path();
    let h1 = rf(&min_p).unwrap_or(1);
    let h2 = rf(&max_p).unwrap_or(511);
    let mut ir = IR::init();
    ir.rf();

    let file = OpenOptions::new().write(true).open(&bri_p);
    let fd = match file { Ok(f) => f.as_raw_fd(), Err(_) => return };

    let mut last_written_val = -1;
    let mut prev_state = gs();
    let mut prev_raw = gb();

    let init_val = sb(prev_raw, h1, h2, ir.mn, ir.mx);
    wb(fd, init_val, &mut last_written_val);
    let mut last_refresh = Instant::now();

    loop {
        if !ir.l && last_refresh.elapsed() >= Duration::from_secs(5) {
            ir.rf();
            last_refresh = Instant::now();
        }
        let cs = gs();
        let cb = gb();
        if cs != 2 {
            if prev_state == 2 { wb(fd, FB_OFF, &mut last_written_val); }
        } else {
            if prev_state != 2 { sleep(Duration::from_millis(200)); }
            let scaled = sb(cb, h1, h2, ir.mn, ir.mx);
            wb(fd, scaled, &mut last_written_val);
        }
        prev_state = cs;
        prev_raw = cb;
        sleep(Duration::from_millis(100));
    }
}
