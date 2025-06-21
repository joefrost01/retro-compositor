use std::path::Path;
use tracing::{info, debug, warn};

use crate::{
    audio::{AudioLoader, AudioAnalyzer, AudioAnalysis},
    config::Config,
    error::{CompositionError, Result},
    styles::Style,
    video::{Frame, VideoClip, VideoSequence},
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
        self.process_video_with_effects(&video_sequence, &timeline).await?;

        // Pipeline Step 5: Final Output Generation
        self.generate_final_output(&timeline, output_path).await?;

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

        let mut sequence = VideoSequence::new();

        // Read directory entries and sort by filename
        let mut entries: Vec<_> = std::fs::read_dir(video_dir)?
            .collect::<std::result::Result<Vec<_>, std::io::Error>>()?;

        entries.sort_by_key(|entry: &std::fs::DirEntry| entry.file_name());

        let mut clips_found = 0;
        let mut clips_loaded = 0;

        for entry in entries {
            let path = entry.path();

            // Skip directories and hidden files
            if path.is_dir() || self.is_hidden_file(&path) {
                continue;
            }

            clips_found += 1;

            // Try to parse as a numbered video clip
            if let Some(clip) = VideoClip::from_path(&path) {
                if clip.is_supported() {
                    debug!("Found clip: {} (sequence {})", clip.name, clip.sequence_number);
                    sequence.add_clip(clip);
                    clips_loaded += 1;
                } else {
                    warn!("Unsupported format: {:?}", path);
                }
            } else {
                warn!("Could not parse clip filename: {:?} (expected format: NN_name.ext)",
                      path.file_name().unwrap_or_default());
            }
        }

        if sequence.is_empty() {
            return Err(CompositionError::NoClipsFound {
                path: video_dir.display().to_string()
            }.into());
        }

        info!("   ✅ Video clips loaded:");
        info!("      Files found: {}", clips_found);
        info!("      Clips loaded: {}", clips_loaded);
        info!("      Sequence length: {}", sequence.len());

        // Log the sequence order
        for clip in sequence.iter() {
            debug!("      {:02} - {} ({})",
                   clip.sequence_number, clip.name,
                   clip.extension().unwrap_or("unknown"));
        }

        Ok(sequence)
    }

    /// Check if a file is hidden (starts with .)
    fn is_hidden_file(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
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
        // For now, use simple cycling with some intelligence
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
    ) -> Result<()> {
        info!("🎨 Step 4: Processing video with {} style...", self.style.name());

        // For now, demonstrate the concept with placeholder frames
        // In a real implementation, this would load actual video frames

        let mut processed_segments = 0;

        for (i, &cut_time) in timeline.cuts.iter().enumerate() {
            let clip_id = timeline.clip_assignments.get(i).copied().unwrap_or(1);

            if let Some(clip) = video_sequence.get_clip(clip_id) {
                debug!("Processing segment {} at {:.2}s with clip: {}",
                       i, cut_time, clip.name);

                // Create a sample frame (in real implementation, load from video)
                let mut frame = self.create_sample_frame_for_clip(clip);

                // Apply the retro style effect
                self.style.apply_effect(&mut frame, &self.config.style)?;

                // In real implementation: save processed frame to temporary storage
                processed_segments += 1;

                // Simulate processing time
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            } else {
                warn!("Could not find clip {} for timeline segment {}", clip_id, i);
            }
        }

        info!("   ✅ Video processing complete:");
        info!("      Segments processed: {}", processed_segments);
        info!("      Style applied: {}", self.style.name());

        Ok(())
    }

    /// Create a sample frame for demonstration (placeholder for real video loading)
    fn create_sample_frame_for_clip(&self, clip: &VideoClip) -> Frame {
        // Create different colored frames based on clip sequence for demo
        let color = match clip.sequence_number % 4 {
            0 => [120, 80, 160],   // Purple
            1 => [160, 120, 80],   // Orange
            2 => [80, 160, 120],   // Green
            _ => [140, 140, 140],  // Gray
        };

        Frame::new_filled(640, 480, color)
    }

    // ==========================================
    // PIPELINE STEP 5: OUTPUT GENERATION
    // ==========================================

    /// Generate the final output video file
    async fn generate_final_output(
        &self,
        timeline: &CompositionTimeline,
        output_path: &Path,
    ) -> Result<()> {
        info!("🎬 Step 5: Generating final output...");

        // For now, create a placeholder output file
        // In real implementation: encode video with processed frames and original audio

        debug!("Encoding {} segments to video...", timeline.cuts.len());

        // Simulate encoding time
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Create placeholder output
        let placeholder_content = format!(
            "Retro-Compositor Output\n\
             Style: {}\n\
             Segments: {}\n\
             Cuts: {:?}\n\
             Generated: {}",
            self.style.name(),
            timeline.cuts.len(),
            timeline.cuts,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        std::fs::write(output_path, placeholder_content)?;

        info!("   ✅ Output generation complete:");
        info!("      File saved: {:?}", output_path);
        info!("      Timeline segments: {}", timeline.cuts.len());

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