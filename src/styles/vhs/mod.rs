//! # VHS Style Implementation
//!
//! Recreates the distinctive look of VHS video tapes with scan lines, color bleeding,
//! tracking errors, and characteristic noise patterns.

mod effect;

pub use effect::VhsStyle;

// VHS-specific parameter constants
pub const SCANLINE_INTENSITY: &str = "scanline_intensity";
pub const COLOR_BLEEDING: &str = "color_bleeding";
pub const TRACKING_ERROR: &str = "tracking_error";
pub const NOISE_LEVEL: &str = "noise_level";
pub const CHROMA_SHIFT: &str = "chroma_shift";
pub const SATURATION_BOOST: &str = "saturation_boost";