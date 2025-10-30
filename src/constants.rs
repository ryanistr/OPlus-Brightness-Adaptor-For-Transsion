use std::os::raw::c_int;

// global constants
pub(crate) const LOG_DEBUG: c_int = 3; // android log prio d
pub(crate) const LOG_ERROR: c_int = 6; // android log prio e
pub(crate) const FALLBACK_MAX: i32 = 8191; // fallback if max is null
pub(crate) const FALLBACK_MIN: i32 = 222; // fallback if min is null
pub(crate) const BRIGHTNESS_OFF: i32 = 0; // SCREEN OFF
pub(crate) const OS14_MAX: i32 = 5118; // OS14 max fallback
pub(crate) const OS14_MIN: i32 = 22; // OS14 min fallback

