//! Video Processing Module

pub mod types;
pub mod processor;
pub mod loader_optimized;
pub mod compositor_pure_rust;


pub use types::{Frame, VideoClip, VideoParams, VideoSequence};
pub use processor::{VideoProcessor, ProcessedSegment};
pub use loader_optimized::{VideoLoader, VideoMetadata};
pub use compositor_pure_rust::{VideoCompositor, EncodedVideo};
