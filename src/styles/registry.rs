use std::collections::HashMap;

use crate::styles::{Style, VhsStyle, FilmStyle, VintageStyle, BoardsStyle};

/// Registry for managing available retro styles
///
/// The registry provides a central place to discover and instantiate styles.
/// Styles are registered by name and can be retrieved for use in composition.
pub struct StyleRegistry {
    styles: HashMap<String, Box<dyn Fn() -> Box<dyn Style>>>,
}

impl StyleRegistry {
    /// Create a new style registry with all built-in styles
    pub fn new() -> Self {
        let mut registry = Self {
            styles: HashMap::new(),
        };

        // Register all built-in styles
        registry.register_builtin_styles();
        registry
    }

    /// Register all built-in styles
    fn register_builtin_styles(&mut self) {
        // VHS style
        self.styles.insert(
            "vhs".to_string(),
            Box::new(|| Box::new(VhsStyle::new())),
        );

        // Film style
        self.styles.insert(
            "film".to_string(),
            Box::new(|| Box::new(FilmStyle::new())),
        );

        // Vintage style
        self.styles.insert(
            "vintage".to_string(),
            Box::new(|| Box::new(VintageStyle::new())),
        );

        // Boards style
        self.styles.insert(
            "boards".to_string(),
            Box::new(|| Box::new(BoardsStyle::new())),
        );
    }

    /// Register a custom style
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the style
    /// * `factory` - Function that creates new instances of the style
    pub fn register<F>(&mut self, name: String, factory: F)
    where
        F: Fn() -> Box<dyn Style> + 'static,
    {
        self.styles.insert(name, Box::new(factory));
    }

    /// Get a style by name
    ///
    /// Returns a new instance of the requested style, or None if the style
    /// is not registered.
    pub fn get_style(&self, name: &str) -> Option<Box<dyn Style>> {
        self.styles.get(name).map(|factory| factory())
    }

    /// Get all available style names
    pub fn available_styles(&self) -> Vec<String> {
        self.styles.keys().cloned().collect()
    }

    /// Check if a style is available
    pub fn has_style(&self, name: &str) -> bool {
        self.styles.contains_key(name)
    }

    /// Get the number of registered styles
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

impl Default for StyleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_styles_available() {
        let registry = StyleRegistry::new();

        assert!(registry.has_style("vhs"));
        assert!(registry.has_style("film"));
        assert!(registry.has_style("vintage"));
        assert!(registry.has_style("boards"));

        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn test_get_style() {
        let registry = StyleRegistry::new();

        let vhs_style = registry.get_style("vhs");
        assert!(vhs_style.is_some());
        assert_eq!(vhs_style.unwrap().name(), "vhs");

        let unknown_style = registry.get_style("unknown");
        assert!(unknown_style.is_none());
    }

    #[test]
    fn test_available_styles() {
        let registry = StyleRegistry::new();
        let styles = registry.available_styles();

        assert!(styles.contains(&"vhs".to_string()));
        assert!(styles.contains(&"film".to_string()));
        assert!(styles.contains(&"vintage".to_string()));
        assert!(styles.contains(&"boards".to_string()));
    }

    #[test]
    fn test_custom_style_registration() {
        let mut registry = StyleRegistry::new();

        // Register a custom style
        registry.register("custom".to_string(), || {
            Box::new(VhsStyle::new()) // Using VHS as a placeholder
        });

        assert!(registry.has_style("custom"));
        assert_eq!(registry.len(), 5); // 4 built-in + 1 custom
    }
}