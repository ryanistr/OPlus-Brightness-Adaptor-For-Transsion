use crate::properties::get_prop;
use crate::range::BrightnessRange;
use crate::constants::FALLBACK_MIN;

// brightness property getter
pub(crate) fn get_prop_brightness(range: &BrightnessRange, is_float: bool) -> i32 {
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
pub(crate) fn get_screen_state() -> i32 {
    // screen_state values from debug.tracing.screen_state:
    // 0: OFF
    // 1: OFF (AOD)
    // 2: ON
    // 3: DOZE (AOD)
    // 4: DOZE_SUSPEND (AOD DIMMED)
    get_prop("debug.tracing.screen_state").and_then(|v| v.parse::<i32>().ok()).unwrap_or(2)
}
