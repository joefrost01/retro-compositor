use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use tracing::{debug, info, warn};

use crate::error::{VideoError, Result};
use crate::styles::{Style, StyleConfig};
use crate::video::types::{Frame, VideoClip, VideoParams};
use crate::video::loader_pure_rust::{VideoLoader, VideoMetadata};
use crate::composition::engine::CompositionTimeline;

/// Processes video frames with retro effects and manages frame extraction
pub struct VideoProcessor {
    loader: VideoLoader,
    frame_cache: HashMap<String, Vec<CachedFrame>>,
    target_params: VideoParams,
}

/// A cached frame with its timestamp
#[derive(Clone)]
struct CachedFrame {
    frame: Frame,
    timestamp: f64,
}

/// Represents a processed video segment
#[derive(Debug, Clone)]
pub struct ProcessedSegment {
    /// Start time in the original timeline
    pub start_time: f64,

    /// End time in the original timeline  
    pub end_time: f64,

    /// Which clip this segment comes from
    pub clip_id: u32,

    /// Processed frames for this segment
    pub frames: Vec<Frame>,

    /// Frame timestamps relative to segment start
    pub frame_timestamps: Vec<f64>,
}

impl VideoProcessor {
    /// Create a new video processor
    pub fn new(target_params: VideoParams) -> Result<Self> {
        Ok(Self {
            loader: VideoLoader::new()?,
            frame_cache: HashMap::new(),
            target_params,
        })
    }

    /// Process all video segments according to the timeline
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

            // Find the corresponding video clip
            let clip = video_clips.iter()
                .find(|c| c.sequence_number == clip_id)
                .ok_or_else(|| VideoError::LoadFailed {
                    path: format!("clip_{}", clip_id),
                })?;

            // Calculate segment duration
            let segment_end = timeline.cuts.get(i + 1).copied().unwrap_or(total_duration);
            let segment_duration = segment_end - cut_time;

            debug!("Processing segment {}: {:.2}s-{:.2}s using clip '{}' ({:.2}s)",
                   i, cut_time, segment_end, clip.name, segment_duration);

            // Process this segment
            let segment = self.process_segment(
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

    /// Process a single timeline segment
    async fn process_segment(
        &mut self,
        clip: &VideoClip,
        start_time: f64,
        end_time: f64,
        duration: f64,
        style: &dyn Style,
        style_config: &StyleConfig,
    ) -> Result<ProcessedSegment> {
        // Calculate how many frames we need for this segment
        let target_fps = self.target_params.fps;
        let frame_count = (duration * target_fps).ceil() as usize;
        let frame_interval = duration / frame_count.max(1) as f64;

        debug!("Segment needs {} frames at {:.1} fps (interval: {:.3}s)",
               frame_count, target_fps, frame_interval);

        // Extract frames from the source clip
        let source_frames = self.extract_frames_for_segment(clip, duration, frame_count).await?;

        // Apply retro effects to each frame
        let processed_frames = self.apply_effects_to_frames(
            source_frames,
            style,
            style_config,
        ).await?;

        // Generate frame timestamps
        let frame_timestamps: Vec<f64> = (0..frame_count)
            .map(|i| i as f64 * frame_interval)
            .collect();

        Ok(ProcessedSegment {
            start_time,
            end_time,
            clip_id: clip.sequence_number,
            frames: processed_frames,
            frame_timestamps,
        })
    }

    /// Extract frames from a clip for a specific segment duration
    async fn extract_frames_for_segment(
        &mut self,
        clip: &VideoClip,
        duration: f64,
        frame_count: usize,
    ) -> Result<Vec<Frame>> {
        let path_str = clip.path.display().to_string();

        // Check if we have this clip cached
        if let Some(cached_frames) = self.frame_cache.get(&path_str) {
            debug!("Using cached frames for clip: {}", clip.name);
            return Ok(self.sample_frames_from_cache(cached_frames, duration, frame_count));
        }

        // Load metadata to understand the clip
        let metadata = self.loader.load_metadata(&clip.path)?;
        debug!("Clip metadata: {:.1}s, {:.1} fps, {}x{}", 
               metadata.duration, metadata.fps, metadata.width, metadata.height);

        // Determine timestamps to extract
        let timestamps = self.calculate_extraction_timestamps(&metadata, duration, frame_count);

        // Extract frames
        debug!("Extracting {} frames from clip: {}", timestamps.len(), clip.name);
        let frames = self.loader.extract_frames_at_times(&clip.path, &timestamps)?;

        // Cache the frames for potential reuse
        let cached_frames: Vec<CachedFrame> = frames.iter()
            .zip(timestamps.iter())
            .map(|(frame, &timestamp)| CachedFrame {
                frame: frame.clone(),
                timestamp,
            })
            .collect();

        self.frame_cache.insert(path_str, cached_frames);

        Ok(frames)
    }

    /// Calculate optimal timestamps for frame extraction
    fn calculate_extraction_timestamps(
        &self,
        metadata: &VideoMetadata,
        segment_duration: f64,
        frame_count: usize,
    ) -> Vec<f64> {
        let clip_duration = metadata.duration;

        // Strategy 1: If the clip is longer than the segment, extract evenly across the clip
        if clip_duration >= segment_duration {
            let start_offset = (clip_duration - segment_duration) / 2.0; // Center the extraction
            (0..frame_count)
                .map(|i| {
                    start_offset + (i as f64 * segment_duration) / frame_count.max(1) as f64
                })
                .collect()
        }
        // Strategy 2: If the clip is shorter, loop it or stretch it
        else {
            (0..frame_count)
                .map(|i| {
                    let relative_time = (i as f64) / frame_count.max(1) as f64;
                    (relative_time * clip_duration) % clip_duration
                })
                .collect()
        }
    }

    /// Sample frames from cache to match the required duration and count
    fn sample_frames_from_cache(
        &self,
        cached_frames: &[CachedFrame],
        duration: f64,
        frame_count: usize,
    ) -> Vec<Frame> {
        if cached_frames.is_empty() {
            return vec![Frame::new_black(self.target_params.resolution.0, self.target_params.resolution.1); frame_count];
        }

        let mut sampled_frames = Vec::with_capacity(frame_count);

        for i in 0..frame_count {
            let relative_time = (i as f64) / frame_count.max(1) as f64;

            // Find the closest cached frame
            let closest_frame = cached_frames
                .iter()
                .min_by(|a, b| {
                    let a_dist = (a.timestamp - relative_time).abs();
                    let b_dist = (b.timestamp - relative_time).abs();
                    a_dist.partial_cmp(&b_dist).unwrap()
                })
                .map(|cached| &cached.frame)
                .unwrap_or(&cached_frames[0].frame);

            sampled_frames.push(closest_frame.clone());
        }

        sampled_frames
    }

    /// Apply retro effects to a batch of frames
    async fn apply_effects_to_frames(
        &self,
        mut frames: Vec<Frame>,
        style: &dyn Style,
        style_config: &StyleConfig,
    ) -> Result<Vec<Frame>> {
        debug!("Applying {} effects to {} frames", style.name(), frames.len());

        // Process frames in parallel for better performance
        frames.par_iter_mut().try_for_each(|frame| {
            style.apply_effect(frame, style_config)
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
                *frame = self.resize_frame(frame, target_resolution)?;
            }
        }

        Ok(())
    }

    /// Resize a single frame
    fn resize_frame(&self, frame: &Frame, target_size: (u32, u32)) -> Result<Frame> {
        use image::imageops::FilterType;

        let resized = image::imageops::resize(
            frame.as_image(),
            target_size.0,
            target_size.1,
            FilterType::Lanczos3,
        );

        Ok(Frame::new(resized))
    }

    /// Get processing statistics
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

    /// Clear frame cache to free memory
    pub fn clear_cache(&mut self) {
        self.frame_cache.clear();
        self.loader.clear_cache();
    }
}

/// Statistics about video processing
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub cached_clips: usize,
    pub total_cached_frames: usize,
    pub target_fps: f64,
    pub target_resolution: (u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::VhsStyle;
    use crate::composition::engine::CompositionTimeline;

    #[test]
    fn test_timestamp_calculation() {
        let processor = VideoProcessor::new(VideoParams::default()).unwrap();

        let metadata = VideoMetadata {
            duration: 10.0,
            fps: 30.0,
            width: 1920,
            height: 1080,
            codec: "h264".to_string(),
            frame_count: 300,
        };

        // Test normal case
        let timestamps = processor.calculate_extraction_timestamps(&metadata, 5.0, 10);
        assert_eq!(timestamps.len(), 10);
        assert!(timestamps[0] >= 0.0);
        assert!(timestamps.last().unwrap() <= &metadata.duration);

        // Test short clip case
        let metadata_short = VideoMetadata {
            duration: 2.0,
            fps: 30.0,
            width: 1920,
            height: 1080,
            codec: "h264".to_string(),
            frame_count: 60,
        };

        let timestamps = processor.calculate_extraction_timestamps(&metadata_short, 5.0, 10);
        assert_eq!(timestamps.len(), 10);
        assert!(timestamps.iter().all(|&t| t >= 0.0 && t <= metadata_short.duration));
    }

    #[tokio::test]
    async fn test_frame_processing() {
        let processor = VideoProcessor::new(VideoParams::default()).unwrap();
        let style = VhsStyle::new();
        let config = StyleConfig::default();

        // Create test frames
        let frames = vec![
            Frame::new_filled(100, 100, [255, 0, 0]),
            Frame::new_filled(100, 100, [0, 255, 0]),
            Frame::new_filled(100, 100, [0, 0, 255]),
        ];

        let result = processor.apply_effects_to_frames(frames, &style, &config).await;
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.len(), 3);
    }
}