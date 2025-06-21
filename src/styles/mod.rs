//! # Retro Style System
//!
//! This module provides the extensible style system for applying retro effects to video frames.
//! Each style is self-contained and can be applied independently or in combination.
//!
//! ## Built-in Styles
//!
//! - **VHS**: Scan lines, color bleeding, tracking errors, noise
//! - **Film**: Grain, scratches, color fading, light leaks
//! - **Vintage**: Sepia tones, vignetting, soft focus
//! - **Boards**: High contrast, bold colors, geometric overlays
//!
//! ## Usage
//!
//! ```rust,no_run
//! use retro_compositor::styles::{StyleRegistry, StyleConfig};
//!
//! let registry = StyleRegistry::new();
//! let vhs_style = registry.get_style("vhs").unwrap();
//!
//! let config = StyleConfig::default();
//! // Apply style to frames during video processing
//! ```

pub mod registry;
pub mod traits;

// Style implementations
pub mod vhs;
pub mod film;
pub mod vintage;
pub mod boards;

// Re-exports for convenience
pub use registry::StyleRegistry;
pub use traits::{Style, StyleConfig, StyleMetadata};

// Re-export all built-in styles
pub use vhs::VhsStyle;
pub use film::FilmStyle;
pub use vintage::VintageStyle;
pub use boards::BoardsStyle;