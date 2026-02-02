use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::Duration;
use std::{thread::sleep};

use crate::constants::{BRIGHTNESS_OFF, FALLBACK_MIN, OS14_MIN, OS14_MAX};
use crate::logging::{log_d, log_e};
use crate::properties::{get_prop, get_prop_int};
use crate::paths::{
    is_oplus_panel_prop, oplus_bright_path, min_bright_path, persist_oplus_min,
    persist_oplus_max, bright_path, persist_dbg, display_type_prop,
    persist_lux_aod_prop, persist_bright_mode_prop, persist_lux_aod_brightness_prop,
};
use crate::utils::{read_file_int, get_max_brightness, get_min_brightness, is_panoramic_aod_enabled};
use crate::scaling::{scale_brightness_linear, scale_brightness_curved, scale_brightness_custom};
use crate::range::BrightnessRange;
use crate::state::{get_prop_brightness, get_screen_state};
use crate::writer::write_brightness;

// debug check
pub(crate) fn dbg_on() -> bool {
    get_prop(persist_dbg()).as_deref() == Some("true")
}

// mode checks
pub(crate) fn is_oplus_panel_mode() -> bool {
    get_prop(is_oplus_panel_prop()).as_deref() == Some("true")
}
pub(crate) fn is_float_mode() -> bool {
    get_prop("persist.sys.rianixia.brightness.isfloat").as_deref() == Some("true")
}
pub(crate) fn is_ips_mode() -> bool {
    get_prop(display_type_prop()).as_deref() == Some("IPS")
}
pub(crate) fn is_lux_aod_mode() -> bool {
    get_prop(persist_lux_aod_prop()).as_deref() == Some("true")
}

// 0 = Curved, 1 = Linear, 2 = Custom
pub(crate) fn get_brightness_mode() -> i32 {
    get_prop_int(persist_bright_mode_prop()).unwrap_or(0)
}

// main dispatcher
pub fn run() {
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
    let hw_min = get_min_brightness(dbg);
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
                    let mode = get_brightness_mode();
                    let target_val = match mode {
                        1 => scale_brightness_linear(oplus_bright, hw_min, hw_max, input_min, input_max),
                        2 => scale_brightness_custom(oplus_bright, hw_min, hw_max, input_min, input_max),
                        _ => scale_brightness_curved(oplus_bright, hw_min, hw_max, input_min, input_max),
                    };

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
    let mode = get_brightness_mode();
    let is_lux_aod = is_lux_aod_mode();
    
    if dbg { 
        let mode_str = match mode { 1 => "Linear", 2 => "Custom", _ => "Curved" };
        log_d(&format!("[Default Mode] Mode: {}, Lux AOD: {}", mode_str, is_lux_aod)); 
    }

    let bright = bright_path();

    let hw_min = get_min_brightness(dbg);
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
    
    let initial = match mode {
        1 => scale_brightness_linear(prev_bright, hw_min, hw_max, range.min, range.max),
        2 => scale_brightness_custom(prev_bright, hw_min, hw_max, range.min, range.max),
        _ => scale_brightness_curved(prev_bright, hw_min, hw_max, range.min, range.max),
    };
    write_brightness(fd, initial, &mut last_val, dbg);

    let is_ips = is_ips_mode();
    if dbg { log_d(&format!("[Default Mode] IPS Mode: {}", is_ips)); }

    loop {
        let cur_state = get_screen_state();
        let raw_bright = get_prop_brightness(&range, is_float);
        let cur_bright = if raw_bright == -1 {
            if dbg { log_d("[DisplayAdaptor] Brightness is 0, ignoring and keeping previous value."); }
            prev_bright // keep old value
        } else {
            raw_bright // use new value
        };

        let current_mode = get_brightness_mode();

        if cur_bright != prev_bright || cur_state != prev_state {
            let val_to_write = if cur_state == 2 {
                if prev_state != 2 { sleep(Duration::from_millis(100)); }
                match current_mode {
                    1 => scale_brightness_linear(cur_bright, hw_min, hw_max, range.min, range.max),
                    2 => scale_brightness_custom(cur_bright, hw_min, hw_max, range.min, range.max),
                    _ => scale_brightness_curved(cur_bright, hw_min, hw_max, range.min, range.max),
                }
            } else if is_ips {
                // IPS mode (no AOD)
                if dbg { log_d(&format!("[DisplayAdaptor] IPS Mode: State is {} (OFF), setting brightness 0", cur_state)); }
                BRIGHTNESS_OFF
            } else {
                // AMOLED / Default
                if cur_state == 0 || cur_state == 1 {
                     // state is 0 (OFF) or 1 (AOD), treat as OFF
                    if dbg { log_d(&format!("[DisplayAdaptor] State is {} (OFF), setting brightness 0", cur_state)); }
                    BRIGHTNESS_OFF
                } else if cur_state == 3 || cur_state == 4 {
                    // state is doze (3) or doze_suspend (4)
                    let is_panoramic = is_panoramic_aod_enabled(dbg);
                    
                    if is_lux_aod && is_panoramic {
                         // Specific case: Lux AOD ON + Panoramic ON
                         // Check if prop is set
                         if let Some(target_lux) = get_prop_int(persist_lux_aod_brightness_prop()) {
                            if target_lux > 0 {
                                if dbg { log_d(&format!("[DisplayAdaptor] Lux+Panoramic AOD active. Forcing brightness: {}", target_lux)); }
                                target_lux
                            } else {
                                // Prop empty or 0, fallback to standard logic
                                if dbg { log_d("[DisplayAdaptor] Lux+Panoramic AOD active but prop empty/0. Maintaining last value."); }
                                last_val
                            }
                         } else {
                             last_val
                         }
                    } else if cur_state == 3 && is_lux_aod {
                        let raw_prop = get_prop("debug.tracing.screen_brightness").unwrap_or_default();
                        if raw_prop.trim() == "2937.773" {
                            let lux_val = get_prop_int(persist_lux_aod_brightness_prop()).unwrap_or(1);
                            if dbg { log_d(&format!("[DisplayAdaptor] Lux AOD: Detected, forcing brightness to {}", lux_val)); }
                            lux_val
                        } else {
                            if dbg { log_d(&format!("[DisplayAdaptor] State is 3 (Doze) & Lux AOD ON: Updating brightness: {}", cur_bright)); }
                            match current_mode {
                                1 => scale_brightness_linear(cur_bright, hw_min, hw_max, range.min, range.max),
                                2 => scale_brightness_custom(cur_bright, hw_min, hw_max, range.min, range.max),
                                _ => scale_brightness_curved(cur_bright, hw_min, hw_max, range.min, range.max),
                            }
                        }
                    } else if is_panoramic {
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
                }
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