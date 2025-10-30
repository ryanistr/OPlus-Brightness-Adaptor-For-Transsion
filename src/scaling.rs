// brightness scaling curves
pub(crate) fn scale_brightness_curved(val: i32, hw_min: i32, hw_max: i32, input_min: i32, input_max: i32) -> i32 {
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

pub(crate) fn scale_brightness_linear(val: i32, hw_min: i32, hw_max: i32, input_min: i32, input_max: i32) -> i32 {
    if input_min >= input_max || hw_min >= hw_max { return hw_min.max(0); }
    let clamped_v = val.clamp(input_min, input_max);
    let scaled = hw_min as i64 + ((clamped_v - input_min) as i64 * (hw_max - hw_min) as i64 / (input_max - input_min) as i64);
    scaled as i32
}
