use std::ffi::{CString, CStr};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use std::{thread::sleep};
use std::os::raw::{c_int, c_char, c_uchar};
use std::process::Command; // patch: import command

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
const OS14_MIN: i32 = 22;
const OS14_MAX: i32 = 5118;

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
fn oplus_bright_path() -> &'static str { "/data/addon/oplus_display/oplus_brightness" } // add for OS14 support
fn persist_oplus_min() -> &'static str { "persist.sys.rianixia-display.min" }
fn persist_oplus_max() -> &'static str { "persist.sys.rianixia-display.max" }
fn is_oplus_panel_prop() -> &'static str { "persist.sys.rianixia.is-displaypanel.support" }
fn persist_custom_devmax_prop() -> &'static str { "persist.sys.rianixia.custom.devmax.brightness" }

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

// patch: add function to check panoramic aod setting
fn is_panoramic_aod_enabled(dbg: bool) -> bool {
    let output = Command::new("settings")
                             .arg("get")
                             .arg("secure")
                             .arg("panoramic_aod_enable")
                             .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                let result = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if dbg { ld(&format!("[DisplayAdaptor] panoramic_aod_enable result: '{}'", result)); }
                result == "1"
            } else {
                if dbg { le(&format!("[DisplayAdaptor] 'settings get' command failed: {}", String::from_utf8_lossy(&out.stderr))); }
                false // failed to get setting, assume disabled
            }
        },
        Err(e) => {
            if dbg { le(&format!("[DisplayAdaptor] Failed to execute 'settings' command: {}", e)); }
            false // failed to run command, assume disabled
        }
    }
}
// end patch

fn is_oplus_panel_mode() -> bool {
    gp(is_oplus_panel_prop()).as_deref() == Some("true")
}
fn is_float_mode() -> bool {
    gp("persist.sys.rianixia.brightness.isfloat").as_deref() == Some("true")
}

// Get current brightness and screenstate
fn rf(p: &str) -> Option<i32> {
    if let Ok(content) = std::fs::read_to_string(p) {
        let numeric_part: String = content
            .trim()
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect();
        numeric_part.parse().ok()
    } else {
        None
    }
}
fn gb(ir: &IR, is_float: bool) -> i32 {
    if is_float {
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

// Brightness scaling
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

fn sb_linear(v: i32, h1: i32, h2: i32, i1: i32, i2: i32) -> i32 {
    if i1 >= i2 || h1 >= h2 { return h1.max(0); }
    let clamped_v = v.clamp(i1, i2);
    let scaled = h1 as i64 + ((clamped_v - i1) as i64 * (h2 - h1) as i64 / (i2 - i1) as i64);
    scaled as i32
}

fn dbg_on() -> bool {
    gp(persist_dbg()).as_deref() == Some("true")
}

// Helper to get max hardware brightness
fn get_max_brightness(dbg: bool) -> i32 {
    if let Some(custom_max) = gp_i(persist_custom_devmax_prop()) {
        if custom_max > 0 {
            if dbg { ld(&format!("[DisplayAdaptor] Using custom devmax brightness: {}", custom_max)); }
            return custom_max;
        }
    }
    let max_from_path = rf(max_bright_path()).unwrap_or(511);
    if dbg { ld(&format!("[DisplayAdaptor] Using devmax brightness from path: {}", max_from_path)); }
    max_from_path
}

// set brightness min max range for float mode
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

// Main dispatcher
fn main() {
    if is_oplus_panel_mode() {
        run_oplus_panel_mode();
    } else {
        run_legacy_mode();
    }
}

// Add for OS14 with DisplayPanel
fn run_oplus_panel_mode() {
    let dbg = dbg_on();
    if dbg { ld("[DisplayAdaptor] Starting in OPlus Panel Mode..."); }

    let oplus_path_str = oplus_bright_path();
    if !std::path::Path::new(oplus_path_str).exists() {
        if dbg { ld(&format!("[OPlus Mode] File {} not found, attempting to create it.", oplus_path_str)); }
        loop {
            match std::fs::File::create(oplus_path_str) {
                Ok(_) => {
                    if dbg { ld(&format!("[OPlus Mode] Successfully created {}.", oplus_path_str)); }
                    break;
                },
                Err(e) => {
                    le(&format!("[OPlus Mode] Failed to create {}, retrying in 1s: {}", oplus_path_str, e));
                    sleep(Duration::from_secs(1));
                }
            }
        }
    }

    let h1 = rf(min_bright_path()).unwrap_or(1);
    let h2 = get_max_brightness(dbg);

    let i1 = gp_i(persist_oplus_min()).unwrap_or(OS14_MIN);
    let i2 = gp_i(persist_oplus_max()).unwrap_or(OS14_MAX);
    if dbg { ld(&format!("[OPlus Mode] Scaling range: {}-{} -> {}-{}", i1, i2, h1, h2)); }

    let file = OpenOptions::new().write(true).open(bright_path());
    let file = match file {
        Ok(f) => f,
        Err(e) => { le(&format!("[OPlus Mode] Could not open brightness file: {}", e)); return; },
    };
    let fd = file.as_raw_fd();

    let mut last_val = -1;

    let mut current_val = rf(bright_path()).unwrap_or(h1);
    wb(fd, current_val, &mut last_val, dbg);

    loop {
        current_val = rf(bright_path()).unwrap_or(current_val);

        match rf(oplus_bright_path()) {
            Some(oplus_bright) => {
                if oplus_bright == 0 {
                    if current_val != F_Z {
                        current_val = F_Z;
                        wb(fd, current_val, &mut last_val, dbg);
                    }
                } else {
                    let target_val = sb_linear(oplus_bright, h1, h2, i1, i2);

                    if current_val != target_val {
                        let diff = target_val - current_val;
                        let mut step = diff / 4;
                        if diff != 0 && step == 0 {
                            step = if diff > 0 { 1 } else { -1 };
                        }
                        
                        current_val += step;
                        
                        if (step > 0 && current_val > target_val) || (step < 0 && current_val < target_val) {
                            current_val = target_val;
                        }

                        wb(fd, current_val, &mut last_val, dbg);
                    }
                }
            },
            None => {
                if dbg { le(&format!("[OPlus Mode] Failed to read from {}", oplus_bright_path())); }
            }
        };
        
        sleep(Duration::from_millis(33));
    }
}

fn run_legacy_mode() {
    let dbg = dbg_on();
    if dbg { ld("[DisplayAdaptor] Starting in Legacy Mode..."); }
    let is_float = is_float_mode();
    let bright = bright_path();

    let h1 = rf(min_bright_path()).unwrap_or(1);
    let h2 = get_max_brightness(dbg);

    let mut ir = IR::init();
    ir.rf();
    if dbg { ld(&format!("[Legacy Mode] IR locked: min={}, max={}", ir.mn, ir.mx)); }

    let file = OpenOptions::new().write(true).open(bright);
    let file = match file {
        Ok(f) => f,
        Err(e) => { le(&format!("[Legacy Mode] Could not open brightness file: {}", e)); return; },
    };
    let fd = file.as_raw_fd();

    let mut last_val = -1;
    let mut prev_state = gs();
    let mut prev_bright = gb(&ir, is_float);
    let initial = sb(prev_bright, h1, h2, ir.mn, ir.mx);
    wb(fd, initial, &mut last_val, dbg);

    loop {
        let cur_state = gs();
        let cur_bright = gb(&ir, is_float);

        if cur_bright != prev_bright || cur_state != prev_state {
            // patch: modified brightness logic for panoramic aod
            let val_to_write = if cur_state == 2 {
                // state is on
                if prev_state != 2 { sleep(Duration::from_millis(100)); }
                sb(cur_bright, h1, h2, ir.mn, ir.mx)
            } else if cur_state == 0 || cur_state == 1 || cur_state == 3 || cur_state == 4 {
                // state is explicitly off or doze/doze_suspend
                if dbg { ld(&format!("[DisplayAdaptor] State is {}, setting brightness 0", cur_state)); }
                F_Z
            } else if prev_state == 2 {
                // transitioned from on (2) to some other state (not 0, 1, 3, 4)
                // this is the panoramic aod check
                if is_panoramic_aod_enabled(dbg) {
                    if dbg { ld("[DisplayAdaptor] Panoramic AOD enabled, deferring brightness 0"); }
                    last_val // don't set to 0
                } else {
                    if dbg { ld("[DisplayAdaptor] Panoramic AOD disabled, setting brightness 0"); }
                    F_Z // set to 0
                }
            } else {
                // state is not 2, not 0/1/3/4, and did not just transition from 2.
                // keep last value.
                last_val
            };
            // end patch

            if val_to_write != last_val {
                wb(fd, val_to_write, &mut last_val, dbg);
            }
        }

        prev_bright = cur_bright;
        prev_state = cur_state;
        sleep(Duration::from_millis(100));
    }
}

// Write brightness value
fn wb(fd: i32, v: i32, last: &mut i32, dbg: bool) {
    if *last == v {
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

