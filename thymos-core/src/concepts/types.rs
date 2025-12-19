use serde::{Deserialize, Serialize};

use super::alias::Alias;

/// Context information for concept extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Session or domain context identifier
    pub domain: String,
    /// Optional user or agent context
    pub source: Option<String>,
    /// Optional temporal context
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl Context {
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            source: None,
            timestamp: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Represents an extracted concept/entity from text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Concept {
    /// The exact text of the concept as extracted
    pub text: String,

    /// Concept type (domain-specific: "character", "location", "item", etc.)
    pub concept_type: String,

    /// Contextual snippet where this concept appeared
    pub context: String,

    /// Significance score (0.0-1.0)
    pub significance: f64,

    /// Whether this concept meets the extraction threshold
    pub is_significant: bool,

    /// Alternative names and references for this concept
    #[serde(default)]
    pub aliases: Vec<Alias>,
}

impl Concept {
    /// Create a new concept with the given parameters.
    pub fn new(
        text: impl Into<String>,
        concept_type: impl Into<String>,
        context: impl Into<String>,
        significance: f64,
    ) -> Self {
        let is_significant = significance >= 0.5; // Default threshold
        Self {
            text: text.into(),
            concept_type: concept_type.into(),
            context: context.into(),
            significance,
            is_significant,
            aliases: Vec::new(),
        }
    }

    /// Create a concept with explicit significance threshold.
    pub fn with_threshold(
        text: impl Into<String>,
        concept_type: impl Into<String>,
        context: impl Into<String>,
        significance: f64,
        threshold: f64,
    ) -> Self {
        let is_significant = significance >= threshold;
        Self {
            text: text.into(),
            concept_type: concept_type.into(),
            context: context.into(),
            significance,
            is_significant,
            aliases: Vec::new(),
        }
    }

    /// Add an alias to this concept.
    pub fn with_alias(mut self, alias: Alias) -> Self {
        self.aliases.push(alias);
        self
    }

    /// Add multiple aliases to this concept.
    pub fn with_aliases(mut self, aliases: Vec<Alias>) -> Self {
        self.aliases.extend(aliases);
        self
    }

    /// Get all aliases sorted by confidence (highest first).
    pub fn aliases_by_confidence(&self) -> Vec<&Alias> {
        let mut aliases = self.aliases.iter().collect::<Vec<_>>();
        aliases.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        aliases
    }

    /// Find an alias by exact text match.
    pub fn find_alias(&self, text: &str) -> Option<&Alias> {
        self.aliases.iter().find(|a| a.text == text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let context = Context::new("game").with_source("user123");
        assert_eq!(context.domain, "game");
        assert_eq!(context.source, Some("user123".to_string()));
    }

    #[test]
    fn test_concept_creation() {
        let concept = Concept::new("Elder Rowan", "character", "Elder Rowan was wise", 0.8);
        assert_eq!(concept.text, "Elder Rowan");
        assert_eq!(concept.concept_type, "character");
        assert!(concept.is_significant);
        assert_eq!(concept.significance, 0.8);
    }

    #[test]
    fn test_concept_threshold() {
        let low_sig = Concept::with_threshold("Item", "item", "A small item", 0.3, 0.5);
        assert!(!low_sig.is_significant);

        let high_sig = Concept::with_threshold("Item", "item", "A small item", 0.8, 0.5);
        assert!(high_sig.is_significant);
    }
}
