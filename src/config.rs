use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::{
    error::{ConfigError, Result},
    styles::StyleConfig,
    video::VideoParams,
};

/// Main configuration for the Retro-Compositor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Audio analysis settings
    pub audio: AudioConfig,

    /// Video processing settings
    pub video: VideoConfig,

    /// Composition settings
    pub composition: CompositionConfig,

    /// Default style configuration
    pub style: StyleConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            video: VideoConfig::default(),
            composition: CompositionConfig::default(),
            style: StyleConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|_| ConfigError::FileNotFound { path: path.display().to_string() })?;

        let config: Config = toml::from_str(&content)
            .map_err(|_| ConfigError::ParseFailed { path: path.display().to_string() })?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::InvalidValue {
                key: "config".to_string(),
                value: e.to_string()
            })?;

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.audio.validate()?;
        self.video.validate()?;
        self.composition.validate()?;
        Ok(())
    }
}

/// Audio analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate for analysis (Hz)
    pub sample_rate: u32,

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

    /// Energy threshold for beat detection
    pub energy_threshold: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            window_size: 1024,
            hop_size: 512,
            min_bpm: 60.0,
            max_bpm: 200.0,
            beat_sensitivity: 0.7,
            energy_threshold: 0.1,
        }
    }
}

impl AudioConfig {
    fn validate(&self) -> Result<()> {
        if self.sample_rate == 0 {
            return Err(ConfigError::InvalidValue {
                key: "audio.sample_rate".to_string(),
                value: self.sample_rate.to_string()
            }.into());
        }

        if self.window_size == 0 || !self.window_size.is_power_of_two() {
            return Err(ConfigError::InvalidValue {
                key: "audio.window_size".to_string(),
                value: self.window_size.to_string()
            }.into());
        }

        if self.min_bpm >= self.max_bpm {
            return Err(ConfigError::InvalidValue {
                key: "audio.bpm_range".to_string(),
                value: format!("{}-{}", self.min_bpm, self.max_bpm)
            }.into());
        }

        Ok(())
    }
}

/// Video processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Video processing parameters
    pub params: VideoParams,

    /// Maximum clip duration in seconds
    pub max_clip_duration: f64,

    /// Minimum clip duration in seconds
    pub min_clip_duration: f64,

    /// Number of parallel processing threads
    pub processing_threads: usize,

    /// Enable GPU acceleration if available
    pub gpu_acceleration: bool,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            params: VideoParams::default(),
            max_clip_duration: 30.0,
            min_clip_duration: 0.5,
            processing_threads: num_cpus::get(),
            gpu_acceleration: false, // Conservative default
        }
    }
}

impl VideoConfig {
    fn validate(&self) -> Result<()> {
        if self.max_clip_duration <= self.min_clip_duration {
            return Err(ConfigError::InvalidValue {
                key: "video.clip_duration_range".to_string(),
                value: format!("{}-{}", self.min_clip_duration, self.max_clip_duration)
            }.into());
        }

        if self.processing_threads == 0 {
            return Err(ConfigError::InvalidValue {
                key: "video.processing_threads".to_string(),
                value: self.processing_threads.to_string()
            }.into());
        }

        Ok(())
    }
}

/// Composition engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionConfig {
    /// How closely to sync cuts to beats (0.0-1.0)
    pub beat_sync_strength: f32,

    /// Allow cuts on musical phrases (not just beats)
    pub phrase_aware_cuts: bool,

    /// Minimum time between cuts (seconds)
    pub min_cut_interval: f64,

    /// Maximum time between cuts (seconds)
    pub max_cut_interval: f64,

    /// Use energy levels to influence cut timing
    pub energy_based_cuts: bool,

    /// Crossfade duration between clips (seconds)
    pub crossfade_duration: f64,
}

impl Default for CompositionConfig {
    fn default() -> Self {
        Self {
            beat_sync_strength: 0.8,
            phrase_aware_cuts: true,
            min_cut_interval: 1.0,
            max_cut_interval: 8.0,
            energy_based_cuts: true,
            crossfade_duration: 0.1,
        }
    }
}

impl CompositionConfig {
    fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.beat_sync_strength) {
            return Err(ConfigError::InvalidValue {
                key: "composition.beat_sync_strength".to_string(),
                value: self.beat_sync_strength.to_string()
            }.into());
        }

        if self.max_cut_interval <= self.min_cut_interval {
            return Err(ConfigError::InvalidValue {
                key: "composition.cut_interval_range".to_string(),
                value: format!("{}-{}", self.min_cut_interval, self.max_cut_interval)
            }.into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config_is_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_config.toml");

        let original_config = Config::default();

        // Save and load
        original_config.save_to_file(&file_path).unwrap();
        let loaded_config = Config::from_file(&file_path).unwrap();

        // Compare (this is simplified - in practice you'd want proper PartialEq)
        assert_eq!(original_config.audio.sample_rate, loaded_config.audio.sample_rate);
        assert_eq!(original_config.video.params.fps, loaded_config.video.params.fps);
    }

    #[test]
    fn test_invalid_audio_config() {
        let mut config = Config::default();
        config.audio.sample_rate = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_bpm_range() {
        let mut config = Config::default();
        config.audio.min_bpm = 150.0;
        config.audio.max_bpm = 100.0;
        assert!(config.validate().is_err());
    }
}