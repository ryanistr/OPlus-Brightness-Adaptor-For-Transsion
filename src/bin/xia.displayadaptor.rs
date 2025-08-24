use std::ffi::{CString, CStr};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use std::{thread::sleep};
use std::os::raw::{c_int, c_char, c_uchar};
use base64::{engine::general_purpose, Engine};

// === Android system property and logging bindings ===
unsafe extern "C" {
    fn __system_property_get(name: *const c_uchar, value: *mut c_uchar) -> c_int;
    fn __system_property_set(name: *const c_uchar, value: *const c_uchar) -> c_int;
    fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

// === Log levels ===
const L_A: c_int = 3; // Debug
const L_B: c_int = 6; // Error

// === Default brightness values ===
const F_X: i32 = 8191; // Maximum fallback brightness
const F_Y: i32 = 222;  // Minimum fallback brightness
const F_Z: i32 = 0;    // Screen off

// === Encoded system paths / property keys ===
#[allow(dead_code)]
fn d1() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvbWluX2JyaWdodG5lc3M=") } // min_brightness path
#[allow(dead_code)]
fn d2() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvbWF4X2h3X2JyaWdodG5lc3M=") } // max_brightness path
#[allow(dead_code)]
fn d3() -> String { dx("L3N5cy9jbGFzcy9sZWRzL2xjZC1iYWNrbGlnaHQvYnJpZ2h0bmVzcw==") } // brightness path
#[allow(dead_code)]
fn d4() -> String { dx("c3lzLm9wbHVzLm11bHRpYnJpZ2h0bmVzcw==") } // system prop
#[allow(dead_code)]
fn d5() -> String { dx("c3lzLm9wbHVzLm11bHRpYnJpZ2h0bmVzcy5taW4=") } // system prop
#[allow(dead_code)]
fn d6() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEubXVsdGlicmlnaHRuZXNzLm1heA==") } // persist property: max brightness
#[allow(dead_code)]
fn d7() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEubXVsdGlicmlnaHRuZXNzLm1pbg==") } // persist property: min brightness
#[allow(dead_code)]
fn d8() -> String { dx("cmlhbml4aWFEaXNwbGF5QWRhcHRvcg==") } // log tag
#[allow(dead_code)]
fn d9() -> String { dx("L3Byb2MvY21kbGluZQ==") } // Kernel command line
#[allow(dead_code)]
fn d10() -> String { dx("cGVyc2lzdC5zeXMucmlhbml4aWEuZGlzcGxheS1kZWJ1Zw==") } // persist property: display debug

// === Base64 decoder helper ===
fn dx(s: &str) -> String {
    let bytes = general_purpose::STANDARD.decode(s).unwrap_or_else(|_| general_purpose::STANDARD.decode(format!("{}==", s)).unwrap());
    String::from_utf8(bytes).unwrap()
}


// === Logging helpers ===
fn lg(l: c_int, m: &str) {
    let t = CString::new(d8()).unwrap();
    let f = CString::new("%s").unwrap();
    let c = CString::new(m).unwrap();
    unsafe { __android_log_print(l, t.as_ptr(), f.as_ptr(), c.as_ptr()) };
}
fn ld(m: &str) { lg(L_A, m); } // Debug log
fn le(m: &str) { lg(L_B, m); } // Error log

// === System property helpers ===
fn gp(k: &str) -> Option<String> {
    // Android's max property value length is 92 characters.
    const PROP_VALUE_MAX: usize = 92;

    let ck = match CString::new(k) {
        Ok(c) => c,
        Err(_) => {
            // Describes the problem (invalid key) not the function that failed.
            ld(&format!("[PROP] Invalid character in key, cannot create CString: '{}'", k));
            return None;
        }
    };

    let mut b = vec![0u8; PROP_VALUE_MAX];
    let len = unsafe { __system_property_get(ck.as_ptr() as *const u8, b.as_mut_ptr() as *mut u8) };

    if len > 0 {
        let val_len = len as usize;
        let cs = unsafe { CStr::from_ptr(b.as_ptr() as *const std::os::raw::c_char) };
        let val = cs.to_string_lossy().into_owned();

        if dbg_on() {
            // Consolidated log: shows key, value, and length in one go.
            ld(&format!("[PROP] Get: '{}' -> '{}' (len={})", k, val, val_len));

            // Detail: Adds a specific warning if the value might be truncated.
            if val_len == PROP_VALUE_MAX - 1 {
                ld(&format!("[PROP] WARN: Value for '{}' may be truncated (max length reached)", k));
            }
        }
        Some(val)
    } else {
        if dbg_on() {
            // Clear "not found" message.
            ld(&format!("[PROP] Get: '{}' -> Not found", k));
        }
        None
    }
}

fn gp_i(k: &str) -> Option<i32> { gp(k)?.parse::<i32>().ok() }
fn sp(k: &str, v: &str) -> bool {
    let ck = CString::new(k).ok().unwrap();
    let cv = CString::new(v).ok().unwrap();
    unsafe { __system_property_set(ck.as_ptr(), cv.as_ptr()) == 0 }
}

// === File reader helper ===
fn rf(p: &str) -> Option<i32> { std::fs::read_to_string(p).ok()?.trim().parse().ok() }

// === Get brightness / state from system properties ===
fn gb() -> i32 { gp("debug.tracing.screen_brightness").and_then(|v| v.split('.').next()?.parse::<i32>().ok()).unwrap_or(F_Y) }
fn gs() -> i32 { gp("debug.tracing.screen_state").and_then(|v| v.parse::<i32>().ok()).unwrap_or(2) }

// === Brightness scaling logic ===
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

// === Debug mode flag ===
fn dbg_on() -> bool {
    let k = CString::new(d10()).unwrap();
    let mut b = [0i8; 92];
    let r = unsafe { __system_property_get(k.as_ptr() as *const u8, b.as_mut_ptr() as *mut u8) };
    if r <= 0 { return false; }
    let cs = unsafe { CStr::from_ptr(b.as_ptr() as *const c_char) };
    cs.to_str().unwrap_or("") == "true"
}

// === Brightness range struct ===
#[derive(Clone, Copy, Debug)]
struct IR { mn: i32, mx: i32, l: bool }
impl IR {
    // init() function remains the same.
    fn init() -> Self {
        let s = match (gp_i(&d7()), gp_i(&d6())) {
            (Some(a), Some(b)) if a < b => Self { mn: a, mx: b, l: false },
            _ => Self { mn: F_Y, mx: F_X, l: false },
        };
        if dbg_on() {
            ld(&format!("[IR] Initialized with range: min={}, max={}", s.mn, s.mx));
        }
        s
    }

    fn rf(&mut self) {
        if self.l { return; }

        let dbg = dbg_on();
        if dbg { ld("[IR] Refreshing min/max range..."); }

        // 1. Read all sources of truth upfront.
        let pmin = gp_i(&d7());
        let pmax = gp_i(&d6());
        let rmin = gp_i(&d5());
        let rmax = gp_i(&d4());

        if dbg {
            ld(&format!("[IR] Reading persisted range: min={:?}, max={:?}", pmin, pmax));
            ld(&format!("[IR] Reading system range:   min={:?}, max={:?}", rmin, rmax));
        }

        // 2. The system range is the "source of truth". Use it to decide and verify.
        if let (Some(rm), Some(rx)) = (rmin, rmax) {
            if rm < rx {
                // The source of truth is valid. Now check if persisted values match.
                if pmin == Some(rm) && pmax == Some(rx) {
                    // "Golden path": Persisted values exist and are correct.
                    self.mn = rm; // or pmin.unwrap(), they are the same
                    self.mx = rx;
                    if dbg { ld(&format!("[IR] OK. Persisted range matches system. Using: min={}, max={}", self.mn, self.mx)); }
                } else {
                    // Mismatch detected! Persisted values are wrong/outdated.
                    if dbg { ld(&format!("[IR] Mismatch detected. Prioritizing system values.")); }
                    
                    // USE the correct system values immediately.
                    self.mn = rm;
                    self.mx = rx;

                    // CORRECT the persisted values for the next run.
                    if pmin != Some(rm) {
                        sp(&d7(), &rm.to_string());
                        if dbg { ld(&format!("[IR] Correction: Persisted min ({:?}) differs from system ({}). Updating.", pmin, rm)); }
                    }
                    if pmax != Some(rx) {
                        sp(&d6(), &rx.to_string());
                        if dbg { ld(&format!("[IR] Correction: Persisted max ({:?}) differs from system ({}). Updating.", pmax, rx)); }
                    }
                }
                self.l = true; // Lock after verification is complete.
                if dbg { ld(&format!("[IR] Verification complete. Range is now locked: min={}, max={}", self.mn, self.mx)); }
            }
        } else {
            // 3. Fallback: The "source of truth" is invalid. We can't verify.
            // Trust persisted values if they are valid, otherwise use defaults.
            if dbg { ld("[IR] System range invalid. Cannot verify. Falling back..."); }
            if let (Some(a), Some(b)) = (pmin, pmax) {
                if a < b {
                    self.mn = a;
                    self.mx = b;
                    if dbg { ld(&format!("[IR] Using unverified persisted range: min={}, max={}", self.mn, self.mx)); }
                }
            } else {
                self.mn = F_Y;
                self.mx = F_X;
                if dbg { ld("[IR] All sources invalid. Using default constants."); }
            }
        }

        // 4. Final safety check on whatever values were determined.
        if self.mn >= self.mx {
            self.mn = F_Y;
            self.mx = F_X;
            if dbg { ld(&format!("[IR] SAFETY CHECK: Final values were invalid. Resetting to defaults: min={}, max={}", self.mn, self.mx)); }
        }
    }
}
// === Main program loop ===
fn main() {
    let dbg = dbg_on();
    if dbg { ld("[DisplayAdaptor] Debug mode enabled. Service starting..."); }

    // Decode paths once for cleaner logs
    let min_bright_path = d1();
    let max_bright_path = d2();
    let bright_path = d3();

    // Load min/max hardware brightness from sysfs
    let h1 = rf(&min_bright_path).unwrap_or_else(|| {
        if dbg { le(&format!("[DisplayAdaptor] Could not read min brightness from '{}'. Falling back to 1.", min_bright_path)); }
        1
    });
    let h2 = rf(&max_bright_path).unwrap_or_else(|| {
        if dbg { le(&format!("[DisplayAdaptor] Could not read max brightness from '{}'. Falling back to 511.", max_bright_path)); }
        511
    });

    if dbg { ld(&format!("[DisplayAdaptor] Using hardware brightness range: {} to {}", h1, h2)); }

    // Initialize InputRange (IR) - it has its own logging
    let mut ir = IR::init();
    ir.rf();

    // Open brightness file
    let file = OpenOptions::new().write(true).open(&bright_path);
    let fd = match file {
        Ok(f) => {
            if dbg { ld(&format!("[DisplayAdaptor] Opened brightness control file: '{}'", bright_path)); }
            f.as_raw_fd()
        },
        Err(e) => {
            le(&format!("[DisplayAdaptor] FATAL: Could not open brightness control file '{}': {}", bright_path, e));
            return;
        }
    };

    // Track previous states to reduce log spam
    let mut last_written_val = -1;
    let mut prev_screen_state = gs(); // Initialize with current state
    let mut prev_raw_brightness = gb();

    // Compute and write initial brightness
    let initial_val = sb(prev_raw_brightness, h1, h2, ir.mn, ir.mx);
    if dbg { ld(&format!("[DisplayAdaptor] Initial raw brightness={}, scaled to {}", prev_raw_brightness, initial_val)); }
    wb(fd, initial_val, &mut last_written_val, dbg);

    let mut last_refresh = Instant::now();

    // === Main loop ===
    loop {
        // Refresh InputRange every 5 seconds if not locked
        if !ir.l && last_refresh.elapsed() >= Duration::from_secs(5) {
            ir.rf(); // IR has its own detailed logging
            last_refresh = Instant::now();
        }

        // Read screen state and current brightness
        let current_screen_state = gs();
        let current_raw_brightness = gb();

        // LOG ONLY ON CHANGE to prevent spam
        if dbg {
            if current_screen_state != prev_screen_state {
                ld(&format!("[DisplayAdaptor] Screen state changed: {} -> {}", prev_screen_state, current_screen_state));
            }
            if current_raw_brightness != prev_raw_brightness {
                ld(&format!("[DisplayAdaptor] Raw brightness changed: {} -> {}", prev_raw_brightness, current_raw_brightness));
            }
        }

        // Handle screen state logic
        if current_screen_state != 2 { // Screen is off or dozing
            if prev_screen_state == 2 {
                if dbg { ld("[DisplayAdaptor] Action: Screen turned off. Setting brightness to 0."); }
                wb(fd, F_Z, &mut last_written_val, dbg);
            }
        } else { // Screen is on
            if prev_screen_state != 2 {
                if dbg { ld("[DisplayAdaptor] Action: Screen waking. Pausing before brightness update."); }
                sleep(Duration::from_millis(200));
            }
            let scaled_val = sb(current_raw_brightness, h1, h2, ir.mn, ir.mx);
            wb(fd, scaled_val, &mut last_written_val, dbg);
        }

        // Update previous state trackers
        prev_screen_state = current_screen_state;
        prev_raw_brightness = current_raw_brightness;
        
        sleep(Duration::from_millis(100)); // fast polling
    }
}

// === Write brightness to file with detailed logging ===
fn wb(fd: i32, v: i32, last: &mut i32, dbg: bool) {
    if *last == v {
        return;
    }

    if dbg { ld(&format!("[DisplaySvc] Writing brightness: {} -> {}", *last, v)); }

    // Convert the value to a C-style string for the system call.
    let s = v.to_string();
    let c_string = match std::ffi::CString::new(s.as_bytes()) {
        Ok(c) => c,
        Err(_) => {
            if dbg { le("[DisplaySvc] Write failed: could not create CString"); }
            return;
        }
    };

    // Perform a direct, low-level write using the raw file descriptor.
    let bytes = c_string.as_bytes_with_nul();
    let result = unsafe {
        // This is the C `write` function. It doesn't involve Rust's File ownership.
        libc::write(fd, bytes.as_ptr() as *const std::ffi::c_void, bytes.len())
    };

    if result < 0 {
        // libc::write returns -1 on error.
        if dbg { le(&format!("[DisplaySvc] Write failed for value {}: {}", v, std::io::Error::last_os_error())); }
    } else {
        *last = v; // Update last value only on successful write.
    }
}