//! # Video Processing Module
//!
//! Handles video file loading, frame processing, and output generation.
//! This module provides a simple interface for working with video data
//! while abstracting away the complexity of different video formats.

pub mod types;
// TODO: Implement these modules
// pub mod loader;
// pub mod processor;
// pub mod compositor;

// Re-exports for convenience
pub use types::{Frame, VideoClip, VideoParams, VideoSequence};
// pub use loader::VideoLoader;
// pub use processor::VideoProcessor;
// pub use compositor::VideoCompositor;