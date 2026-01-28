// scaling functions

pub(crate) fn scale_brightness_linear(
    val: i32,
    hw_min: i32,
    hw_max: i32,
    input_min: i32,
    input_max: i32,
) -> i32 {
    if val <= input_min { return hw_min; }
    if val >= input_max { return hw_max; }

    let range_input = (input_max - input_min) as f32;
    let range_hw = (hw_max - hw_min) as f32;
    let ratio = (val - input_min) as f32 / range_input;

    (hw_min as f32 + ratio * range_hw).round() as i32
}

pub(crate) fn scale_brightness_curved(
    val: i32,
    hw_min: i32,
    hw_max: i32,
    input_min: i32,
    input_max: i32,
) -> i32 {
    if val <= input_min { return hw_min; }
    if val >= input_max { return hw_max; }

    let range_input = (input_max - input_min) as f32;
    let range_hw = (hw_max - hw_min) as f32;
    let ratio = (val - input_min) as f32 / range_input;

    // Standard Gamma 2.2 approximation (Perceptual -> Linear)
    let gamma: f32 = 2.2; 
    let curve = ratio.powf(gamma);

    (hw_min as f32 + curve * range_hw).round() as i32
}

// Custom % Curve:
// Example
// Min -> 1
// Max -> 511
// @ 75% Input -> 255 Output
pub(crate) fn scale_brightness_custom(
    val: i32,
    hw_min: i32,
    hw_max: i32,
    input_min: i32,
    input_max: i32,
) -> i32 {
    if val <= input_min { return hw_min; }
    if val >= input_max { return hw_max; }

    let range_input = (input_max - input_min) as f32;
    let normalized = (val - input_min) as f32 / range_input;

    let mid_in = 0.75;
    let mid_out = 255.0;
    if normalized <= mid_in {
        let ratio = normalized / mid_in;
        (hw_min as f32 + ratio * (mid_out - hw_min as f32)).round() as i32
    } else {
        let ratio = (normalized - mid_in) / (1.0 - mid_in);
        (mid_out + ratio * (hw_max as f32 - mid_out)).round() as i32
    }
}