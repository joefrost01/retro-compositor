use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Raw audio data with metadata
#[derive(Debug, Clone)]
pub struct AudioData {
    /// Audio samples (interleaved for stereo, mono for single channel)
    pub samples: Vec<f32>,

    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u16,

    /// Duration in seconds
    pub duration: f64,

    /// Original file path
    pub file_path: PathBuf,

    /// Audio format information
    pub format: AudioFormat,
}

impl AudioData {
    /// Get samples for a specific channel (0-based)
    pub fn channel_samples(&self, channel: usize) -> Vec<f32> {
        if self.channels == 1 || channel >= self.channels as usize {
            return self.samples.clone();
        }

        self.samples
            .iter()
            .skip(channel)
            .step_by(self.channels as usize)
            .copied()
            .collect()
    }

    /// Get mono mix of all channels
    pub fn mono_samples(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }

        let mut mono = Vec::with_capacity(self.samples.len() / self.channels as usize);

        for chunk in self.samples.chunks(self.channels as usize) {
            let sum: f32 = chunk.iter().sum();
            mono.push(sum / self.channels as f32);
        }

        mono
    }

    /// Get sample at specific time (in seconds)
    pub fn sample_at_time(&self, time: f64, channel: usize) -> f32 {
        let sample_index = (time * self.sample_rate as f64) as usize;
        let actual_index = sample_index * self.channels as usize + channel;

        self.samples.get(actual_index).copied().unwrap_or(0.0)
    }

    /// Get time in seconds for a sample index
    pub fn time_for_sample(&self, sample_index: usize) -> f64 {
        sample_index as f64 / self.sample_rate as f64
    }
}

/// Audio file format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormat {
    /// File extension (wav, mp3, flac, etc.)
    pub extension: String,

    /// Bit depth (16, 24, 32, etc.)
    pub bit_depth: Option<u16>,

    /// Compression type (if any)
    pub compression: Option<String>,

    /// Bitrate for compressed formats
    pub bitrate: Option<u32>,
}

/// Complete analysis results for an audio file
#[derive(Debug, Clone)]
pub struct AudioAnalysis {
    /// Detected beats with timestamps
    pub beats: Vec<Beat>,

    /// Calculated tempo information
    pub tempo: TempoMap,

    /// Energy levels over time
    pub energy_levels: Vec<EnergyLevel>,

    /// Overall BPM (beats per minute)
    pub bpm: f32,

    /// Confidence in BPM detection (0.0-1.0)
    pub bpm_confidence: f32,

    /// Total duration in seconds
    pub duration: f64,

    /// Analysis configuration used
    pub config: AnalysisConfig,

    /// Musical phrases and sections
    pub phrases: Vec<Phrase>,

    /// Spectral features for advanced analysis
    pub spectral_features: SpectralFeatures,
}

impl AudioAnalysis {
    /// Get beats within a time range
    pub fn beats_in_range(&self, start: f64, end: f64) -> Vec<&Beat> {
        self.beats
            .iter()
            .filter(|beat| beat.time >= start && beat.time <= end)
            .collect()
    }

    /// Get average energy in a time range
    pub fn average_energy_in_range(&self, start: f64, end: f64) -> f32 {
        let relevant_energies: Vec<f32> = self.energy_levels
            .iter()
            .filter(|energy| energy.time >= start && energy.time <= end)
            .map(|energy| energy.rms)
            .collect();

        if relevant_energies.is_empty() {
            0.0
        } else {
            relevant_energies.iter().sum::<f32>() / relevant_energies.len() as f32
        }
    }

    /// Find the next beat after a given time
    pub fn next_beat_after(&self, time: f64) -> Option<&Beat> {
        self.beats.iter().find(|beat| beat.time > time)
    }

    /// Get tempo at a specific time
    pub fn tempo_at_time(&self, time: f64) -> f32 {
        // For now, return the global BPM
        // In future versions, this could support tempo changes
        self.bpm
    }
}

/// Individual beat detection with metadata
#[derive(Debug, Clone)]
pub struct Beat {
    /// Time of the beat in seconds
    pub time: f64,

    /// Strength/confidence of the beat (0.0-1.0)
    pub strength: f32,

    /// Type of beat (downbeat, upbeat, etc.)
    pub beat_type: BeatType,

    /// Onset detection function value
    pub onset_value: f32,

    /// Local energy around this beat
    pub local_energy: f32,
}

/// Classification of beat types
#[derive(Debug, Clone, PartialEq)]
pub enum BeatType {
    /// Strong downbeat (typically beat 1 of a measure)
    Downbeat,

    /// Regular beat
    Beat,

    /// Weak beat or off-beat
    Offbeat,

    /// Detected onset that might not be a musical beat
    Onset,
}

/// Tempo mapping information
#[derive(Debug, Clone)]
pub struct TempoMap {
    /// Global BPM
    pub global_bpm: f32,

    /// Confidence in global BPM (0.0-1.0)
    pub confidence: f32,

    /// Tempo changes over time (future feature)
    pub tempo_changes: Vec<TempoChange>,

    /// Time signature (4/4, 3/4, etc.)
    pub time_signature: TimeSignature,
}

/// Tempo change point (for songs with varying tempo)
#[derive(Debug, Clone)]
pub struct TempoChange {
    /// Time when tempo changes
    pub time: f64,

    /// New BPM value
    pub bpm: f32,

    /// Confidence in this tempo change
    pub confidence: f32,
}

/// Time signature information
#[derive(Debug, Clone)]
pub struct TimeSignature {
    /// Beats per measure (numerator)
    pub beats_per_measure: u8,

    /// Note value for beat (denominator, e.g., 4 for quarter note)
    pub beat_note_value: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            beats_per_measure: 4,
            beat_note_value: 4,
        }
    }
}

/// Energy level measurement at a point in time
#[derive(Debug, Clone)]
pub struct EnergyLevel {
    /// Time in seconds
    pub time: f64,

    /// RMS (Root Mean Square) energy
    pub rms: f32,

    /// Peak energy
    pub peak: f32,

    /// Spectral centroid (brightness)
    pub spectral_centroid: f32,

    /// Zero crossing rate (roughness indicator)
    pub zero_crossing_rate: f32,
}

/// Musical phrase or section
#[derive(Debug, Clone)]
pub struct Phrase {
    /// Start time in seconds
    pub start: f64,

    /// End time in seconds
    pub end: f64,

    /// Type of phrase/section
    pub phrase_type: PhraseType,

    /// Confidence in this segmentation
    pub confidence: f32,
}

/// Types of musical phrases/sections
#[derive(Debug, Clone, PartialEq)]
pub enum PhraseType {
    /// Introduction
    Intro,

    /// Verse section
    Verse,

    /// Chorus section
    Chorus,

    /// Bridge section
    Bridge,

    /// Outro/ending
    Outro,

    /// Unknown/unclassified section
    Unknown,
}

/// Spectral analysis features
#[derive(Debug, Clone)]
pub struct SpectralFeatures {
    /// Mel-frequency cepstral coefficients
    pub mfcc: Vec<Vec<f32>>,

    /// Spectral centroid over time
    pub spectral_centroid: Vec<f32>,

    /// Spectral rolloff over time
    pub spectral_rolloff: Vec<f32>,

    /// Chroma features for harmonic analysis
    pub chroma: Vec<Vec<f32>>,

    /// Onset detection function values
    pub onset_detection_function: Vec<f32>,
}

/// Configuration for audio analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Window size for FFT analysis
    pub window_size: usize,

    /// Hop size for analysis windows
    pub hop_size: usize,

    /// Minimum BPM to detect
    pub min_bpm: f32,

    /// Maximum BPM to detect
    pub max_bpm: f32,

    /// Beat detection sensitivity (0.0-1.0)
    pub beat_sensitivity: f32,

    /// Energy analysis window size in seconds
    pub energy_window_size: f64,

    /// Whether to perform phrase detection
    pub detect_phrases: bool,

    /// Whether to calculate spectral features
    pub calculate_spectral_features: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            window_size: 1024,
            hop_size: 512,
            min_bpm: 60.0,
            max_bpm: 200.0,
            beat_sensitivity: 0.7,
            energy_window_size: 0.1, // 100ms windows
            detect_phrases: true,
            calculate_spectral_features: true,
        }
    }
}

impl AnalysisConfig {
    /// Create a fast analysis config (lower quality, faster processing)
    pub fn fast() -> Self {
        Self {
            window_size: 512,
            hop_size: 256,
            detect_phrases: false,
            calculate_spectral_features: false,
            ..Default::default()
        }
    }

    /// Create a high-quality analysis config (slower but more accurate)
    pub fn high_quality() -> Self {
        Self {
            window_size: 2048,
            hop_size: 512,
            beat_sensitivity: 0.8,
            energy_window_size: 0.05, // 50ms windows
            ..Default::default()
        }
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.window_size == 0 || !self.window_size.is_power_of_two() {
            return Err("Window size must be a power of two".to_string());
        }

        if self.hop_size > self.window_size {
            return Err("Hop size cannot be larger than window size".to_string());
        }

        if self.min_bpm >= self.max_bpm {
            return Err("Minimum BPM must be less than maximum BPM".to_string());
        }

        if !(0.0..=1.0).contains(&self.beat_sensitivity) {
            return Err("Beat sensitivity must be between 0.0 and 1.0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_data_mono_conversion() {
        let stereo_samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // L, R, L, R, L, R
        let audio_data = AudioData {
            samples: stereo_samples,
            sample_rate: 44100,
            channels: 2,
            duration: 1.0,
            file_path: PathBuf::from("test.wav"),
            format: AudioFormat {
                extension: "wav".to_string(),
                bit_depth: Some(16),
                compression: None,
                bitrate: None,
            },
        };

        let mono = audio_data.mono_samples();
        assert_eq!(mono, vec![1.5, 3.5, 5.5]); // Average of L and R channels
    }

    #[test]
    fn test_analysis_config_validation() {
        let config = AnalysisConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = AnalysisConfig {
            window_size: 1000, // Not a power of two
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_beat_range_filtering() {
        let beats = vec![
            Beat {
                time: 1.0,
                strength: 0.8,
                beat_type: BeatType::Beat,
                onset_value: 0.5,
                local_energy: 0.3,
            },
            Beat {
                time: 2.5,
                strength: 0.9,
                beat_type: BeatType::Downbeat,
                onset_value: 0.7,
                local_energy: 0.4,
            },
            Beat {
                time: 4.0,
                strength: 0.7,
                beat_type: BeatType::Beat,
                onset_value: 0.4,
                local_energy: 0.2,
            },
        ];

        let analysis = AudioAnalysis {
            beats,
            tempo: TempoMap {
                global_bpm: 120.0,
                confidence: 0.9,
                tempo_changes: vec![],
                time_signature: TimeSignature::default(),
            },
            energy_levels: vec![],
            bpm: 120.0,
            bpm_confidence: 0.9,
            duration: 5.0,
            config: AnalysisConfig::default(),
            phrases: vec![],
            spectral_features: SpectralFeatures {
                mfcc: vec![],
                spectral_centroid: vec![],
                spectral_rolloff: vec![],
                chroma: vec![],
                onset_detection_function: vec![],
            },
        };

        let beats_in_range = analysis.beats_in_range(1.5, 3.0);
        assert_eq!(beats_in_range.len(), 1);
        assert_eq!(beats_in_range[0].time, 2.5);
    }
}