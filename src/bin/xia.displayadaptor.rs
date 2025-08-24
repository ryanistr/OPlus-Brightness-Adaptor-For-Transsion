use std::ffi::{CString, CStr};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use std::{thread::sleep};
use std::os::raw::{c_int, c_char, c_uchar};

// import android stuff
unsafe extern "C" {
    fn __system_property_get(name: *const c_uchar, value: *mut c_uchar) -> c_int;
    fn __system_property_set(name: *const c_uchar, value: *const c_uchar) -> c_int;
    fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

// fixed brightness min/max for fallback if dynamic fail for whatever reason
const L_A: c_int = 3;
const L_B: c_int = 6;
const F_X: i32 = 8191;
const F_Y: i32 = 222;
const F_Z: i32 = 0;

// brightness props and path
fn min_bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/min_brightness" } // devmin path
fn max_bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/max_hw_brightness" } // devmax path
fn bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/brightness" } //writeable brightness path
fn sys_prop_max() -> &'static str { "sys.oplus.multibrightness" } //oplus max
fn sys_prop_min() -> &'static str { "sys.oplus.multibrightness.min" } //oplus min
fn persist_max() -> &'static str { "persist.sys.rianixia.multibrightness.max" } //set to persist for fluidity
fn persist_min() -> &'static str { "persist.sys.rianixia.multibrightness.min" }
fn log_tag() -> &'static str { "Xia-DisplayAdaptor" } //logtag
fn persist_dbg() -> &'static str { "persist.sys.rianixia.display-debug" } //prop for logs set to true for logging

// logging utils
fn lg(l: c_int, m: &str) {
    let t = CString::new(log_tag()).unwrap();
    let f = CString::new("%s").unwrap();
    let c = CString::new(m).unwrap();
    unsafe { __android_log_print(l, t.as_ptr(), f.as_ptr(), c.as_ptr()) };
}
fn ld(m: &str) { lg(L_A, m); }
fn le(m: &str) { lg(L_B, m); }

// system props looker
fn gp(k: &str) -> Option<String> {
    const PROP_VALUE_MAX: usize = 92;
    let ck = CString::new(k).ok()?;
    let mut b = vec![0u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(ck.as_ptr() as *const u8, b.as_mut_ptr() as *mut u8) };
    if len > 0 {
        let cs = unsafe { CStr::from_ptr(b.as_ptr() as *const c_char) };
        Some(cs.to_string_lossy().into_owned())
    } else { None }
}
fn gp_i(k: &str) -> Option<i32> { gp(k)?.parse::<i32>().ok() }
fn sp(k: &str, v: &str) -> bool {
    let ck = CString::new(k).ok().unwrap();
    let cv = CString::new(v).ok().unwrap();
    unsafe { __system_property_set(ck.as_ptr(), cv.as_ptr()) == 0 }
}

// add for float support (OS14)
fn is_float_mode() -> bool {
    gp("persist.sys.rianixia.brightness.isfloat").as_deref() == Some("true")
}

// get current brightness and screenstate
fn rf(p: &str) -> Option<i32> { std::fs::read_to_string(p).ok()?.trim().parse().ok() }
fn gb(ir: &IR) -> i32 {
    if is_float_mode() {
        if let Some(val_str) = gp("debug.tracing.screen_brightness") {
            if let Ok(f) = val_str.parse::<f32>() {
                let f = f.clamp(0.0, 1.0);
                return (ir.mn as f32 + f * (ir.mx - ir.mn) as f32).round() as i32;
            }
        }
        F_Y
    } else {
        gp("debug.tracing.screen_brightness")
            .and_then(|v| v.split('.').next()?.parse::<i32>().ok())
            .unwrap_or(F_Y)
    }
}
fn gs() -> i32 { gp("debug.tracing.screen_state").and_then(|v| v.parse::<i32>().ok()).unwrap_or(2) }

// scale brightness
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

fn dbg_on() -> bool {
    gp(persist_dbg()).as_deref() == Some("true")
}

// set brigtness min max range
#[derive(Clone, Copy, Debug)]
struct IR { mn: i32, mx: i32, l: bool }
impl IR {
    fn init() -> Self {
        let s = match (gp_i(persist_min()), gp_i(persist_max())) {
            (Some(a), Some(b)) if a < b => Self { mn: a, mx: b, l: false },
            _ => Self { mn: F_Y, mx: F_X, l: false },
        };
        if dbg_on() { ld(&format!("[IR] Initialized with range: min={}, max={}", s.mn, s.mx)); }
        s
    }

    fn rf(&mut self) {
        if self.l { return; }
        let dbg = dbg_on();
        let pmin = gp_i(persist_min());
        let pmax = gp_i(persist_max());
        let rmin = gp_i(sys_prop_min());
        let rmax = gp_i(sys_prop_max());

        if let (Some(rm), Some(rx)) = (rmin, rmax) {
            if rm < rx {
                self.mn = rm;
                self.mx = rx;
                if pmin != Some(rm) { sp(persist_min(), &rm.to_string()); }
                if pmax != Some(rx) { sp(persist_max(), &rx.to_string()); }
                self.l = true;
            }
        } else if let (Some(a), Some(b)) = (pmin, pmax) {
            if a < b { self.mn = a; self.mx = b; }
        } else {
            self.mn = F_Y;
            self.mx = F_X;
        }
        if self.mn >= self.mx { self.mn = F_Y; self.mx = F_X; }
    }
}

// brightness service
fn main() {
    let dbg = dbg_on();
    if dbg { ld("[DisplayAdaptor] Service starting..."); }

    let min_path = min_bright_path();
    let max_path = max_bright_path();
    let bright = bright_path();

    let h1 = rf(min_path).unwrap_or(1);
    let h2 = rf(max_path).unwrap_or(511);

    let mut ir = IR::init();
    ir.rf(); 
    if dbg { ld(&format!("[DisplayAdaptor] IR locked: min={}, max={}", ir.mn, ir.mx)); }

    let file = OpenOptions::new().write(true).open(bright);
    let fd = match file {
        Ok(f) => f.as_raw_fd(),
        Err(e) => { le(&format!("[DisplayAdaptor] Could not open brightness file: {}", e)); return; },
    };

    let mut last_val = -1;
    let mut prev_state = gs();
    let mut prev_bright = gb(&ir);

    let initial = sb(prev_bright, h1, h2, ir.mn, ir.mx);
    wb(fd, initial, &mut last_val, dbg);

    loop {
        let cur_state = gs();
        let cur_bright = gb(&ir);

        if cur_state != 2 && prev_state == 2 {
            // Screen just turned off
            wb(fd, F_Z, &mut last_val, dbg);
        } else if cur_state == 2 {
            if prev_state != 2 { sleep(Duration::from_millis(200)); }
            let scaled = sb(cur_bright, h1, h2, ir.mn, ir.mx);
            wb(fd, scaled, &mut last_val, dbg);
        }

        prev_state = cur_state;
        prev_bright = cur_bright;
        sleep(Duration::from_millis(100));
    }
}

fn wb(fd: i32, v: i32, last: &mut i32, dbg: bool) {
    if *last == v {
        if dbg { ld(&format!("[DisplayAdaptor] Brightness unchanged, value still {}", v)); }
        return;
    }

    if dbg { ld(&format!("[DisplayAdaptor] Writing brightness: {} -> {}", *last, v)); }

    let s = v.to_string();
    let c_str = match CString::new(s.as_bytes()) { Ok(c) => c, Err(_) => { le("[DisplayAdaptor] Failed to create CString"); return; } };
    let bytes = c_str.as_bytes_with_nul();

    let result = unsafe { libc::write(fd, bytes.as_ptr() as *const _, bytes.len()) };
    if result < 0 {
        if dbg { le(&format!("[DisplayAdaptor] Write failed for value {}: {}", v, std::io::Error::last_os_error())); }
    } else {
        *last = v;
    }
}