use rand::Rng;

use crate::{
    error::Result,
    styles::{Style, StyleConfig},
    styles::traits::StyleMetadata,
    video::types::Frame,
};

use super::{SCANLINE_INTENSITY, COLOR_BLEEDING, TRACKING_ERROR, NOISE_LEVEL, CHROMA_SHIFT, SATURATION_BOOST};

/// VHS-style video effect implementation
///
/// Applies authentic VHS characteristics including:
/// - Horizontal scan lines with varying intensity
/// - Color bleeding and chromatic aberration
/// - Tracking errors and horizontal displacement
/// - Video noise and grain
/// - Chroma subsampling artifacts
/// - Slightly boosted saturation typical of VHS
pub struct VhsStyle;

impl VhsStyle {
    pub fn new() -> Self {
        Self
    }

    /// Apply scan line effect to the frame
    fn apply_scanlines(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        for y in 0..height {
            // Every other line gets darkened to simulate scan lines
            if y % 2 == 0 {
                let darken_factor = 1.0 - (intensity * 0.3);

                for x in 0..width {
                    let pixel = frame.get_pixel_mut(x, y);
                    pixel[0] = (pixel[0] as f32 * darken_factor) as u8;
                    pixel[1] = (pixel[1] as f32 * darken_factor) as u8;
                    pixel[2] = (pixel[2] as f32 * darken_factor) as u8;
                }
            }
        }
    }

    /// Apply color bleeding effect (simulates VHS chroma artifacts)
    fn apply_color_bleeding(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        // Create a copy for reading from while modifying the original
        let original = frame.clone();

        for y in 0..height {
            for x in 1..width-1 {
                let current = original.get_pixel(x, y);
                let left = original.get_pixel(x-1, y);
                let right = original.get_pixel(x+1, y);

                // Blend chroma channels with neighboring pixels
                let blend_factor = intensity * 0.2;

                let new_pixel = frame.get_pixel_mut(x, y);

                // Red channel bleeds right
                new_pixel[0] = (current[0] as f32 * (1.0 - blend_factor) +
                    right[0] as f32 * blend_factor) as u8;

                // Blue channel bleeds left  
                new_pixel[2] = (current[2] as f32 * (1.0 - blend_factor) +
                    left[2] as f32 * blend_factor) as u8;

                // Green stays mostly in place but gets slight averaging
                new_pixel[1] = (current[1] as f32 * (1.0 - blend_factor * 0.5) +
                    ((left[1] as f32 + right[1] as f32) * 0.5) * (blend_factor * 0.5)) as u8;
            }
        }
    }

    /// Apply tracking errors (horizontal displacement)
    fn apply_tracking_error(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();

        let mut rng = rand::thread_rng();

        // Randomly displace some scan lines
        for y in 0..height {
            if rng.gen::<f32>() < intensity * 0.05 {
                let displacement = rng.gen_range(-3..=3);
                self.displace_scanline(frame, y, displacement);
            }
        }
    }

    /// Displace a single scan line horizontally
    fn displace_scanline(&self, frame: &mut Frame, y: u32, displacement: i32) {
        let width = frame.width() as i32;

        if displacement == 0 { return; }

        // Create a copy of the line
        let mut line_data = Vec::with_capacity(width as usize * 3);
        for x in 0..width {
            let pixel = frame.get_pixel(x as u32, y);
            line_data.extend_from_slice(&pixel);
        }

        // Apply displacement
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
            }
        }
    }

    /// Add VHS-style noise
    fn apply_noise(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();

        let mut rng = rand::thread_rng();

        for y in 0..height {
            for x in 0..width {
                if rng.gen::<f32>() < intensity * 0.02 {
                    let noise = rng.gen_range(-20..=20);
                    let pixel = frame.get_pixel_mut(x, y);

                    for channel in 0..3 {
                        let new_value = (pixel[channel] as i16 + noise).clamp(0, 255);
                        pixel[channel] = new_value as u8;
                    }
                }
            }
        }
    }

    /// Apply chromatic aberration
    fn apply_chroma_shift(&self, frame: &mut Frame, intensity: f32) {
        let height = frame.height();
        let width = frame.width();
        let shift = (intensity * 2.0) as i32;

        if shift == 0 { return; }

        let original = frame.clone();

        for y in 0..height {
            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);

                // Shift red channel
                let red_x = (x as i32 + shift).clamp(0, width as i32 - 1) as u32;
                pixel[0] = original.get_pixel(red_x, y)[0];

                // Shift blue channel in opposite direction
                let blue_x = (x as i32 - shift).clamp(0, width as i32 - 1) as u32;
                pixel[2] = original.get_pixel(blue_x, y)[2];
            }
        }
    }

    /// Boost saturation slightly (VHS characteristic)
    fn apply_saturation_boost(&self, frame: &mut Frame, boost: f32) {
        let height = frame.height();
        let width = frame.width();

        for y in 0..height {
            for x in 0..width {
                let pixel = frame.get_pixel_mut(x, y);

                // Convert to HSV-like adjustment
                let r = pixel[0] as f32 / 255.0;
                let g = pixel[1] as f32 / 255.0;
                let b = pixel[2] as f32 / 255.0;

                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                let delta = max - min;

                if delta > 0.0 {
                    let saturation_factor = 1.0 + boost * 0.3;

                    // Boost saturation by moving values away from the average
                    let avg = (r + g + b) / 3.0;

                    let new_r = (avg + (r - avg) * saturation_factor).clamp(0.0, 1.0);
                    let new_g = (avg + (g - avg) * saturation_factor).clamp(0.0, 1.0);
                    let new_b = (avg + (b - avg) * saturation_factor).clamp(0.0, 1.0);

                    pixel[0] = (new_r * 255.0) as u8;
                    pixel[1] = (new_g * 255.0) as u8;
                    pixel[2] = (new_b * 255.0) as u8;
                }
            }
        }
    }
}

impl Style for VhsStyle {
    fn name(&self) -> &str {
        "vhs"
    }

    fn description(&self) -> &str {
        "Authentic VHS video tape aesthetic with scan lines, color bleeding, tracking errors, and noise"
    }

    fn apply_effect(&self, frame: &mut Frame, config: &StyleConfig) -> Result<()> {
        let intensity = config.intensity;

        // Get VHS-specific parameters with defaults
        let scanline_intensity = config.get_f32_or(SCANLINE_INTENSITY, 0.8);
        let color_bleeding = config.get_f32_or(COLOR_BLEEDING, 0.6);
        let tracking_error = config.get_f32_or(TRACKING_ERROR, 0.3);
        let noise_level = config.get_f32_or(NOISE_LEVEL, 0.4);
        let chroma_shift = config.get_f32_or(CHROMA_SHIFT, 0.5);
        let saturation_boost = config.get_f32_or(SATURATION_BOOST, 0.2);

        // Apply effects in order, scaling by overall intensity
        self.apply_scanlines(frame, scanline_intensity * intensity);
        self.apply_color_bleeding(frame, color_bleeding * intensity);
        self.apply_tracking_error(frame, tracking_error * intensity);
        self.apply_noise(frame, noise_level * intensity);
        self.apply_chroma_shift(frame, chroma_shift * intensity);
        self.apply_saturation_boost(frame, saturation_boost * intensity);

        Ok(())
    }

    fn metadata(&self) -> StyleMetadata {
        StyleMetadata {
            gpu_accelerated: false, // CPU implementation for now
            performance_impact: 0.6, // Moderate impact
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