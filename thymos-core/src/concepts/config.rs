use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a specific concept type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptTypeConfig {
    /// Human-readable name for the concept type
    pub name: String,

    /// Regex patterns for this concept type
    pub patterns: Vec<String>,

    /// Base significance score for matches
    pub base_significance: f64,

    /// Whether this type is enabled
    pub enabled: bool,
}

impl ConceptTypeConfig {
    /// Create a new concept type configuration.
    pub fn new(name: impl Into<String>, patterns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            patterns,
            base_significance: 0.7,
            enabled: true,
        }
    }

    /// Set the base significance for this concept type.
    pub fn with_base_significance(mut self, significance: f64) -> Self {
        self.base_significance = significance.clamp(0.0, 1.0);
        self
    }

    /// Disable this concept type.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Configuration for concept extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptExtractionConfig {
    /// Concept types and their extraction patterns
    pub concept_types: HashMap<String, ConceptTypeConfig>,

    /// Significance threshold (0.0-1.0) for concepts to be considered "significant"
    pub significance_threshold: f64,
}

impl ConceptExtractionConfig {
    /// Create a new concept extraction configuration.
    pub fn new() -> Self {
        Self {
            concept_types: HashMap::new(),
            significance_threshold: 0.5,
        }
    }

    /// Add a concept type to the configuration.
    pub fn with_concept_type(
        mut self,
        type_id: impl Into<String>,
        config: ConceptTypeConfig,
    ) -> Self {
        self.concept_types.insert(type_id.into(), config);
        self
    }

    /// Set the significance threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.significance_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Create default configuration with common concept types.
    pub fn default_with_common_types() -> Self {
        let mut config = Self::new();

        // Character type: Names with capitals or titles
        config = config.with_concept_type(
            "character",
            ConceptTypeConfig::new(
                "Character",
                vec![
                    r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)\b".to_string(), // Capitalized names
                ],
            )
            .with_base_significance(0.8),
        );

        // Location type: Common location descriptors
        config = config.with_concept_type(
            "location",
            ConceptTypeConfig::new(
                "Location",
                vec![r"\b(?:in|at|from)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)\b".to_string()],
            )
            .with_base_significance(0.6),
        );

        config.with_threshold(0.5)
    }
}

impl Default for ConceptExtractionConfig {
    fn default() -> Self {
        Self::default_with_common_types()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concept_type_config() {
        let config = ConceptTypeConfig::new("character", vec![r"\b[A-Z][a-z]+\b".to_string()]);

        assert_eq!(config.name, "character");
        assert!(!config.patterns.is_empty());
        assert!(config.enabled);
    }

    #[test]
    fn test_extraction_config() {
        let config = ConceptExtractionConfig::new()
            .with_concept_type("character", ConceptTypeConfig::new("Character", vec![]))
            .with_threshold(0.6);

        assert_eq!(config.significance_threshold, 0.6);
        assert!(config.concept_types.contains_key("character"));
    }

    #[test]
    fn test_default_config() {
        let config = ConceptExtractionConfig::default();
        assert!(config.concept_types.contains_key("character"));
        assert!(config.concept_types.contains_key("location"));
    }
}
