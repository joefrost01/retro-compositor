// src/styles/vhs/effect.rs - Enhanced VHS effects

use rand::Rng;

use crate::{
    error::Result,
    styles::{Style, StyleConfig},
    styles::traits::StyleMetadata,
    video::types::Frame,
};

use super::{SCANLINE_INTENSITY, COLOR_BLEEDING, TRACKING_ERROR, NOISE_LEVEL, CHROMA_SHIFT, SATURATION_BOOST};

/// VHS-style video effect implementation with enhanced visual impact
pub struct VhsStyle;

impl VhsStyle {
    pub fn new() -> Self {
        Self
    }

    /// Apply **ENHANCED** scan line effect to the frame
    fn apply_scanlines(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        for y in 0..height {
            // **ENHANCED**: More pronounced scan lines with varying intensity
            let line_intensity = if y % 2 == 0 {
                // Primary scan lines
                1.0 - (intensity * 0.4) // Darker lines
            } else {
                // Secondary lines - slightly dimmed
                1.0 - (intensity * 0.2)
            };

            // **ENHANCED**: Add occasional "thick" scan lines
            let thick_line = (y % 8 == 0) && intensity > 0.5;
            let final_intensity = if thick_line {
                line_intensity * 0.7 // Much darker thick lines
            } else {
                line_intensity
            };

            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);
                pixel[0] = (pixel[0] as f32 * final_intensity) as u8;
                pixel[1] = (pixel[1] as f32 * final_intensity) as u8;
                pixel[2] = (pixel[2] as f32 * final_intensity) as u8;
            }
        }
    }

    /// Apply **ENHANCED** color bleeding effect
    fn apply_color_bleeding(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        let original = frame.clone();

        for y in 0..height {
            for x in 2..width-2 { // Wider sampling for more bleeding
                let current = original.get_pixel(x, y);
                let left1 = original.get_pixel(x-1, y);
                let left2 = original.get_pixel(x-2, y);
                let right1 = original.get_pixel(x+1, y);
                let right2 = original.get_pixel(x+2, y);

                // **ENHANCED**: Stronger bleeding with multiple pixel influence
                let blend_factor = intensity * 0.4; // Increased from 0.2

                let new_pixel = frame.get_pixel_mut(x, y);

                // Red channel bleeds right (stronger effect)
                let red_bleed = (right1[0] as f32 * 0.7 + right2[0] as f32 * 0.3) * blend_factor;
                new_pixel[0] = ((current[0] as f32 * (1.0 - blend_factor)) + red_bleed) as u8;

                // Blue channel bleeds left (stronger effect)
                let blue_bleed = (left1[2] as f32 * 0.7 + left2[2] as f32 * 0.3) * blend_factor;
                new_pixel[2] = ((current[2] as f32 * (1.0 - blend_factor)) + blue_bleed) as u8;

                // Green gets slight chromatic aberration
                let green_shift = ((left1[1] as f32 + right1[1] as f32) * 0.5) * (blend_factor * 0.3);
                new_pixel[1] = ((current[1] as f32 * (1.0 - blend_factor * 0.3)) + green_shift) as u8;
            }
        }
    }

    /// Apply **ENHANCED** tracking errors
    fn apply_tracking_error(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let mut rng = rand::thread_rng();

        // **ENHANCED**: More frequent and varied tracking errors
        for y in 0..height {
            let error_probability = intensity * 0.15; // Increased from 0.05

            if rng.gen::<f32>() < error_probability {
                // **ENHANCED**: Varied displacement amounts
                let displacement = if rng.gen::<f32>() < 0.7 {
                    rng.gen_range(-2..=2) // Small displacements
                } else {
                    rng.gen_range(-8..=8) // Occasional large glitches
                };

                self.displace_scanline(frame, y, displacement);

                // **ENHANCED**: Sometimes affect multiple consecutive lines
                if rng.gen::<f32>() < 0.3 && y < height - 1 {
                    self.displace_scanline(frame, y + 1, displacement / 2);
                }
            }
        }

        // **NEW**: Add occasional "tape stretch" effect
        if intensity > 0.5 && rng.gen::<f32>() < 0.1 {
            let stretch_line = rng.gen_range(0..height);
            self.apply_tape_stretch(frame, stretch_line, intensity);
        }
    }

    /// **NEW**: Apply tape stretch effect
    fn apply_tape_stretch(&self, frame: &mut Frame, line: u32, intensity: f32) {
        let width = frame.width();
        let stretch_factor = 1.0 + (intensity * 0.3); // Up to 30% stretch

        let mut stretched_line = Vec::new();

        // Sample the line with stretching
        for x in 0..width {
            let source_x = (x as f32 / stretch_factor) as u32;
            if source_x < width {
                let pixel = frame.get_pixel(source_x, line);
                stretched_line.extend_from_slice(&pixel);
            } else {
                // Repeat last pixel - fix borrowing issue
                let last_pixel = if stretched_line.len() >= 3 {
                    [
                        stretched_line[stretched_line.len() - 3],
                        stretched_line[stretched_line.len() - 2],
                        stretched_line[stretched_line.len() - 1]
                    ]
                } else {
                    [128, 128, 128] // Gray fallback
                };
                stretched_line.extend_from_slice(&last_pixel);
            }
        }

        // Apply the stretched line back
        for x in 0..width {
            let pixel_idx = x as usize * 3;
            if pixel_idx + 2 < stretched_line.len() {
                let target_pixel = frame.get_pixel_mut(x, line);
                target_pixel[0] = stretched_line[pixel_idx];
                target_pixel[1] = stretched_line[pixel_idx + 1];
                target_pixel[2] = stretched_line[pixel_idx + 2];
            }
        }
    }

    fn displace_scanline(&self, frame: &mut Frame, y: u32, displacement: i32) {
        let width = frame.width() as i32;

        if displacement == 0 { return; }

        let mut line_data = Vec::with_capacity(width as usize * 3);
        for x in 0..width {
            let pixel = frame.get_pixel(x as u32, y);
            line_data.extend_from_slice(&pixel);
        }

        for x in 0..width {
            let source_x = x - displacement;
            if source_x >= 0 && source_x < width {
                let pixel_idx = source_x as usize * 3;
                if pixel_idx + 2 < line_data.len() {
                    let target_pixel = frame.get_pixel_mut(x as u32, y);
                    target_pixel[0] = line_data[pixel_idx];
                    target_pixel[1] = line_data[pixel_idx + 1];
                    target_pixel[2] = line_data[pixel_idx + 2];
                }
            } else {
                // **ENHANCED**: Fill displaced areas with "snow"
                let target_pixel = frame.get_pixel_mut(x as u32, y);
                let noise = rand::thread_rng().gen_range(0..=64);
                target_pixel[0] = noise;
                target_pixel[1] = noise;
                target_pixel[2] = noise;
            }
        }
    }

    /// Apply **ENHANCED** VHS-style noise
    fn apply_noise(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();
        let mut rng = rand::thread_rng();

        // **ENHANCED**: More varied noise patterns
        for y in 0..height {
            for x in 0..width {
                let noise_probability = intensity * 0.08; // Increased from 0.02

                if rng.gen::<f32>() < noise_probability {
                    let pixel = frame.get_pixel_mut(x, y);

                    // **ENHANCED**: Different types of noise
                    let noise_type = rng.gen::<f32>();

                    if noise_type < 0.6 {
                        // Regular grain noise
                        let noise = rng.gen_range(-30..=30);
                        for channel in 0..3 {
                            let new_value = (pixel[channel] as i16 + noise).clamp(0, 255);
                            pixel[channel] = new_value as u8;
                        }
                    } else if noise_type < 0.8 {
                        // "Snow" noise (bright dots)
                        let snow_value = rng.gen_range(200..=255);
                        pixel[0] = snow_value;
                        pixel[1] = snow_value;
                        pixel[2] = snow_value;
                    } else {
                        // "Dropout" noise (dark spots)
                        let dropout = rng.gen_range(0..=40);
                        pixel[0] = dropout;
                        pixel[1] = dropout;
                        pixel[2] = dropout;
                    }
                }
            }
        }

        // **NEW**: Add occasional "noise bands"
        if intensity > 0.6 && rng.gen::<f32>() < 0.2 {
            let band_start = rng.gen_range(0..height);
            let band_height = rng.gen_range(2..=8);
            self.apply_noise_band(frame, band_start, band_height, intensity);
        }
    }

    /// **NEW**: Apply horizontal noise bands
    fn apply_noise_band(&self, frame: &mut Frame, start_y: u32, height: u32, intensity: f32) {
        let width = frame.width();
        let frame_height = frame.height();
        let mut rng = rand::thread_rng();

        for y in start_y..=(start_y + height).min(frame_height - 1) {
            for x in 0..width {
                if rng.gen::<f32>() < intensity * 0.5 {
                    let pixel = frame.get_pixel_mut(x, y);
                    let noise = rng.gen_range(-50..=50);

                    for channel in 0..3 {
                        let new_value = (pixel[channel] as i16 + noise).clamp(0, 255);
                        pixel[channel] = new_value as u8;
                    }
                }
            }
        }
    }

    /// Apply **ENHANCED** chromatic aberration
    fn apply_chroma_shift(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();
        let shift = (intensity * 4.0) as i32; // Increased from 2.0

        if shift == 0 { return; }

        let original = frame.clone();

        for y in 0..height {
            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);

                // **ENHANCED**: More pronounced shifts with varied directions
                // Red channel shifts right
                let red_x = (x as i32 + shift).clamp(0, width as i32 - 1) as u32;
                pixel[0] = original.get_pixel(red_x, y)[0];

                // Blue channel shifts left
                let blue_x = (x as i32 - shift).clamp(0, width as i32 - 1) as u32;
                pixel[2] = original.get_pixel(blue_x, y)[2];

                // **NEW**: Green channel gets slight vertical shift for more realism
                let green_y = if intensity > 0.7 {
                    (y as i32 + shift / 2).clamp(0, height as i32 - 1) as u32
                } else {
                    y
                };
                pixel[1] = original.get_pixel(x, green_y)[1];
            }
        }
    }

    /// Apply **ENHANCED** saturation boost
    fn apply_saturation_boost(&self, frame: &mut Frame, boost: f32) {
        let height = frame.height();
        let width = frame.width();

        for y in 0..height {
            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);

                let r = pixel[0] as f32 / 255.0;
                let g = pixel[1] as f32 / 255.0;
                let b = pixel[2] as f32 / 255.0;

                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                let delta = max - min;

                if delta > 0.0 {
                    // **ENHANCED**: Stronger saturation with VHS color characteristics
                    let saturation_factor = 1.0 + boost * 0.6; // Increased from 0.3
                    let avg = (r + g + b) / 3.0;

                    let mut new_r = avg + (r - avg) * saturation_factor;
                    let mut new_g = avg + (g - avg) * saturation_factor;
                    let mut new_b = avg + (b - avg) * saturation_factor;

                    // **NEW**: Add VHS color cast (slight magenta/red bias)
                    if boost > 0.5 {
                        new_r *= 1.05;
                        new_g *= 0.98;
                        new_b *= 1.02;
                    }

                    pixel[0] = (new_r.clamp(0.0, 1.0) * 255.0) as u8;
                    pixel[1] = (new_g.clamp(0.0, 1.0) * 255.0) as u8;
                    pixel[2] = (new_b.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
    }

    /// **NEW**: Apply VHS-style color temperature shift
    fn apply_color_temperature(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        // VHS tapes often had warm, slightly degraded colors
        let warmth = intensity * 0.3;

        for y in 0..height {
            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);

                // Warm up the image (more red, less blue)
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;

                let new_r = (r * (1.0 + warmth * 0.2)).min(255.0);
                let new_g = (g * (1.0 + warmth * 0.1)).min(255.0);
                let new_b = (b * (1.0 - warmth * 0.15)).max(0.0);

                pixel[0] = new_r as u8;
                pixel[1] = new_g as u8;
                pixel[2] = new_b as u8;
            }
        }
    }
}

impl Style for VhsStyle {
    fn name(&self) -> &str {
        "vhs"
    }

    fn description(&self) -> &str {
        "Enhanced VHS video tape aesthetic with pronounced scan lines, color bleeding, tracking errors, and noise"
    }

    fn apply_effect(&self, frame: &mut Frame, config: &StyleConfig) -> Result<()> {
        let intensity = config.intensity;

        // Get VHS-specific parameters with enhanced defaults
        let scanline_intensity = config.get_f32_or(SCANLINE_INTENSITY, 0.9);
        let color_bleeding = config.get_f32_or(COLOR_BLEEDING, 0.8);
        let tracking_error = config.get_f32_or(TRACKING_ERROR, 0.5);
        let noise_level = config.get_f32_or(NOISE_LEVEL, 0.6);
        let chroma_shift = config.get_f32_or(CHROMA_SHIFT, 0.7);
        let saturation_boost = config.get_f32_or(SATURATION_BOOST, 0.4);

        // **ENHANCED**: Apply effects in optimal order for maximum visual impact
        self.apply_scanlines(frame, scanline_intensity * intensity);
        self.apply_color_bleeding(frame, color_bleeding * intensity);
        self.apply_chroma_shift(frame, chroma_shift * intensity);
        self.apply_tracking_error(frame, tracking_error * intensity);
        self.apply_noise(frame, noise_level * intensity);
        self.apply_saturation_boost(frame, saturation_boost * intensity);

        // **NEW**: Add color temperature shift for authentic VHS look
        self.apply_color_temperature(frame, intensity);

        Ok(())
    }

    fn metadata(&self) -> StyleMetadata {
        StyleMetadata {
            gpu_accelerated: false,
            performance_impact: 0.7, // Increased due to enhanced effects
            composable: true,
            required_parameters: vec![],
            optional_parameters: vec![
                (SCANLINE_INTENSITY.to_string(), "Intensity of horizontal scan lines (0.0-1.0)".to_string()),
                (COLOR_BLEEDING.to_string(), "Amount of color channel bleeding (0.0-1.0)".to_string()),
                (TRACKING_ERROR.to_string(), "Frequency of tracking errors (0.0-1.0)".to_string()),
                (NOISE_LEVEL.to_string(), "Amount of video noise (0.0-1.0)".to_string()),
                (CHROMA_SHIFT.to_string(), "Chromatic aberration intensity (0.0-1.0)".to_string()),
                (SATURATION_BOOST.to_string(), "Saturation enhancement (0.0-1.0)".to_string()),
            ],
        }
    }
}