//! # Retro-Compositor
//!
//! Transform your music into retro-styled video compositions with AI-driven beat synchronization.
//!
//! This library provides a complete toolkit for analyzing audio tracks and creating synchronized
//! video compositions with authentic retro visual effects.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use retro_compositor::{
//!     composition::CompositionEngine,
//!     config::Config,
//!     styles::StyleRegistry,
//! };
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let config = Config::default();
//! let style_registry = StyleRegistry::new();
//! let vhs_style = style_registry.get_style("vhs").unwrap();
//!
//! let engine = CompositionEngine::new(config, vhs_style);
//! engine.compose(
//!     "song.wav",
//!     "video_clips/",
//!     "output.mp4"
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several key modules:
//!
//! - [`video`] - Video processing and composition
//! - [`composition`] - Main composition engine
//! - [`styles`] - Retro effect styles and processing
//! - [`config`] - Configuration management
//!
//! ## Creating Custom Styles
//!
//! You can create custom retro styles by implementing the [`Style`](styles::Style) trait:
//!
//! ```rust,no_run
//! use retro_compositor::styles::{Style, StyleConfig};
//! use retro_compositor::video::types::Frame;
//! use anyhow::Result;
//!
//! struct MyCustomStyle;
//!
//! impl Style for MyCustomStyle {
//!     fn name(&self) -> &str {
//!         "my_custom"
//!     }
//!
//!     fn apply_effect(&self, frame: &mut Frame, config: &StyleConfig) -> Result<()> {
//!         // Your custom effect implementation
//!         Ok(())
//!     }
//! }
//! ```

pub mod audio;
pub mod composition;
pub mod config;
pub mod error;
pub mod styles;
pub mod video;

// Re-export commonly used types for convenience
pub use crate::{
    composition::CompositionEngine,
    config::Config,
    error::{CompositorError, Result},
    styles::{Style, StyleRegistry}, // Export Style trait
};