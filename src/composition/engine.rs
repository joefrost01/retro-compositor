// src/composition/engine.rs - Improved video selection logic

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
pub struct CompositionEngine {
    config: Config,
    style: Box<dyn Style>,
}

impl CompositionEngine {
    pub fn new(config: Config, style: Box<dyn Style>) -> Self {
        Self { config, style }
    }

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

    // Audio analysis (unchanged)
    async fn analyze_audio(&self, audio_path: &Path) -> Result<AudioAnalysis> {
        info!("🎵 Step 1: Analyzing audio file...");

        debug!("Loading audio from: {:?}", audio_path);
        let audio_data = AudioLoader::load(audio_path).await
            .map_err(|e| {
                warn!("Failed to load audio file: {}", e);
                e
            })?;

        info!("   Loaded: {:.1}s, {} Hz, {} channels",
              audio_data.duration, audio_data.sample_rate, audio_data.channels);

        let analysis_config = crate::audio::types::AnalysisConfig {
            window_size: self.config.audio.window_size,
            hop_size: self.config.audio.hop_size,
            min_bpm: self.config.audio.min_bpm,
            max_bpm: self.config.audio.max_bpm,
            beat_sensitivity: self.config.audio.beat_sensitivity,
            energy_window_size: 0.1,
            detect_phrases: true,
            calculate_spectral_features: true,
        };

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

    // Video loading (unchanged)
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

        for clip in sequence.iter() {
            debug!("      {:02} - {} ({})",
                   clip.sequence_number, clip.name,
                   clip.extension().unwrap_or("unknown"));
        }

        Ok(sequence)
    }

    // **IMPROVED TIMELINE GENERATION** - Better video selection
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
        let available_clips: Vec<u32> = video_sequence.iter()
            .map(|clip| clip.sequence_number)
            .collect();

        debug!("Available clips: {:?}", available_clips);
        debug!("Processing {} beats", audio_analysis.beats.len());

        // **IMPROVED ALGORITHM**: Use all available clips with smart rotation
        timeline.add_cut(0.0, available_clips[0]);

        let mut clip_rotation_index = 0;
        let mut last_cut_time = 0.0;
        let mut segment_count = 0;

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
                // **SMART VIDEO SELECTION** - Rotate through ALL available clips
                clip_rotation_index = (clip_rotation_index + 1) % available_clips.len();
                let selected_clip = available_clips[clip_rotation_index];

                timeline.add_cut(beat.time, selected_clip);
                last_cut_time = beat.time;
                segment_count += 1;

                debug!("Cut {} at {:.2}s -> Clip {} (beat strength: {:.2})",
                       segment_count, beat.time, selected_clip, beat.strength);
            }
        }

        // **ENSURE GOOD DISTRIBUTION** - Add clips that haven't been used enough
        self.ensure_clip_distribution(&mut timeline, &available_clips, audio_analysis.duration);

        info!("   ✅ Timeline generated:");
        info!("      Total cuts: {}", timeline.cuts.len());
        info!("      Average segment: {:.1}s",
              audio_analysis.duration / timeline.cuts.len() as f64);
        info!("      Clips used: {:?}", timeline.unique_clips());

        Ok(timeline)
    }

    /// Determine if we should cut at this beat (improved logic)
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

        // **IMPROVED PROBABILITY CALCULATION**
        let mut cut_probability = 0.0;

        // Beat strength factor (stronger beats more likely to cut)
        cut_probability += beat.strength * 0.5;

        // Energy factor (high energy sections more likely to cut)
        let local_energy = audio_analysis.average_energy_in_range(
            beat.time - 0.5,
            beat.time + 0.5
        );
        cut_probability += local_energy * 0.4;

        // Beat type factor (downbeats more likely to cut)
        if beat.beat_type == crate::audio::types::BeatType::Downbeat {
            cut_probability += 0.3;
        }

        // Time factor (encourage cuts at reasonable intervals)
        let ideal_interval = (config.min_cut_interval + config.max_cut_interval) / 2.0;
        let time_factor = if time_since_last_cut >= ideal_interval {
            0.2 + (time_since_last_cut - ideal_interval) / ideal_interval * 0.3
        } else {
            0.1
        } as f32; // Convert to f32
        cut_probability += time_factor;

        // Apply beat sync strength from configuration
        cut_probability *= config.beat_sync_strength;

        // **LOWER THRESHOLD** for more frequent cuts and better video distribution
        cut_probability >= 0.4
    }

    /// Ensure all clips get used and good distribution
    fn ensure_clip_distribution(
        &self,
        timeline: &mut CompositionTimeline,
        available_clips: &[u32],
        duration: f64,
    ) {
        let clips_used = timeline.unique_clips();
        let unused_clips: Vec<u32> = available_clips.iter()
            .filter(|&&clip| !clips_used.contains(&clip))
            .copied()
            .collect();

        debug!("Clips used: {:?}", clips_used);
        debug!("Unused clips: {:?}", unused_clips);

        // If we have unused clips and not too many cuts already, add some strategic cuts
        if !unused_clips.is_empty() && timeline.cuts.len() < (duration / 3.0) as usize {
            let segments_to_add = unused_clips.len().min(3); // Don't add too many

            for (i, &unused_clip) in unused_clips.iter().take(segments_to_add).enumerate() {
                // Add cuts at strategic points
                let strategic_time = duration * (0.3 + i as f64 * 0.2);

                // Only add if not too close to existing cuts
                if !timeline.cuts.iter().any(|&t| (t - strategic_time).abs() < 2.0) {
                    timeline.add_cut(strategic_time, unused_clip);
                    debug!("Added strategic cut at {:.1}s for unused clip {}", strategic_time, unused_clip);
                }
            }

            timeline.sort_cuts();
        }

        // **FINAL DISTRIBUTION CHECK** - Make sure we're using variety
        if clips_used.len() < available_clips.len() / 2 {
            warn!("Only using {}/{} available clips - consider adjusting beat sensitivity", 
                  clips_used.len(), available_clips.len());
        }
    }

    // Video processing (unchanged but with better logging)
    async fn process_video_with_effects(
        &self,
        video_sequence: &VideoSequence,
        timeline: &CompositionTimeline,
        audio_analysis: &AudioAnalysis,
    ) -> Result<Vec<crate::video::ProcessedSegment>> {
        info!("🎨 Step 4: Processing video with {} style...", self.style.name());

        let mut processor = VideoProcessor::new(self.config.video.params.clone())
            .map_err(|e| CompositionError::SequencingFailed {
                reason: format!("Failed to initialize video processor: {}", e)
            })?;

        let clips: Vec<VideoClip> = video_sequence.clips().to_vec();
        let mut mapped_timeline = timeline.clone();
        self.map_timeline_to_available_clips(&mut mapped_timeline, &clips);

        // **ENHANCED STYLE CONFIG** for more obvious effects
        let mut enhanced_style_config = self.config.style.clone();
        enhanced_style_config.intensity = 0.9; // Increase intensity

        // Add VHS-specific enhancements
        enhanced_style_config = enhanced_style_config
            .set("scanline_intensity", 0.9)
            .set("color_bleeding", 0.8)
            .set("noise_level", 0.6)
            .set("tracking_error", 0.5)
            .set("chroma_shift", 0.7);

        info!("   Using enhanced {} style with intensity {:.1}", 
              self.style.name(), enhanced_style_config.intensity);

        let processed_segments = processor.process_timeline(
            &mapped_timeline,
            &clips,
            self.style.as_ref(),
            &enhanced_style_config,
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

    fn map_timeline_to_available_clips(
        &self,
        timeline: &mut CompositionTimeline,
        clips: &[VideoClip],
    ) {
        if clips.is_empty() {
            return;
        }

        let mut available_sequences: Vec<u32> = clips.iter()
            .map(|c| c.sequence_number)
            .collect();
        available_sequences.sort();

        debug!("Available clip sequences: {:?}", available_sequences);
        debug!("Original timeline assignments: {:?}", timeline.clip_assignments);

        // **IMPROVED MAPPING** - Direct mapping instead of modulo
        for assignment in timeline.clip_assignments.iter_mut() {
            // If the requested clip exists, use it; otherwise map to available clips
            if !available_sequences.contains(assignment) {
                let clip_index = (*assignment as usize - 1) % available_sequences.len();
                *assignment = available_sequences[clip_index];
            }
        }

        debug!("Mapped timeline assignments: {:?}", timeline.clip_assignments);
    }

    // Output generation (unchanged)
    async fn generate_final_output(
        &self,
        processed_segments: &[crate::video::ProcessedSegment],
        audio_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        info!("🎬 Step 5: Generating final output...");

        let mut compositor = VideoCompositor::new(self.config.video.params.clone());

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

        compositor.cleanup().map_err(|e| CompositionError::OutputFailed {
            reason: format!("Cleanup failed: {}", e)
        })?;

        Ok(())
    }
}

// Timeline data structures (unchanged)
#[derive(Debug, Clone)]
pub struct CompositionTimeline {
    pub cuts: Vec<f64>,
    pub clip_assignments: Vec<u32>,
}

impl CompositionTimeline {
    pub fn new() -> Self {
        Self {
            cuts: Vec::new(),
            clip_assignments: Vec::new(),
        }
    }

    pub fn add_cut(&mut self, time: f64, clip_id: u32) {
        self.cuts.push(time);
        self.clip_assignments.push(clip_id);
    }

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

    pub fn unique_clips(&self) -> Vec<u32> {
        let mut clips = self.clip_assignments.clone();
        clips.sort_unstable();
        clips.dedup();
        clips
    }

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