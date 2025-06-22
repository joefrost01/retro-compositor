// src/video/processor.rs - Enhanced for smoother motion

use std::collections::HashMap;

use rayon::prelude::*;
use tracing::{debug, info, warn};

use crate::error::{VideoError, Result};
use crate::styles::{Style, StyleConfig};
use crate::video::types::{Frame, VideoClip, VideoParams};
use crate::video::loader_optimized::{VideoLoader, VideoMetadata};
use crate::composition::engine::CompositionTimeline;

pub struct VideoProcessor {
    loader: VideoLoader,
    frame_cache: HashMap<String, Vec<CachedFrame>>,
    target_params: VideoParams,
}

#[derive(Clone)]
struct CachedFrame {
    frame: Frame,
    timestamp: f64,
}

#[derive(Debug, Clone)]
pub struct ProcessedSegment {
    pub start_time: f64,
    pub end_time: f64,
    pub clip_id: u32,
    pub frames: Vec<Frame>,
    pub frame_timestamps: Vec<f64>,
}

impl VideoProcessor {
    pub fn new(target_params: VideoParams) -> Result<Self> {
        Ok(Self {
            loader: VideoLoader::new()?,
            frame_cache: HashMap::new(),
            target_params,
        })
    }

    pub async fn process_timeline(
        &mut self,
        timeline: &CompositionTimeline,
        video_clips: &[VideoClip],
        style: &dyn Style,
        style_config: &StyleConfig,
        total_duration: f64,
    ) -> Result<Vec<ProcessedSegment>> {
        info!("Processing {} timeline segments with {} style",
              timeline.cuts.len(), style.name());

        let mut processed_segments = Vec::new();

        for (i, &cut_time) in timeline.cuts.iter().enumerate() {
            let clip_id = timeline.clip_assignments.get(i).copied().unwrap_or(1);

            let clip = video_clips.iter()
                .find(|c| c.sequence_number == clip_id)
                .ok_or_else(|| VideoError::LoadFailed {
                    path: format!("clip_{}", clip_id),
                })?;

            let segment_end = timeline.cuts.get(i + 1).copied().unwrap_or(total_duration);
            let segment_duration = segment_end - cut_time;

            debug!("Processing segment {}: {:.2}s-{:.2}s using clip '{}' ({:.2}s)",
                   i, cut_time, segment_end, clip.name, segment_duration);

            let segment = self.process_segment_smooth(
                clip,
                cut_time,
                segment_end,
                segment_duration,
                style,
                style_config,
            ).await?;

            processed_segments.push(segment);
        }

        info!("Successfully processed {} segments", processed_segments.len());
        Ok(processed_segments)
    }

    /// **ENHANCED** segment processing for smoother motion
    async fn process_segment_smooth(
        &mut self,
        clip: &VideoClip,
        start_time: f64,
        end_time: f64,
        duration: f64,
        style: &dyn Style,
        style_config: &StyleConfig,
    ) -> Result<ProcessedSegment> {
        let target_fps = self.target_params.fps;

        // **SMOOTH MOTION**: Calculate precise frame count and timing
        let frame_count = (duration * target_fps).round() as usize;
        let precise_frame_interval = duration / frame_count.max(1) as f64;

        debug!("Segment needs {} frames at {:.1} fps (precise interval: {:.6}s)",
               frame_count, target_fps, precise_frame_interval);

        // **SMOOTH EXTRACTION**: Get frames with better temporal distribution
        let source_frames = self.extract_frames_smooth(clip, duration, frame_count).await?;

        // **ENHANCED EFFECTS**: Apply with temporal consistency
        let processed_frames = self.apply_effects_with_consistency(
            source_frames,
            style,
            style_config,
            frame_count,
        ).await?;

        // Generate precise frame timestamps
        let frame_timestamps: Vec<f64> = (0..frame_count)
            .map(|i| i as f64 * precise_frame_interval)
            .collect();

        Ok(ProcessedSegment {
            start_time,
            end_time,
            clip_id: clip.sequence_number,
            frames: processed_frames,
            frame_timestamps,
        })
    }

    /// **SMOOTH EXTRACTION** with better temporal sampling
    async fn extract_frames_smooth(
        &mut self,
        clip: &VideoClip,
        duration: f64,
        frame_count: usize,
    ) -> Result<Vec<Frame>> {
        let path_str = clip.path.display().to_string();

        // Load metadata to understand the clip
        let metadata = self.loader.load_metadata(&clip.path)?;
        debug!("Clip metadata: {:.1}s, {:.1} fps, {}x{}",
               metadata.duration, metadata.fps, metadata.width, metadata.height);

        // **SMOOTH SAMPLING**: Calculate optimal timestamps for natural motion
        let timestamps = self.calculate_smooth_timestamps(&metadata, duration, frame_count);

        debug!("Extracting {} frames with smooth sampling from clip: {}", 
               timestamps.len(), clip.name);

        // Extract frames
        let frames = self.loader.extract_frames_at_times(&clip.path, &timestamps)?;

        // **ENSURE CONSISTENT SIZING**: Resize all frames to target resolution
        let mut consistent_frames = Vec::with_capacity(frames.len());
        let target_size = self.target_params.resolution;

        for frame in frames {
            let resized_frame = if frame.width() != target_size.0 || frame.height() != target_size.1 {
                self.resize_frame_smooth(&frame, target_size)?
            } else {
                frame
            };
            consistent_frames.push(resized_frame);
        }

        Ok(consistent_frames)
    }

    /// **SMOOTH TIMESTAMP CALCULATION** for natural motion
    fn calculate_smooth_timestamps(
        &self,
        metadata: &VideoMetadata,
        segment_duration: f64,
        frame_count: usize,
    ) -> Vec<f64> {
        let clip_duration = metadata.duration;

        if clip_duration >= segment_duration {
            // **STRATEGY 1**: Clip is longer - sample from the middle for stability
            let start_offset = (clip_duration - segment_duration) / 2.0;

            (0..frame_count)
                .map(|i| {
                    start_offset + (i as f64 * segment_duration) / frame_count.max(1) as f64
                })
                .collect()
        } else {
            // **STRATEGY 2**: Clip is shorter - use smooth looping
            (0..frame_count)
                .map(|i| {
                    let relative_time = (i as f64) / frame_count.max(1) as f64;
                    let absolute_time = relative_time * segment_duration;

                    // **SMOOTH LOOPING**: Avoid jumps at loop boundaries
                    let mut loop_position = absolute_time % clip_duration;

                    // Blend near loop boundaries for smoother transitions
                    if loop_position < 0.1 && absolute_time > clip_duration {
                        // Near start of loop - blend with end
                        let blend_factor = loop_position / 0.1;
                        loop_position = loop_position * blend_factor + (clip_duration - 0.1) * (1.0 - blend_factor);
                    }

                    loop_position
                })
                .collect()
        }
    }

    /// **SMOOTH RESIZING** with quality preservation
    fn resize_frame_smooth(&self, frame: &Frame, target_size: (u32, u32)) -> Result<Frame> {
        use image::imageops::FilterType;

        // Use high-quality Lanczos3 filter for smooth resizing
        let resized = image::imageops::resize(
            frame.as_image(),
            target_size.0,
            target_size.1,
            FilterType::Lanczos3,
        );

        Ok(Frame::new(resized))
    }

    /// **ENHANCED EFFECTS** with temporal consistency
    async fn apply_effects_with_consistency(
        &self,
        mut frames: Vec<Frame>,
        style: &dyn Style,
        style_config: &StyleConfig,
        frame_count: usize,
    ) -> Result<Vec<Frame>> {
        debug!("Applying {} effects to {} frames with temporal consistency", 
               style.name(), frames.len());

        // **TEMPORAL CONSISTENCY**: Create variation that changes smoothly over time
        frames.par_iter_mut().enumerate().try_for_each(|(i, frame)| {
            // Create frame-specific config with temporal variation
            let mut frame_config = style_config.clone();

            // **SMOOTH VARIATION**: Slowly varying parameters for natural feel
            let time_factor = i as f32 / frame_count.max(1) as f32;
            let slow_wave = (time_factor * std::f32::consts::PI * 0.5).sin() * 0.2;

            // Vary intensity slightly over time to avoid static look
            frame_config.intensity = (style_config.intensity + slow_wave * 0.3).clamp(0.0, 1.0);

            // For VHS effects, add subtle temporal variation
            if style.name() == "vhs" {
                // Vary tracking errors over time
                let tracking_base = style_config.get_f32_or("tracking_error", 0.5);
                let tracking_variation = (time_factor * std::f32::consts::PI * 2.0).sin() * 0.1;
                frame_config = frame_config.set("tracking_error", tracking_base + tracking_variation);

                // Vary noise slightly
                let noise_base = style_config.get_f32_or("noise_level", 0.6);
                let noise_variation = (time_factor * std::f32::consts::PI * 3.0).sin() * 0.05;
                frame_config = frame_config.set("noise_level", noise_base + noise_variation);
            }

            style.apply_effect(frame, &frame_config)
                .map_err(|e| VideoError::FrameProcessingFailed {
                    reason: format!("Effect application failed: {}", e),
                })
        })?;

        Ok(frames)
    }

    /// Resize frames to match target resolution
    pub fn resize_frames(&self, frames: &mut [Frame]) -> Result<()> {
        let target_resolution = self.target_params.resolution;

        for frame in frames.iter_mut() {
            if frame.width() != target_resolution.0 || frame.height() != target_resolution.1 {
                *frame = self.resize_frame_smooth(frame, target_resolution)?;
            }
        }

        Ok(())
    }

    pub fn get_stats(&self) -> ProcessingStats {
        let cached_clips = self.frame_cache.len();
        let total_cached_frames: usize = self.frame_cache.values()
            .map(|frames| frames.len())
            .sum();

        ProcessingStats {
            cached_clips,
            total_cached_frames,
            target_fps: self.target_params.fps,
            target_resolution: self.target_params.resolution,
        }
    }

    pub fn clear_cache(&mut self) {
        self.frame_cache.clear();
        self.loader.clear_cache();
    }
}

#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub cached_clips: usize,
    pub total_cached_frames: usize,
    pub target_fps: f64,
    pub target_resolution: (u32, u32),
}