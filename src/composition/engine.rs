use std::path::Path;
use tracing::{info, debug, warn};

use crate::{
    audio::{AudioLoader, AudioAnalyzer, AudioAnalysis},
    config::Config,
    error::{CompositionError, Result},
    styles::Style,
    video::{VideoLoader, VideoProcessor, VideoCompositor, VideoSequence, VideoClip},
};

/// Main composition engine that orchestrates the entire retro video creation process
///
/// The engine follows a clear pipeline:
/// 1. Audio Analysis - Load and analyze audio for beats, tempo, energy
/// 2. Video Loading - Discover and load video clips from directory
/// 3. Timeline Generation - Map audio beats to video cut points
/// 4. Video Processing - Apply retro effects to video frames
/// 5. Output Generation - Compose final video with synchronized cuts
pub struct CompositionEngine {
    config: Config,
    style: Box<dyn Style>,
}

impl CompositionEngine {
    /// Create a new composition engine with the given configuration and style
    pub fn new(config: Config, style: Box<dyn Style>) -> Self {
        Self { config, style }
    }

    /// Main composition method - orchestrates the entire pipeline
    ///
    /// # Arguments
    ///
    /// * `audio_path` - Path to the audio file (WAV, MP3, FLAC, etc.)
    /// * `video_dir` - Directory containing numbered video clips (01_intro.mp4, etc.)
    /// * `output_path` - Path for the final output video
    pub async fn compose<P: AsRef<Path>>(
        &self,
        audio_path: P,
        video_dir: P,
        output_path: P,
    ) -> Result<()> {
        let audio_path = audio_path.as_ref();
        let video_dir = video_dir.as_ref();
        let output_path = output_path.as_ref();

        info!("🎬 Starting Retro-Compositor composition");
        info!("   Audio: {:?}", audio_path);
        info!("   Videos: {:?}", video_dir);
        info!("   Output: {:?}", output_path);
        info!("   Style: {}", self.style.name());

        // Pipeline Step 1: Audio Analysis
        let audio_analysis = self.analyze_audio(audio_path).await?;

        // Pipeline Step 2: Video Discovery and Loading
        let video_sequence = self.load_video_clips(video_dir).await?;

        // Pipeline Step 3: Timeline Generation
        let timeline = self.generate_timeline(&audio_analysis, &video_sequence).await?;

        // Pipeline Step 4: Video Processing with Effects
        let processed_segments = self.process_video_with_effects(
            &video_sequence,
            &timeline,
            &audio_analysis
        ).await?;

        // Pipeline Step 5: Final Output Generation
        self.generate_final_output(&processed_segments, audio_path, output_path).await?;

        info!("🎉 Composition complete! Output saved to: {:?}", output_path);
        Ok(())
    }

    // ==========================================
    // PIPELINE STEP 1: AUDIO ANALYSIS
    // ==========================================

    /// Load and analyze audio file for beats, tempo, and energy levels
    async fn analyze_audio(&self, audio_path: &Path) -> Result<AudioAnalysis> {
        info!("🎵 Step 1: Analyzing audio file...");

        // Load the audio file
        debug!("Loading audio from: {:?}", audio_path);
        let audio_data = AudioLoader::load(audio_path).await
            .map_err(|e| {
                warn!("Failed to load audio file: {}", e);
                e
            })?;

        info!("   Loaded: {:.1}s, {} Hz, {} channels",
              audio_data.duration, audio_data.sample_rate, audio_data.channels);

        // Configure analysis based on user settings
        let analysis_config = crate::audio::types::AnalysisConfig {
            window_size: self.config.audio.window_size,
            hop_size: self.config.audio.hop_size,
            min_bpm: self.config.audio.min_bpm,
            max_bpm: self.config.audio.max_bpm,
            beat_sensitivity: self.config.audio.beat_sensitivity,
            energy_window_size: 0.1, // 100ms energy windows
            detect_phrases: true,
            calculate_spectral_features: true,
        };

        // Perform the analysis
        debug!("Running audio analysis with sensitivity: {:.1}", analysis_config.beat_sensitivity);
        let analyzer = AudioAnalyzer::with_config(analysis_config);
        let analysis = analyzer.analyze(&audio_data).await?;

        info!("   ✅ Analysis complete:");
        info!("      Beats detected: {}", analysis.beats.len());
        info!("      BPM: {:.1} (confidence: {:.2})", analysis.bpm, analysis.bpm_confidence);
        info!("      Energy levels: {}", analysis.energy_levels.len());
        info!("      Musical phrases: {}", analysis.phrases.len());

        Ok(analysis)
    }

    // ==========================================
    // PIPELINE STEP 2: VIDEO DISCOVERY & LOADING
    // ==========================================

    /// Discover and load video clips from the specified directory
    async fn load_video_clips(&self, video_dir: &Path) -> Result<VideoSequence> {
        info!("📹 Step 2: Loading video clips...");

        if !video_dir.exists() {
            return Err(CompositionError::NoClipsFound {
                path: video_dir.display().to_string()
            }.into());
        }

        let mut video_loader = VideoLoader::new()
            .map_err(|e| CompositionError::SequencingFailed {
                reason: format!("Failed to initialize video loader: {}", e)
            })?;

        let clips = video_loader.load_clips_from_directory(video_dir)
            .map_err(|e| CompositionError::NoClipsFound {
                path: format!("{}: {}", video_dir.display(), e)
            })?;

        if clips.is_empty() {
            return Err(CompositionError::NoClipsFound {
                path: video_dir.display().to_string()
            }.into());
        }

        let sequence: VideoSequence = clips.into_iter().collect();

        info!("   ✅ Video clips loaded:");
        info!("      Clips loaded: {}", sequence.len());
        info!("      Sequence length: {}", sequence.len());

        // Log the sequence order
        for clip in sequence.iter() {
            debug!("      {:02} - {} ({})",
                   clip.sequence_number, clip.name,
                   clip.extension().unwrap_or("unknown"));
        }

        Ok(sequence)
    }

    // ==========================================
    // PIPELINE STEP 3: TIMELINE GENERATION
    // ==========================================

    /// Generate intelligent timeline mapping audio beats to video cuts
    async fn generate_timeline(
        &self,
        audio_analysis: &AudioAnalysis,
        video_sequence: &VideoSequence,
    ) -> Result<CompositionTimeline> {
        info!("⏱️  Step 3: Generating composition timeline...");

        if video_sequence.is_empty() {
            return Err(CompositionError::SequencingFailed {
                reason: "No video clips available".to_string()
            }.into());
        }

        let mut timeline = CompositionTimeline::new();
        let num_clips = video_sequence.len() as u32;

        debug!("Processing {} beats with {} clips available",
               audio_analysis.beats.len(), num_clips);

        // Start timeline at the beginning
        timeline.add_cut(0.0, 1);

        let mut current_clip = 1u32;
        let mut last_cut_time = 0.0;

        // Process each beat for potential cuts
        for beat in &audio_analysis.beats {
            let time_since_last_cut = beat.time - last_cut_time;

            // Determine if we should cut at this beat
            let should_cut = self.should_cut_at_beat(
                beat,
                time_since_last_cut,
                audio_analysis
            );

            if should_cut {
                // Choose next clip using intelligent selection
                current_clip = self.select_next_clip(
                    current_clip,
                    num_clips,
                    beat,
                    audio_analysis
                );

                timeline.add_cut(beat.time, current_clip);
                last_cut_time = beat.time;

                debug!("Cut at {:.2}s -> Clip {} (beat strength: {:.2})",
                       beat.time, current_clip, beat.strength);
            }
        }

        // Ensure we have reasonable coverage
        self.ensure_minimum_coverage(&mut timeline, audio_analysis.duration, num_clips);

        info!("   ✅ Timeline generated:");
        info!("      Total cuts: {}", timeline.cuts.len());
        info!("      Average segment: {:.1}s",
              audio_analysis.duration / timeline.cuts.len() as f64);
        info!("      Clips used: {}", timeline.unique_clips().len());

        Ok(timeline)
    }

    /// Determine if we should cut at this beat based on various factors
    fn should_cut_at_beat(
        &self,
        beat: &crate::audio::types::Beat,
        time_since_last_cut: f64,
        audio_analysis: &AudioAnalysis,
    ) -> bool {
        let config = &self.config.composition;

        // Force cut if maximum interval exceeded
        if time_since_last_cut >= config.max_cut_interval {
            return true;
        }

        // Don't cut if minimum interval not met
        if time_since_last_cut < config.min_cut_interval {
            return false;
        }

        // Calculate cut probability based on multiple factors
        let mut cut_probability = 0.0;

        // Beat strength factor (stronger beats more likely to cut)
        cut_probability += beat.strength * 0.4;

        // Energy factor (high energy sections more likely to cut)
        let local_energy = audio_analysis.average_energy_in_range(
            beat.time - 0.5,
            beat.time + 0.5
        );
        cut_probability += local_energy * 0.3;

        // Beat type factor (downbeats more likely to cut)
        if beat.beat_type == crate::audio::types::BeatType::Downbeat {
            cut_probability += 0.2;
        }

        // Time factor (longer since last cut = higher probability)
        let time_factor = ((time_since_last_cut - config.min_cut_interval) /
            (config.max_cut_interval - config.min_cut_interval)).min(1.0) as f32;
        cut_probability += time_factor * 0.1;

        // Apply beat sync strength from configuration
        cut_probability *= config.beat_sync_strength;

        // Threshold for cutting (tuned for good results)
        cut_probability >= 0.6
    }

    /// Select the next clip to use, considering musical context
    fn select_next_clip(
        &self,
        current_clip: u32,
        num_clips: u32,
        beat: &crate::audio::types::Beat,
        audio_analysis: &AudioAnalysis,
    ) -> u32 {
        // Simple cycling through available clip indices (1 to num_clips)
        let mut next_clip = (current_clip % num_clips) + 1;

        // Consider energy levels for clip selection
        let energy = audio_analysis.average_energy_in_range(
            beat.time - 1.0,
            beat.time + 1.0
        );

        // High energy sections: prefer later clips in sequence
        // Low energy sections: prefer earlier clips in sequence
        if energy > 0.7 && num_clips > 2 {
            next_clip = ((num_clips + 1) / 2..=num_clips)
                .cycle()
                .nth(current_clip as usize % 3)
                .unwrap_or(next_clip);
        } else if energy < 0.3 && num_clips > 1 {
            next_clip = (1..=(num_clips / 2).max(1))
                .cycle()
                .nth(current_clip as usize % 2)
                .unwrap_or(next_clip);
        }

        next_clip
    }

    /// Ensure the timeline has minimum coverage of the audio
    fn ensure_minimum_coverage(
        &self,
        timeline: &mut CompositionTimeline,
        duration: f64,
        num_clips: u32,
    ) {
        // If we have very few cuts, add some evenly spaced ones
        if timeline.cuts.len() < 3 && duration > 10.0 {
            let segments_needed = (duration / 8.0).ceil() as usize; // Aim for ~8s segments

            for i in 1..segments_needed {
                let cut_time = (i as f64 * duration) / segments_needed as f64;

                // Only add if not too close to existing cuts
                if !timeline.cuts.iter().any(|&t| (t - cut_time).abs() < 2.0) {
                    let clip = ((i as u32 - 1) % num_clips) + 1;
                    timeline.add_cut(cut_time, clip);
                }
            }

            timeline.sort_cuts();
            debug!("Added {} coverage cuts for better distribution",
                   segments_needed - 1);
        }
    }

    // ==========================================
    // PIPELINE STEP 4: VIDEO PROCESSING
    // ==========================================

    /// Process video clips with retro effects according to timeline
    async fn process_video_with_effects(
        &self,
        video_sequence: &VideoSequence,
        timeline: &CompositionTimeline,
        audio_analysis: &AudioAnalysis,
    ) -> Result<Vec<crate::video::ProcessedSegment>> {
        info!("🎨 Step 4: Processing video with {} style...", self.style.name());

        // Initialize video processor
        let mut processor = VideoProcessor::new(self.config.video.params.clone())
            .map_err(|e| CompositionError::SequencingFailed {
                reason: format!("Failed to initialize video processor: {}", e)
            })?;

        // Convert VideoSequence to Vec<VideoClip> for compatibility
        let clips: Vec<VideoClip> = video_sequence.clips().to_vec();

        // Map timeline clip IDs to actual available clips
        let mut mapped_timeline = timeline.clone();
        self.map_timeline_to_available_clips(&mut mapped_timeline, &clips);

        // Process the timeline
        let processed_segments = processor.process_timeline(
            &mapped_timeline,
            &clips,
            self.style.as_ref(),
            &self.config.style,
            audio_analysis.duration,
        ).await.map_err(|e| CompositionError::SequencingFailed {
            reason: format!("Video processing failed: {}", e)
        })?;

        info!("   ✅ Video processing complete:");
        info!("      Segments processed: {}", processed_segments.len());
        info!("      Total frames: {}", 
              processed_segments.iter().map(|s| s.frames.len()).sum::<usize>());
        info!("      Style applied: {}", self.style.name());

        Ok(processed_segments)
    }

    /// Map timeline clip IDs to actual available clip sequence numbers
    fn map_timeline_to_available_clips(
        &self,
        timeline: &mut CompositionTimeline,
        clips: &[VideoClip],
    ) {
        if clips.is_empty() {
            return;
        }

        // Get sorted list of available sequence numbers
        let mut available_sequences: Vec<u32> = clips.iter()
            .map(|c| c.sequence_number)
            .collect();
        available_sequences.sort();

        debug!("Available clip sequences: {:?}", available_sequences);
        debug!("Original timeline assignments: {:?}", timeline.clip_assignments);

        // Map each timeline assignment to an available clip
        for assignment in timeline.clip_assignments.iter_mut() {
            // Convert 1-based timeline index to 0-based array index
            let clip_index = ((*assignment - 1) % available_sequences.len() as u32) as usize;
            *assignment = available_sequences[clip_index];
        }

        debug!("Mapped timeline assignments: {:?}", timeline.clip_assignments);
    }

    // ==========================================
    // PIPELINE STEP 5: OUTPUT GENERATION
    // ==========================================

    /// Generate the final output video file
    async fn generate_final_output(
        &self,
        processed_segments: &[crate::video::ProcessedSegment],
        audio_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        info!("🎬 Step 5: Generating final output...");

        // Initialize video compositor
        let mut compositor = VideoCompositor::new(self.config.video.params.clone());

        // Compose the final video
        let encoded_video = compositor.compose_video(
            processed_segments,
            audio_path,
            output_path,
        ).await.map_err(|e| CompositionError::OutputFailed {
            reason: format!("Video composition failed: {}", e)
        })?;

        info!("   ✅ Output generation complete:");
        info!("      File saved: {:?}", output_path);
        info!("      Duration: {:.1}s", encoded_video.duration);
        info!("      Frame count: {}", encoded_video.frame_count);
        info!("      File size: {:.1} MB", encoded_video.file_size as f64 / 1024.0 / 1024.0);

        // Clean up temporary files
        compositor.cleanup().map_err(|e| CompositionError::OutputFailed {
            reason: format!("Cleanup failed: {}", e)
        })?;

        Ok(())
    }
}

// ==========================================
// TIMELINE DATA STRUCTURES
// ==========================================

/// Represents the composition timeline with cut points and clip assignments
#[derive(Debug, Clone)]
pub struct CompositionTimeline {
    /// Cut points in seconds (sorted)
    pub cuts: Vec<f64>,

    /// Which clip to use for each segment (aligned with cuts)
    pub clip_assignments: Vec<u32>,
}

impl CompositionTimeline {
    /// Create a new empty timeline
    pub fn new() -> Self {
        Self {
            cuts: Vec::new(),
            clip_assignments: Vec::new(),
        }
    }

    /// Add a cut point with clip assignment
    pub fn add_cut(&mut self, time: f64, clip_id: u32) {
        self.cuts.push(time);
        self.clip_assignments.push(clip_id);
    }

    /// Sort cuts by time (maintaining clip assignment alignment)
    pub fn sort_cuts(&mut self) {
        let mut paired: Vec<(f64, u32)> = self.cuts
            .iter()
            .zip(self.clip_assignments.iter())
            .map(|(&time, &clip)| (time, clip))
            .collect();

        paired.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        self.cuts = paired.iter().map(|&(time, _)| time).collect();
        self.clip_assignments = paired.iter().map(|&(_, clip)| clip).collect();
    }

    /// Get unique clips used in this timeline
    pub fn unique_clips(&self) -> Vec<u32> {
        let mut clips = self.clip_assignments.clone();
        clips.sort_unstable();
        clips.dedup();
        clips
    }

    /// Get the duration of a segment at the given index
    pub fn segment_duration(&self, index: usize, total_duration: f64) -> f64 {
        if index >= self.cuts.len() {
            return 0.0;
        }

        let start = self.cuts[index];
        let end = self.cuts.get(index + 1).copied().unwrap_or(total_duration);

        end - start
    }
}

impl Default for CompositionTimeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{styles::VhsStyle, config::Config};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_empty_video_directory() {
        let config = Config::default();
        let style = Box::new(VhsStyle::new());
        let engine = CompositionEngine::new(config, style);

        let temp_dir = tempdir().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        std::fs::create_dir(&empty_dir).unwrap();

        let result = engine.load_video_clips(&empty_dir).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_timeline_operations() {
        let mut timeline = CompositionTimeline::new();

        // Add cuts out of order
        timeline.add_cut(2.0, 2);
        timeline.add_cut(1.0, 1);
        timeline.add_cut(3.0, 3);

        // Sort and verify
        timeline.sort_cuts();
        assert_eq!(timeline.cuts, vec![1.0, 2.0, 3.0]);
        assert_eq!(timeline.clip_assignments, vec![1, 2, 3]);

        // Test unique clips
        timeline.add_cut(4.0, 1); // Duplicate clip
        let unique = timeline.unique_clips();
        assert_eq!(unique, vec![1, 2, 3]);

        // Test segment duration
        let duration = timeline.segment_duration(0, 10.0);
        assert_eq!(duration, 1.0); // 2.0 - 1.0
    }

    #[test]
    fn test_cut_decision_logic() {
        let config = Config::default();
        let style = Box::new(VhsStyle::new());
        let engine = CompositionEngine::new(config, style);

        // Create a test beat
        let beat = crate::audio::types::Beat {
            time: 5.0,
            strength: 0.8,
            beat_type: crate::audio::types::BeatType::Downbeat,
            onset_value: 0.7,
            local_energy: 0.6,
        };

        // Test minimum interval constraint
        let should_cut = engine.should_cut_at_beat(&beat, 0.5, &create_test_analysis());
        assert!(!should_cut, "Should not cut below minimum interval");

        // Test normal cutting decision
        let should_cut = engine.should_cut_at_beat(&beat, 3.0, &create_test_analysis());
        // Result depends on beat strength and configuration
    }

    fn create_test_analysis() -> AudioAnalysis {
        use crate::audio::types::*;

        AudioAnalysis {
            beats: vec![],
            tempo: TempoMap {
                global_bpm: 120.0,
                confidence: 0.8,
                tempo_changes: vec![],
                time_signature: TimeSignature::default(),
            },
            energy_levels: vec![
                EnergyLevel {
                    time: 4.5,
                    rms: 0.6,
                    peak: 0.8,
                    spectral_centroid: 1000.0,
                    zero_crossing_rate: 0.1,
                },
                EnergyLevel {
                    time: 5.5,
                    rms: 0.6,
                    peak: 0.8,
                    spectral_centroid: 1000.0,
                    zero_crossing_rate: 0.1,
                },
            ],
            bpm: 120.0,
            bpm_confidence: 0.8,
            duration: 10.0,
            config: AnalysisConfig::default(),
            phrases: vec![],
            spectral_features: SpectralFeatures {
                mfcc: vec![],
                spectral_centroid: vec![],
                spectral_rolloff: vec![],
                chroma: vec![],
                onset_detection_function: vec![],
            },
        }
    }
}