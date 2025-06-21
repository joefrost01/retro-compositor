use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{error::Result, video::types::Frame};

/// Core trait that all retro styles must implement
pub trait Style: Send + Sync {
    /// Returns the unique name of this style
    fn name(&self) -> &str;

    /// Returns a human-readable description of this style
    fn description(&self) -> &str;

    /// Apply the retro effect to a video frame
    ///
    /// # Arguments
    ///
    /// * `frame` - The video frame to modify in-place
    /// * `config` - Style-specific configuration parameters
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the effect was applied successfully, or an error if processing failed.
    fn apply_effect(&self, frame: &mut Frame, config: &StyleConfig) -> Result<()>;

    /// Get the default configuration for this style
    fn default_config(&self) -> StyleConfig {
        StyleConfig::default()
    }

    /// Validate that the given configuration is valid for this style
    ///
    /// This allows styles to check that required parameters are present and valid
    /// before processing begins.
    fn validate_config(&self, config: &StyleConfig) -> Result<()> {
        // Default implementation accepts any config
        let _ = config;
        Ok(())
    }

    /// Get style-specific metadata or capabilities
    ///
    /// This can be used to expose information about what the style supports,
    /// performance characteristics, or other metadata.
    fn metadata(&self) -> StyleMetadata {
        StyleMetadata::default()
    }

    /// Initialize any resources needed by this style
    ///
    /// Called once before processing begins. Useful for loading shaders,
    /// initializing GPU resources, or pre-computing expensive data.
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Clean up resources used by this style
    ///
    /// Called once after processing is complete.
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Configuration for style effects
///
/// This is a flexible configuration system that allows each style to define
/// its own parameters while providing a common interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    /// Intensity of the effect (0.0 = none, 1.0 = full intensity)
    pub intensity: f32,

    /// Style-specific parameters
    pub parameters: HashMap<String, ConfigValue>,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            intensity: 0.8, // Default to fairly strong effect
            parameters: HashMap::new(),
        }
    }
}

impl StyleConfig {
    /// Create a new config with the given intensity
    pub fn with_intensity(intensity: f32) -> Self {
        Self {
            intensity: intensity.clamp(0.0, 1.0),
            parameters: HashMap::new(),
        }
    }

    /// Set a parameter value
    pub fn set<K: Into<String>, V: Into<ConfigValue>>(mut self, key: K, value: V) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }

    /// Get a parameter value as a specific type
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.parameters.get(key).and_then(|v| v.as_f32())
    }

    /// Get a parameter value as a boolean
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.parameters.get(key).and_then(|v| v.as_bool())
    }

    /// Get a parameter value as a string
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).and_then(|v| v.as_string())
    }

    /// Get a parameter value with a default
    pub fn get_f32_or(&self, key: &str, default: f32) -> f32 {
        self.get_f32(key).unwrap_or(default)
    }

    /// Get a parameter value with a default
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }
}

/// Flexible configuration value that can hold different types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Float(f32),
    Bool(bool),
    String(String),
    Integer(i32),
}

impl ConfigValue {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            ConfigValue::Integer(i) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            ConfigValue::Float(f) => Some(*f as i32),
            _ => None,
        }
    }
}

impl From<f32> for ConfigValue {
    fn from(value: f32) -> Self {
        ConfigValue::Float(value)
    }
}

impl From<bool> for ConfigValue {
    fn from(value: bool) -> Self {
        ConfigValue::Bool(value)
    }
}

impl From<String> for ConfigValue {
    fn from(value: String) -> Self {
        ConfigValue::String(value)
    }
}

impl From<&str> for ConfigValue {
    fn from(value: &str) -> Self {
        ConfigValue::String(value.to_string())
    }
}

impl From<i32> for ConfigValue {
    fn from(value: i32) -> Self {
        ConfigValue::Integer(value)
    }
}

/// Metadata about a style's capabilities and characteristics
#[derive(Debug, Clone, Default)]
pub struct StyleMetadata {
    /// Whether this style can utilize GPU acceleration
    pub gpu_accelerated: bool,

    /// Estimated performance impact (0.0 = minimal, 1.0 = heavy)
    pub performance_impact: f32,

    /// Whether this style works well with other styles
    pub composable: bool,

    /// List of required parameters
    pub required_parameters: Vec<String>,

    /// List of optional parameters with descriptions
    pub optional_parameters: Vec<(String, String)>,
}