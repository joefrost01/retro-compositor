use thiserror::Error;

/// Main error type for the Retro-Compositor library
#[derive(Error, Debug)]
pub enum CompositorError {
    #[error("Audio processing error: {0}")]
    Audio(#[from] AudioError),

    #[error("Video processing error: {0}")]
    Video(#[from] VideoError),

    #[error("Composition error: {0}")]
    Composition(#[from] CompositionError),

    #[error("Style processing error: {0}")]
    Style(#[from] StyleError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Generic error: {0}")]
    Generic(String),
}

/// Audio-specific errors
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Failed to load audio file: {path}")]
    LoadFailed { path: String },

    #[error("Unsupported audio format: {format}")]
    UnsupportedFormat { format: String },

    #[error("Beat detection failed: {reason}")]
    BeatDetectionFailed { reason: String },

    #[error("Audio analysis failed: {reason}")]
    AnalysisFailed { reason: String },

    #[error("Invalid audio parameters: {details}")]
    InvalidParameters { details: String },
}

/// Video-specific errors
#[derive(Error, Debug)]
pub enum VideoError {
    #[error("Failed to load video file: {path}")]
    LoadFailed { path: String },

    #[error("Unsupported video format: {format}")]
    UnsupportedFormat { format: String },

    #[error("Video encoding failed: {reason}")]
    EncodingFailed { reason: String },

    #[error("Video decoding failed: {reason}")]
    DecodingFailed { reason: String },

    #[error("Frame processing failed: {reason}")]
    FrameProcessingFailed { reason: String },

    #[error("Invalid video parameters: {details}")]
    InvalidParameters { details: String },
}

/// Composition-specific errors
#[derive(Error, Debug)]
pub enum CompositionError {
    #[error("Timeline synchronization failed: {reason}")]
    SyncFailed { reason: String },

    #[error("No video clips found in directory: {path}")]
    NoClipsFound { path: String },

    #[error("Clip sequencing failed: {reason}")]
    SequencingFailed { reason: String },

    #[error("Output generation failed: {reason}")]
    OutputFailed { reason: String },

    #[error("Invalid composition parameters: {details}")]
    InvalidParameters { details: String },
}

/// Style-specific errors
#[derive(Error, Debug)]
pub enum StyleError {
    #[error("Style not found: {name}")]
    NotFound { name: String },

    #[error("Effect application failed: {effect} - {reason}")]
    EffectFailed { effect: String, reason: String },

    #[error("Style configuration invalid: {details}")]
    InvalidConfig { details: String },

    #[error("Style loading failed: {name} - {reason}")]
    LoadFailed { name: String, reason: String },
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to parse configuration file: {path}")]
    ParseFailed { path: String },

    #[error("Invalid configuration value: {key} = {value}")]
    InvalidValue { key: String, value: String },

    #[error("Missing required configuration: {key}")]
    MissingKey { key: String },

    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
}

/// Convenience type alias for Results using CompositorError
pub type Result<T> = std::result::Result<T, CompositorError>;

impl CompositorError {
    /// Create a generic error with a custom message
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic(message.into())
    }

    /// Check if this error is recoverable (can be retried)
    pub fn is_recoverable(&self) -> bool {
        match self {
            // IO errors might be temporary
            Self::Io(_) => true,
            // Audio/video loading might work on retry
            Self::Audio(AudioError::LoadFailed { .. }) => true,
            Self::Video(VideoError::LoadFailed { .. }) => true,
            // Most other errors are permanent
            _ => false,
        }
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Audio(AudioError::LoadFailed { path }) => {
                format!("Could not load audio file '{}'. Please check the file exists and is a supported format.", path)
            }
            Self::Video(VideoError::LoadFailed { path }) => {
                format!("Could not load video file '{}'. Please check the file exists and is a supported format.", path)
            }
            Self::Style(StyleError::NotFound { name }) => {
                format!("Style '{}' not found. Available styles: vhs, film, vintage, boards", name)
            }
            Self::Config(ConfigError::FileNotFound { path }) => {
                format!("Configuration file '{}' not found.", path)
            }
            _ => self.to_string(),
        }
    }
}