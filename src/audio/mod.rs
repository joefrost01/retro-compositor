//! # Audio Analysis Module
//!
//! Provides comprehensive audio analysis capabilities including beat detection,
//! tempo analysis, and energy level calculation for synchronizing video cuts
//! to musical elements.
//!
//! ## Core Features
//!
//! - **Beat Detection**: FFT-based onset detection with configurable sensitivity
//! - **Tempo Analysis**: BPM calculation using autocorrelation and peak detection
//! - **Energy Analysis**: RMS energy calculation for dynamic cut timing
//! - **Musical Structure**: Phrase and section boundary detection
//!
//! ## Usage
//!
//! ```rust,no_run
//! use retro_compositor::audio::{AudioAnalyzer, AudioLoader};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Load audio file
//! let audio_data = AudioLoader::load("song.wav").await?;
//!
//! // Analyze for beats and tempo
//! let analyzer = AudioAnalyzer::new();
//! let analysis = analyzer.analyze(&audio_data).await?;
//!
//! println!("Detected BPM: {}", analysis.bpm);
//! println!("Found {} beats", analysis.beats.len());
//! # Ok(())
//! # }
//! ```

pub mod analyzer;
pub mod loader;
pub mod types;
pub use analyzer::AudioAnalyzer;
pub use loader::AudioLoader;
pub use types::{
    AudioData, AudioAnalysis, Beat, EnergyLevel,
    TempoMap, AudioFormat, AnalysisConfig
};