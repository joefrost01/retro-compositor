//! # Video Processing Module
//!
//! Handles video file loading, frame processing, and output generation.

pub mod types;
pub mod processor;

// Use pure Rust implementation (no FFmpeg linking issues)
mod loader_pure_rust;
mod compositor_pure_rust;

pub use types::{Frame, VideoClip, VideoParams, VideoSequence};
pub use processor::{VideoProcessor, ProcessedSegment};
pub use loader_pure_rust::{VideoLoader, VideoMetadata};
pub use compositor_pure_rust::{VideoCompositor, EncodedVideo};