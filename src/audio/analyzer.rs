use std::collections::VecDeque;

use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

use crate::audio::types::{
    AudioData, AudioAnalysis, Beat, BeatType, EnergyLevel,
    TempoMap, TimeSignature, Phrase, PhraseType, SpectralFeatures,
    AnalysisConfig, TempoChange
};
use crate::error::{AudioError, Result};

/// Core audio analyzer implementing FFT-based beat detection and tempo analysis
pub struct AudioAnalyzer {
    config: AnalysisConfig,
}

impl AudioAnalyzer {
    /// Create a new analyzer with default configuration
    pub fn new() -> Self {
        Self::with_config(AnalysisConfig::default())
    }

    /// Create a new analyzer with custom configuration
    pub fn with_config(config: AnalysisConfig) -> Self {
        Self { config }
    }

    /// Perform comprehensive audio analysis
    pub async fn analyze(&self, audio_data: &AudioData) -> Result<AudioAnalysis> {
        // Validate configuration
        self.config.validate()
            .map_err(|e| AudioError::InvalidParameters { details: e })?;

        tracing::info!("Starting audio analysis for {} seconds of audio", audio_data.duration);
        tracing::info!("Sample rate: {} Hz, Channels: {}", audio_data.sample_rate, audio_data.channels);

        // Get mono samples for analysis
        let mono_samples = audio_data.mono_samples();

        // Step 1: Calculate energy levels over time
        tracing::debug!("Calculating energy levels...");
        let energy_levels = self.calculate_energy_levels(&mono_samples, audio_data.sample_rate)?;

        // Step 2: Onset detection using spectral flux
        tracing::debug!("Performing onset detection...");
        let (onsets, onset_detection_function) = self.detect_onsets(&mono_samples, audio_data.sample_rate)?;

        // Step 3: Beat tracking from onsets
        tracing::debug!("Tracking beats from onsets...");
        let beats = self.track_beats(&onsets, &energy_levels)?;

        // Step 4: Tempo estimation
        tracing::debug!("Estimating tempo...");
        let tempo = self.estimate_tempo(&beats, audio_data.duration)?;

        // Step 5: Optional spectral features
        let spectral_features = if self.config.calculate_spectral_features {
            tracing::debug!("Calculating spectral features...");
            self.calculate_spectral_features(&mono_samples, audio_data.sample_rate)?
        } else {
            SpectralFeatures {
                mfcc: vec![],
                spectral_centroid: vec![],
                spectral_rolloff: vec![],
                chroma: vec![],
                onset_detection_function,
            }
        };

        // Step 6: Optional phrase detection
        let phrases = if self.config.detect_phrases {
            tracing::debug!("Detecting musical phrases...");
            self.detect_phrases(&beats, &energy_levels, audio_data.duration)?
        } else {
            vec![]
        };

        tracing::info!(
            "Analysis complete: {} beats detected, BPM: {:.1}, confidence: {:.2}",
            beats.len(),
            tempo.global_bpm,
            tempo.confidence
        );

        Ok(AudioAnalysis {
            beats,
            tempo: tempo.clone(),
            energy_levels,
            bpm: tempo.global_bpm,
            bpm_confidence: tempo.confidence,
            duration: audio_data.duration,
            config: self.config.clone(),
            phrases,
            spectral_features,
        })
    }

    /// Calculate RMS energy levels over time using sliding windows
    fn calculate_energy_levels(&self, samples: &[f32], sample_rate: u32) -> Result<Vec<EnergyLevel>> {
        let window_samples = (self.config.energy_window_size * sample_rate as f64) as usize;
        let hop_samples = window_samples / 2; // 50% overlap

        let mut energy_levels = Vec::new();

        for (i, window) in samples.windows(window_samples).step_by(hop_samples).enumerate() {
            let time = (i * hop_samples) as f64 / sample_rate as f64;

            // Calculate RMS energy
            let rms = (window.iter().map(|&x| x * x).sum::<f32>() / window.len() as f32).sqrt();

            // Calculate peak energy
            let peak = window.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);

            // Calculate zero crossing rate
            let zero_crossings = window
                .windows(2)
                .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
                .count();
            let zero_crossing_rate = zero_crossings as f32 / window.len() as f32;

            // Spectral centroid (simplified - would need FFT for full implementation)
            let spectral_centroid = rms * 1000.0; // Placeholder for now

            energy_levels.push(EnergyLevel {
                time,
                rms,
                peak,
                spectral_centroid,
                zero_crossing_rate,
            });
        }

        Ok(energy_levels)
    }

    /// Detect onsets using spectral flux method
    fn detect_onsets(&self, samples: &[f32], sample_rate: u32) -> Result<(Vec<f64>, Vec<f32>)> {
        // Create a new FFT planner for this analysis
        let mut planner = RealFftPlanner::new();
        let fft = planner.plan_fft_forward(self.config.window_size);
        let mut spectrum_buffer = fft.make_output_vec();
        let mut input_buffer = fft.make_input_vec();

        let mut previous_magnitude = vec![0.0f32; self.config.window_size / 2 + 1];
        let mut spectral_flux = Vec::new();
        let mut onsets = Vec::new();

        let mut max_flux = 0.0f32;
        let mut flux_values = Vec::new();

        // Process audio in windows
        for (frame_idx, window) in samples
            .windows(self.config.window_size)
            .step_by(self.config.hop_size)
            .enumerate()
        {
            // Apply window function (Hann window)
            for (i, &sample) in window.iter().enumerate() {
                let window_val = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (self.config.window_size - 1) as f32).cos());
                input_buffer[i] = sample * window_val;
            }

            // Zero-pad if necessary
            if window.len() < self.config.window_size {
                for i in window.len()..self.config.window_size {
                    input_buffer[i] = 0.0;
                }
            }

            // Perform FFT
            fft.process(&mut input_buffer, &mut spectrum_buffer)
                .map_err(|_| AudioError::AnalysisFailed {
                    reason: "FFT processing failed".to_string()
                })?;

            // Calculate magnitude spectrum
            let current_magnitude: Vec<f32> = spectrum_buffer
                .iter()
                .map(|&c| c.norm())
                .collect();

            // Calculate spectral flux (sum of positive differences)
            let flux: f32 = current_magnitude
                .iter()
                .zip(previous_magnitude.iter())
                .map(|(&curr, &prev)| (curr - prev).max(0.0))
                .sum();

            spectral_flux.push(flux);
            flux_values.push(flux);
            max_flux = max_flux.max(flux);

            // Update previous magnitude
            previous_magnitude.copy_from_slice(&current_magnitude);

            // Calculate time for this frame
            let time = (frame_idx * self.config.hop_size) as f64 / sample_rate as f64;

            // Simple onset detection: local maxima above threshold
            if frame_idx > 3 && frame_idx < spectral_flux.len() - 3 {
                let window_start = frame_idx.saturating_sub(3);
                let window_end = (frame_idx + 3).min(spectral_flux.len());

                let local_max = spectral_flux[window_start..window_end]
                    .iter()
                    .fold(0.0f32, |acc, &x| acc.max(x));

                // Calculate adaptive threshold based on local statistics
                let local_mean = spectral_flux[window_start..window_end]
                    .iter()
                    .sum::<f32>() / (window_end - window_start) as f32;

                let threshold = local_mean + (self.config.beat_sensitivity * (local_max - local_mean) * 0.5);

                if flux >= threshold && flux == local_max && flux > local_mean * 1.5 {
                    onsets.push(time);
                }
            }
        }

        // Debug output
        tracing::debug!(
            "Spectral flux analysis: {} frames, max flux: {:.3}, {} onsets detected",
            spectral_flux.len(), max_flux, onsets.len()
        );

        // If no onsets detected with adaptive method, try a simpler approach
        if onsets.is_empty() && !flux_values.is_empty() {
            tracing::debug!("No onsets with adaptive method, trying simple threshold...");

            // Calculate global statistics
            let mean_flux = flux_values.iter().sum::<f32>() / flux_values.len() as f32;
            let simple_threshold = mean_flux * (2.0 + self.config.beat_sensitivity);

            for (frame_idx, &flux) in flux_values.iter().enumerate() {
                if flux > simple_threshold {
                    let time = (frame_idx * self.config.hop_size) as f64 / sample_rate as f64;
                    onsets.push(time);
                }
            }

            tracing::debug!(
                "Simple threshold {:.3} (mean: {:.3}) detected {} onsets",
                simple_threshold, mean_flux, onsets.len()
            );
        }

        tracing::debug!("Detected {} onset candidates", onsets.len());
        Ok((onsets, spectral_flux))
    }

    /// Track beats from detected onsets
    fn track_beats(&self, onsets: &[f64], energy_levels: &[EnergyLevel]) -> Result<Vec<Beat>> {
        let mut beats = Vec::new();

        // If we have onsets, use them
        if !onsets.is_empty() {
            beats = self.track_beats_from_onsets(onsets, energy_levels)?;
        }

        // If no beats from onsets, try energy-based detection as fallback
        if beats.is_empty() && !energy_levels.is_empty() {
            tracing::debug!("No beats from onsets, trying energy-based detection...");
            beats = self.track_beats_from_energy(energy_levels)?;
        }

        tracing::debug!("Generated {} beats from {} onsets", beats.len(), onsets.len());
        Ok(beats)
    }

    /// Track beats from detected onsets
    fn track_beats_from_onsets(&self, onsets: &[f64], energy_levels: &[EnergyLevel]) -> Result<Vec<Beat>> {
        let mut beats = Vec::new();

        // Filter onsets to remove those too close together
        let min_beat_interval = 60.0 / self.config.max_bpm as f64; // Minimum time between beats
        let mut filtered_onsets = Vec::new();
        let mut last_onset_time = -1.0;

        for &onset_time in onsets {
            if onset_time - last_onset_time >= min_beat_interval {
                filtered_onsets.push(onset_time);
                last_onset_time = onset_time;
            }
        }

        // Convert filtered onsets to beats with additional metadata
        for (i, &time) in filtered_onsets.iter().enumerate() {
            // Find energy level at this time
            let local_energy = energy_levels
                .iter()
                .min_by(|a, b| (a.time - time).abs().partial_cmp(&(b.time - time).abs()).unwrap())
                .map(|e| e.rms)
                .unwrap_or(0.0);

            // Calculate beat strength based on local energy and onset prominence
            let strength = (local_energy * 2.0).min(1.0);

            // Simple beat type classification (every 4th beat is a downbeat)
            let beat_type = if i % 4 == 0 {
                BeatType::Downbeat
            } else {
                BeatType::Beat
            };

            beats.push(Beat {
                time,
                strength,
                beat_type,
                onset_value: strength, // Using strength as onset value for now
                local_energy,
            });
        }

        Ok(beats)
    }

    /// Fallback: Track beats from energy levels when onset detection fails
    fn track_beats_from_energy(&self, energy_levels: &[EnergyLevel]) -> Result<Vec<Beat>> {
        let mut beats = Vec::new();

        if energy_levels.len() < 10 {
            return Ok(beats);
        }

        // Calculate energy statistics
        let energies: Vec<f32> = energy_levels.iter().map(|e| e.rms).collect();
        let mean_energy = energies.iter().sum::<f32>() / energies.len() as f32;
        let max_energy = energies.iter().fold(0.0f32, |acc, &x| acc.max(x));

        // Look for energy peaks
        let energy_threshold = mean_energy + (max_energy - mean_energy) * 0.3;

        tracing::debug!(
            "Energy-based detection: mean={:.3}, max={:.3}, threshold={:.3}",
            mean_energy, max_energy, energy_threshold
        );

        let min_beat_interval = 60.0 / self.config.max_bpm as f64;
        let mut last_beat_time = -1.0;

        for (i, energy) in energy_levels.iter().enumerate() {
            // Look for local energy maxima above threshold
            if energy.rms > energy_threshold {
                let window_start = i.saturating_sub(2);
                let window_end = (i + 2).min(energy_levels.len());

                let is_local_max = energy_levels[window_start..window_end]
                    .iter()
                    .all(|e| e.rms <= energy.rms);

                if is_local_max && energy.time - last_beat_time >= min_beat_interval {
                    let strength = ((energy.rms - mean_energy) / (max_energy - mean_energy)).min(1.0);

                    let beat_type = if beats.len() % 4 == 0 {
                        BeatType::Downbeat
                    } else {
                        BeatType::Beat
                    };

                    beats.push(Beat {
                        time: energy.time,
                        strength,
                        beat_type,
                        onset_value: energy.rms,
                        local_energy: energy.rms,
                    });

                    last_beat_time = energy.time;
                }
            }
        }

        tracing::debug!("Energy-based detection found {} beats", beats.len());
        Ok(beats)
    }

    /// Estimate tempo using inter-beat interval analysis
    fn estimate_tempo(&self, beats: &[Beat], duration: f64) -> Result<TempoMap> {
        if beats.len() < 2 {
            return Ok(TempoMap {
                global_bpm: 120.0, // Default fallback
                confidence: 0.1,
                tempo_changes: vec![],
                time_signature: TimeSignature::default(),
            });
        }

        // Calculate inter-beat intervals
        let intervals: Vec<f64> = beats
            .windows(2)
            .map(|pair| pair[1].time - pair[0].time)
            .filter(|&interval| {
                let bpm = 60.0 / interval;
                bpm >= self.config.min_bpm as f64 && bpm <= self.config.max_bpm as f64
            })
            .collect();

        if intervals.is_empty() {
            return Ok(TempoMap {
                global_bpm: 120.0,
                confidence: 0.1,
                tempo_changes: vec![],
                time_signature: TimeSignature::default(),
            });
        }

        // Find the most common interval using histogram approach
        let mut interval_counts = std::collections::HashMap::new();

        for &interval in &intervals {
            // Quantize intervals to nearest 1ms for grouping (convert to integer key)
            let quantized_ms = (interval * 1000.0).round() as i64;
            *interval_counts.entry(quantized_ms).or_insert(0) += 1;
        }

        // Find the most frequent interval
        let most_common_interval_ms = interval_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&interval_ms, _)| interval_ms)
            .unwrap_or(500); // 120 BPM fallback (500ms)

        let most_common_interval = most_common_interval_ms as f64 / 1000.0;

        let global_bpm = 60.0 / most_common_interval;

        // Calculate confidence based on how many intervals match the dominant tempo
        let matching_intervals = intervals
            .iter()
            .filter(|&&interval| (interval - most_common_interval).abs() < 0.05)
            .count();

        let confidence = (matching_intervals as f32 / intervals.len() as f32).min(1.0);

        tracing::debug!(
            "Tempo estimation: {:.1} BPM (confidence: {:.2}) from {} intervals",
            global_bpm, confidence, intervals.len()
        );

        Ok(TempoMap {
            global_bpm: global_bpm as f32,
            confidence,
            tempo_changes: vec![], // Future feature
            time_signature: TimeSignature::default(),
        })
    }

    /// Calculate spectral features for advanced analysis
    fn calculate_spectral_features(&self, samples: &[f32], sample_rate: u32) -> Result<SpectralFeatures> {
        // Create a new FFT planner for this analysis
        let mut planner = RealFftPlanner::new();
        let fft = planner.plan_fft_forward(self.config.window_size);
        let mut spectrum_buffer = fft.make_output_vec();
        let mut input_buffer = fft.make_input_vec();

        let mut spectral_centroids = Vec::new();
        let mut spectral_rolloffs = Vec::new();

        // Process audio in windows
        for window in samples
            .windows(self.config.window_size)
            .step_by(self.config.hop_size)
        {
            // Apply window function
            for (i, &sample) in window.iter().enumerate() {
                let window_val = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (self.config.window_size - 1) as f32).cos());
                input_buffer[i] = sample * window_val;
            }

            // Zero-pad if necessary
            if window.len() < self.config.window_size {
                for i in window.len()..self.config.window_size {
                    input_buffer[i] = 0.0;
                }
            }

            // Perform FFT
            fft.process(&mut input_buffer, &mut spectrum_buffer)
                .map_err(|_| AudioError::AnalysisFailed {
                    reason: "FFT processing failed".to_string()
                })?;

            // Calculate magnitude spectrum
            let magnitude: Vec<f32> = spectrum_buffer
                .iter()
                .map(|&c| c.norm())
                .collect();

            // Calculate spectral centroid
            let total_magnitude: f32 = magnitude.iter().sum();
            let weighted_sum: f32 = magnitude
                .iter()
                .enumerate()
                .map(|(i, &mag)| i as f32 * mag)
                .sum();

            let spectral_centroid = if total_magnitude > 0.0 {
                (weighted_sum / total_magnitude) * (sample_rate as f32 / 2.0) / (magnitude.len() as f32)
            } else {
                0.0
            };

            spectral_centroids.push(spectral_centroid);

            // Calculate spectral rolloff (85% of energy)
            let target_energy = total_magnitude * 0.85;
            let mut cumulative_energy = 0.0;
            let mut rolloff_bin = 0;

            for (i, &mag) in magnitude.iter().enumerate() {
                cumulative_energy += mag;
                if cumulative_energy >= target_energy {
                    rolloff_bin = i;
                    break;
                }
            }

            let spectral_rolloff = (rolloff_bin as f32 / magnitude.len() as f32) * (sample_rate as f32 / 2.0);
            spectral_rolloffs.push(spectral_rolloff);
        }

        Ok(SpectralFeatures {
            mfcc: vec![], // MFCC calculation would be more complex
            spectral_centroid: spectral_centroids,
            spectral_rolloff: spectral_rolloffs,
            chroma: vec![], // Chroma features would require additional processing
            onset_detection_function: vec![], // Already calculated in onset detection
        })
    }

    /// Detect musical phrases and sections
    fn detect_phrases(&self, beats: &[Beat], energy_levels: &[EnergyLevel], duration: f64) -> Result<Vec<Phrase>> {
        let mut phrases = Vec::new();

        if beats.is_empty() {
            return Ok(phrases);
        }

        // Simple phrase detection based on energy changes and beat patterns
        let phrase_length = 8.0; // Assume 8-second phrases initially
        let mut current_start = 0.0;

        while current_start < duration {
            let phrase_end = (current_start + phrase_length).min(duration);

            // Determine phrase type based on position and energy
            let phrase_type = if current_start < duration * 0.1 {
                PhraseType::Intro
            } else if current_start > duration * 0.9 {
                PhraseType::Outro
            } else {
                // Simple alternating pattern for demo
                if ((current_start / phrase_length) as usize) % 2 == 0 {
                    PhraseType::Verse
                } else {
                    PhraseType::Chorus
                }
            };

            phrases.push(Phrase {
                start: current_start,
                end: phrase_end,
                phrase_type,
                confidence: 0.6, // Placeholder confidence
            });

            current_start = phrase_end;
        }

        tracing::debug!("Detected {} musical phrases", phrases.len());
        Ok(phrases)
    }
}

impl Default for AudioAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::AudioFormat;
    use std::path::PathBuf;

    fn create_test_audio_data() -> AudioData {
        // Create a simple sine wave for testing
        let sample_rate = 44100;
        let duration = 2.0; // 2 seconds
        let frequency = 440.0; // A note
        let samples: Vec<f32> = (0..(sample_rate as f64 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5
            })
            .collect();

        AudioData {
            samples,
            sample_rate,
            channels: 1,
            duration,
            file_path: PathBuf::from("test.wav"),
            format: AudioFormat {
                extension: "wav".to_string(),
                bit_depth: Some(16),
                compression: None,
                bitrate: None,
            },
        }
    }

    #[tokio::test]
    async fn test_audio_analysis() {
        let audio_data = create_test_audio_data();
        let analyzer = AudioAnalyzer::new();

        let result = analyzer.analyze(&audio_data).await;
        assert!(result.is_ok());

        let analysis = result.unwrap();
        assert_eq!(analysis.duration, 2.0);
        assert!(!analysis.energy_levels.is_empty());
        assert!(analysis.bpm > 0.0);
    }

    #[tokio::test]
    async fn test_energy_calculation() {
        let audio_data = create_test_audio_data();
        let analyzer = AudioAnalyzer::new();

        let energy_levels = analyzer.calculate_energy_levels(
            &audio_data.mono_samples(),
            audio_data.sample_rate
        ).unwrap();

        assert!(!energy_levels.is_empty());
        assert!(energy_levels.iter().all(|e| e.time >= 0.0));
        assert!(energy_levels.iter().all(|e| e.rms >= 0.0));
    }

    #[test]
    fn test_config_validation() {
        let mut config = AnalysisConfig::default();
        assert!(AudioAnalyzer::with_config(config.clone()).config.validate().is_ok());

        config.window_size = 1000; // Not a power of two
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("power of two"));
    }
}