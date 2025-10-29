use std::ffi::{CString, CStr};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use std::{thread::sleep};
use std::os::raw::{c_int, c_char, c_uchar};
use std::process::Command;
use std::io;

// android ffi imports
unsafe extern "C" {
    fn __system_property_get(name: *const c_uchar, value: *mut c_uchar) -> c_int;
    fn __system_property_set(name: *const c_uchar, value: *const c_uchar) -> c_int;
    fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

// global constants
const LOG_DEBUG: c_int = 3; // android log prio d
const LOG_ERROR: c_int = 6; // android log prio e
const FALLBACK_MAX: i32 = 8191;
const FALLBACK_MIN: i32 = 222;
const BRIGHTNESS_OFF: i32 = 0;
const OS14_MIN: i32 = 22;
const OS14_MAX: i32 = 5118;

// file paths & property keys
fn min_bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/min_brightness" }
fn max_bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/max_hw_brightness" }
fn bright_path() -> &'static str { "/sys/class/leds/lcd-backlight/brightness" }
fn sys_prop_max() -> &'static str { "sys.oplus.multibrightness" }
fn sys_prop_min() -> &'static str { "sys.oplus.multibrightness.min" }
fn persist_max() -> &'static str { "persist.sys.rianixia.multibrightness.max" }
fn persist_min() -> &'static str { "persist.sys.rianixia.multibrightness.min" }
fn log_tag() -> &'static str { "Xia-DisplayAdaptor" }
fn persist_dbg() -> &'static str { "persist.sys.rianixia.display-debug" } //set true for debug logs
fn oplus_bright_path() -> &'static str { "/data/addon/oplus_display/oplus_brightness" }
fn persist_oplus_min() -> &'static str { "persist.sys.rianixia-display.min" }
fn persist_oplus_max() -> &'static str { "persist.sys.rianixia-display.max" }
fn is_oplus_panel_prop() -> &'static str { "persist.sys.rianixia.is-displaypanel.support" } // add for OS14 and under
fn persist_custom_devmax_prop() -> &'static str { "persist.sys.rianixia.custom.devmax.brightness" } // adjust device max value for scaling

// logging utilities
fn log_write(level: c_int, msg: &str) {
    let tag = CString::new(log_tag()).unwrap();
    let fmt = CString::new("%s").unwrap();
    let c_msg = CString::new(msg).unwrap();
    unsafe { __android_log_print(level, tag.as_ptr(), fmt.as_ptr(), c_msg.as_ptr()) };
}
fn log_d(msg: &str) { log_write(LOG_DEBUG, msg); }
fn log_e(msg: &str) { log_write(LOG_ERROR, msg); }

// system property utilities
fn get_prop(key: &str) -> Option<String> {
    const PROP_VALUE_MAX: usize = 92;
    let c_key = CString::new(key).ok()?;
    let mut buffer = vec![0u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(c_key.as_ptr() as *const u8, buffer.as_mut_ptr() as *mut u8) };
    if len > 0 {
        let c_str = unsafe { CStr::from_ptr(buffer.as_ptr() as *const c_char) };
        Some(c_str.to_string_lossy().into_owned())
    } else { None }
}
fn get_prop_int(key: &str) -> Option<i32> { get_prop(key)?.parse::<i32>().ok() }
fn set_prop(key: &str, val: &str) -> bool {
    let c_key = CString::new(key).ok().unwrap();
    let c_val = CString::new(val).ok().unwrap();
    unsafe { __system_property_set(c_key.as_ptr(), c_val.as_ptr()) == 0 }
}

// panoramic aod check
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
                if dbg { log_d(&format!("[DisplayAdaptor] panoramic_aod_enable result: '{}'", result)); }
                result == "1"
            } else {
                if dbg { log_e(&format!("[DisplayAdaptor] 'settings get' command failed: {}", String::from_utf8_lossy(&out.stderr))); }
                false
            }
        },
        Err(e) => {
            if dbg { log_e(&format!("[DisplayAdaptor] Failed to execute 'settings' command: {}", e)); }
            false
        }
    }
}

// mode checks
fn is_oplus_panel_mode() -> bool {
    get_prop(is_oplus_panel_prop()).as_deref() == Some("true")
}
fn is_float_mode() -> bool {
    get_prop("persist.sys.rianixia.brightness.isfloat").as_deref() == Some("true")
}

// file & property readers
fn read_file_int(path: &str) -> Option<i32> {
    if let Ok(content) = std::fs::read_to_string(path) {
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

// brightness property getter
fn get_prop_brightness(range: &BrightnessRange, is_float: bool) -> i32 {
    if is_float {
        if let Some(val_str) = get_prop("debug.tracing.screen_brightness") {
            if let Ok(f) = val_str.parse::<f32>() {
                if f == 0.0 { return -1; } // skip
                let f = f.clamp(0.0, 1.0);
                return (range.min as f32 + f * (range.max - range.min) as f32).round() as i32;
            }
        }
        FALLBACK_MIN
    } else {
        let val = get_prop("debug.tracing.screen_brightness")
            .and_then(|v| v.split('.').next()?.parse::<i32>().ok());
        match val {
            Some(0) => -1, // if val is 0, return -1 to skip write
            Some(v) => v,
            None => FALLBACK_MIN
        }
    }
}

// screen state getter
fn get_screen_state() -> i32 {
    // screen_state values from debug.tracing.screen_state:
    // 1: OFF
    // 2: ON
    // 3: DOZE (AOD)
    // 4: DOZE_SUSPEND (AOD DIMMED)
    get_prop("debug.tracing.screen_state").and_then(|v| v.parse::<i32>().ok()).unwrap_or(2)
}

// brightness scaling curves
fn scale_brightness_curved(val: i32, hw_min: i32, hw_max: i32, input_min: i32, input_max: i32) -> i32 {
    if hw_min >= hw_max { return hw_min.max(0); }
    let input_min = input_min.min(input_max - 1);
    let input_max = input_max.max(input_min + 1);
    if val <= input_min { return hw_min; }
    if val >= input_max { return hw_max; }
    let percent = (val - input_min) * 100 / (input_max - input_min);
    let scaled_percent = match percent {
        0..=70 => 1 + (56 * percent / 70),
        71..=90 => 57 + (197 * (percent - 70) / 20),
        91..=100 => 254 + (257 * (percent - 90) / 10),
        _ => 511,
    };
    (hw_min + (scaled_percent * (hw_max - hw_min) / 511)).clamp(hw_min, hw_max)
}

fn scale_brightness_linear(val: i32, hw_min: i32, hw_max: i32, input_min: i32, input_max: i32) -> i32 {
    if input_min >= input_max || hw_min >= hw_max { return hw_min.max(0); }
    let clamped_v = val.clamp(input_min, input_max);
    let scaled = hw_min as i64 + ((clamped_v - input_min) as i64 * (hw_max - hw_min) as i64 / (input_max - input_min) as i64);
    scaled as i32
}

// debug check
fn dbg_on() -> bool {
    get_prop(persist_dbg()).as_deref() == Some("true")
}

// hardware brightness getter
fn get_max_brightness(dbg: bool) -> i32 {
    if let Some(custom_max) = get_prop_int(persist_custom_devmax_prop()) {
        if custom_max > 0 {
            if dbg { log_d(&format!("[DisplayAdaptor] Using custom devmax brightness: {}", custom_max)); }
            return custom_max;
        }
    }
    let max_from_path = read_file_int(max_bright_path()).unwrap_or(511);
    if dbg { log_d(&format!("[DisplayAdaptor] Using devmax brightness from path: {}", max_from_path)); }
    max_from_path
}

// brightness range struct
#[derive(Clone, Copy, Debug)]
struct BrightnessRange { min: i32, max: i32, locked: bool }
impl BrightnessRange {
    fn init() -> Self {
        let s = match (get_prop_int(persist_min()), get_prop_int(persist_max())) {
            (Some(a), Some(b)) if a < b => Self { min: a, max: b, locked: false },
            _ => Self { min: FALLBACK_MIN, max: FALLBACK_MAX, locked: false },
        };
        if dbg_on() { log_d(&format!("[BrightnessRange] Initialized with range: min={}, max={}", s.min, s.max)); }
        s
    }

    fn refresh_range(&mut self) {
        if self.locked { return; }
        let pmin = get_prop_int(persist_min());
        let pmax = get_prop_int(persist_max());
        let rmin = get_prop_int(sys_prop_min());
        let rmax = get_prop_int(sys_prop_max());

        if let (Some(rm), Some(rx)) = (rmin, rmax) {
            if rm < rx {
                self.min = rm;
                self.max = rx;
                if pmin != Some(rm) { set_prop(persist_min(), &rm.to_string()); }
                if pmax != Some(rx) { set_prop(persist_max(), &rx.to_string()); }
                self.locked = true;
            }
        } else if let (Some(a), Some(b)) = (pmin, pmax) {
            if a < b { self.min = a; self.max = b; }
        } else {
            self.min = FALLBACK_MIN;
            self.max = FALLBACK_MAX;
        }
        if self.min >= self.max { self.min = FALLBACK_MIN; self.max = FALLBACK_MAX; }
    }
}

// main dispatcher
fn main() {
    if is_oplus_panel_mode() {
        run_oplus_panel_mode();
    } else {
        run_default_mode();
    }
}

// DisplayPanel mode (os14 and under)
fn run_oplus_panel_mode() {
    let dbg = dbg_on();
    if dbg { log_d("[DisplayAdaptor] Starting in DisplayPanel Mode..."); }

    let oplus_path_str = oplus_bright_path();
    if !std::path::Path::new(oplus_path_str).exists() {
        if dbg { log_d(&format!("[DisplayPanel Mode] File {} not found, attempting to create it.", oplus_path_str)); }
        loop {
            match std::fs::File::create(oplus_path_str) {
                Ok(_) => {
                    if dbg { log_d(&format!("[DisplayPanel Mode] Successfully created {}.", oplus_path_str)); }
                    break;
                },
                Err(e) => {
                    log_e(&format!("[DisplayPanel Mode] Failed to create {}, retrying in 1s: {}", oplus_path_str, e));
                    sleep(Duration::from_secs(1));
                }
            }
        }
    }
    let hw_min = read_file_int(min_bright_path()).unwrap_or(1);
    let hw_max = get_max_brightness(dbg);

    let input_min = get_prop_int(persist_oplus_min()).unwrap_or(OS14_MIN);
    let input_max = get_prop_int(persist_oplus_max()).unwrap_or(OS14_MAX);
    if dbg { log_d(&format!("[DisplayPanel Mode] Scaling range: {}-{} -> {}-{}", input_min, input_max, hw_min, hw_max)); }

    let file = OpenOptions::new().write(true).open(bright_path());
    let file = match file {
        Ok(f) => f,
        Err(e) => { log_e(&format!("[DisplayPanel Mode] Could not open brightness file: {}", e)); return; },
    };
    let fd = file.as_raw_fd();

    let mut last_val = -1;

    let mut current_val = read_file_int(bright_path()).unwrap_or(hw_min);
    write_brightness(fd, current_val, &mut last_val, dbg);

    loop {
        current_val = read_file_int(bright_path()).unwrap_or(current_val);

        match read_file_int(oplus_bright_path()) {
            Some(oplus_bright) => {
                if oplus_bright == 0 {
                    if current_val != BRIGHTNESS_OFF {
                        current_val = BRIGHTNESS_OFF;
                        write_brightness(fd, current_val, &mut last_val, dbg);
                    }
                } else {
                    let target_val = scale_brightness_linear(oplus_bright, hw_min, hw_max, input_min, input_max);

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

                        write_brightness(fd, current_val, &mut last_val, dbg);
                    }
                }
            },
            None => {
                if dbg { log_e(&format!("[DisplayPanel Mode] Failed to read from {}", oplus_bright_path())); }
            }
        };
        
        sleep(Duration::from_millis(33));
    }
}

// default mode (os 15+)
fn run_default_mode() {
    let dbg = dbg_on();
    if dbg { log_d("[DisplayAdaptor] Starting in Default Mode..."); }
    let is_float = is_float_mode();
    let bright = bright_path();

    let hw_min = read_file_int(min_bright_path()).unwrap_or(1);
    let hw_max = get_max_brightness(dbg);

    let mut range = BrightnessRange::init();
    range.refresh_range();
    if dbg { log_d(&format!("[Default Mode] IR locked: min={}, max={}", range.min, range.max)); }

    let file = OpenOptions::new().write(true).open(bright);
    let file = match file {
        Ok(f) => f,
        Err(e) => { log_e(&format!("[Default Mode] Could not open brightness file: {}", e)); return; },
    };
    let fd = file.as_raw_fd();

    let mut last_val = -1;
    let mut prev_state = get_screen_state();
    let mut prev_bright = get_prop_brightness(&range, is_float);
    if prev_bright == -1 { 
        if dbg { log_d("[DisplayAdaptor] Initial brightness is 0, using fallback."); }
        prev_bright = FALLBACK_MIN; 
    }
    let initial = scale_brightness_curved(prev_bright, hw_min, hw_max, range.min, range.max);
    write_brightness(fd, initial, &mut last_val, dbg);

    loop {
        let cur_state = get_screen_state();
        let raw_bright = get_prop_brightness(&range, is_float);
        let cur_bright = if raw_bright == -1 {
            if dbg { log_d("[DisplayAdaptor] Brightness is 0, ignoring and keeping previous value."); }
            prev_bright // keep old value
        } else {
            raw_bright // use new value
        };

        if cur_bright != prev_bright || cur_state != prev_state {
            let val_to_write = if cur_state == 2 {
                // state is on
                if prev_state != 2 { sleep(Duration::from_millis(100)); }
                scale_brightness_curved(cur_bright, hw_min, hw_max, range.min, range.max)
            } else if cur_state == 0 || cur_state == 1 {
                 // state is 0 (OFF) or 1 (AOD), treat as OFF
                if dbg { log_d(&format!("[DisplayAdaptor] State is {} (OFF), setting brightness 0", cur_state)); }
                BRIGHTNESS_OFF
            } else if cur_state == 3 || cur_state == 4 {
                // state is doze (3) or doze_suspend (4)
                if is_panoramic_aod_enabled(dbg) {
                    if dbg { log_d(&format!("[DisplayAdaptor] State is {} Panoramic AOD is ON, skipping brightness write", cur_state)); }
                    last_val // don't set to 0
                } else {
                    if dbg { log_d(&format!("[DisplayAdaptor] State is {} Panoramic AOD is OFF, setting brightness 0", cur_state)); }
                    BRIGHTNESS_OFF // set to 0
                }
            } else if prev_state == 2 {
                // transitioned from on (2) to some other state
                if is_panoramic_aod_enabled(dbg) {
                    if dbg { log_d("[DisplayAdaptor] Transitioned from ON with Panoramic AOD, deferring brightness 0"); }
                    last_val // don't set to 0
                } else {
                    if dbg { log_d("[DisplayAdaptor] Transitioned from ON without Panoramic AOD, setting brightness 0"); }
                    BRIGHTNESS_OFF // set to 0
                }
            } else {
                // other state, keep last value
                last_val
            };

            if val_to_write != last_val {
                write_brightness(fd, val_to_write, &mut last_val, dbg);
            }
        }

        prev_bright = cur_bright;
        prev_state = cur_state;
        sleep(Duration::from_millis(100));
    }
}

// brightness write function
fn write_brightness(fd: i32, val: i32, last_val: &mut i32, dbg: bool) {
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

