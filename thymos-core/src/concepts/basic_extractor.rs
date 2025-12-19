use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

use crate::error::{Result, ThymosError};

use super::config::ConceptExtractionConfig;
use super::traits::ConceptExtractor;
use super::types::{Concept, Context};

/// Cache for compiled regex patterns to avoid recompilation.
static REGEX_CACHE: Lazy<std::sync::Mutex<HashMap<String, Regex>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// Basic concept extractor using regex patterns and significance scoring.
///
/// This implementation provides pattern-based concept extraction suitable for
/// most text sources. It supports configurable patterns and significance thresholds.
pub struct BasicConceptExtractor {
    config: ConceptExtractionConfig,
}

impl BasicConceptExtractor {
    /// Create a new BasicConceptExtractor with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if any regex patterns are invalid.
    pub fn new(config: ConceptExtractionConfig) -> Result<Self> {
        // Validate all patterns compile successfully
        for type_config in config.concept_types.values() {
            if !type_config.enabled {
                continue;
            }

            for pattern in &type_config.patterns {
                Self::compile_pattern(pattern)?;
            }
        }

        Ok(Self { config })
    }

    /// Get or compile a regex pattern, using cache when available.
    fn compile_pattern(pattern: &str) -> Result<Regex> {
        let mut cache = REGEX_CACHE.lock().map_err(|e| {
            ThymosError::Configuration(format!("Failed to lock regex cache: {}", e))
        })?;

        if let Some(regex) = cache.get(pattern) {
            return Ok(regex.clone());
        }

        let regex = Regex::new(pattern).map_err(|e| {
            ThymosError::Configuration(format!("Invalid regex pattern '{}': {}", pattern, e))
        })?;

        cache.insert(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Extract a context snippet around a match.
    fn extract_context(text: &str, start: usize, end: usize, context_chars: usize) -> String {
        let context_start = start.saturating_sub(context_chars);
        let context_end = (end + context_chars).min(text.len());

        text[context_start..context_end].trim().chars().collect()
    }

    /// Score a concept match based on various factors.
    fn score_concept(
        base_significance: f64,
        text: &str,
        position: usize,
        total_length: usize,
    ) -> f64 {
        // Start with base significance
        let mut score = base_significance;

        // Boost for early mentions (more prominent)
        let position_ratio = position as f64 / total_length as f64;
        if position_ratio < 0.25 {
            score += 0.15;
        } else if position_ratio < 0.5 {
            score += 0.05;
        }

        // Boost for length (longer names often more specific)
        if text.len() > 10 {
            score += 0.1;
        }

        // Clamp to valid range
        score.clamp(0.0, 1.0)
    }
}

#[async_trait::async_trait]
impl ConceptExtractor for BasicConceptExtractor {
    async fn extract(&self, text: &str, _context: Option<&Context>) -> Result<Vec<Concept>> {
        let mut concepts = Vec::new();
        let mut seen_texts = std::collections::HashSet::new();

        // Process each concept type
        for (type_id, type_config) in &self.config.concept_types {
            if !type_config.enabled {
                continue;
            }

            // Try each pattern for this type
            for pattern in &type_config.patterns {
                let regex = Self::compile_pattern(pattern)?;

                // Find all matches
                for mat in regex.captures_iter(text) {
                    // Use the first capture group, or the whole match
                    let matched_text = mat
                        .get(1)
                        .map(|m| m.as_str())
                        .or_else(|| mat.get(0).map(|m| m.as_str()))
                        .unwrap_or("");

                    if matched_text.is_empty() {
                        continue;
                    }

                    // Skip duplicates to avoid noise
                    let concept_key = format!("{},{}", type_id, matched_text);
                    if seen_texts.contains(&concept_key) {
                        continue;
                    }
                    seen_texts.insert(concept_key);

                    // Calculate significance
                    let significance = Self::score_concept(
                        type_config.base_significance,
                        matched_text,
                        mat.get(0).unwrap().start(),
                        text.len(),
                    );

                    // Only include if meets threshold
                    if significance < self.config.significance_threshold {
                        continue;
                    }

                    // Extract context
                    let context_str = Self::extract_context(
                        text,
                        mat.get(0).unwrap().start(),
                        mat.get(0).unwrap().end(),
                        50,
                    );

                    let concept = Concept::with_threshold(
                        matched_text,
                        type_id.clone(),
                        context_str,
                        significance,
                        self.config.significance_threshold,
                    );

                    concepts.push(concept);
                }
            }
        }

        // Sort by significance (highest first)
        concepts.sort_by(|a, b| b.significance.partial_cmp(&a.significance).unwrap());

        Ok(concepts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_extraction() {
        let extractor = BasicConceptExtractor::new(ConceptExtractionConfig::default()).unwrap();
        let text = "Elder Rowan lives in Oakshire";

        let concepts = extractor.extract(text, None).await.unwrap();

        assert!(!concepts.is_empty());
        assert!(concepts.iter().any(|c| c.concept_type == "character"));
    }

    #[tokio::test]
    async fn test_significance_filtering() {
        let config = ConceptExtractionConfig::new().with_threshold(0.8);
        let extractor = BasicConceptExtractor::new(config).unwrap();

        let text = "A small text with minimal content";
        let concepts = extractor.extract(text, None).await.unwrap();

        // All returned concepts should meet threshold
        for concept in &concepts {
            assert!(concept.significance >= 0.8);
        }
    }

    #[tokio::test]
    async fn test_duplicate_handling() {
        let extractor = BasicConceptExtractor::new(ConceptExtractionConfig::default()).unwrap();
        let text = "Elder Rowan and Elder Rowan met. Elder Rowan was wise.";

        let concepts = extractor.extract(text, None).await.unwrap();

        // Should not have duplicate "Elder Rowan" entries
        let rowan_count = concepts.iter().filter(|c| c.text.contains("Rowan")).count();
        assert_eq!(rowan_count, 1);
    }

    #[tokio::test]
    async fn test_invalid_regex() {
        let config = ConceptExtractionConfig::new().with_concept_type(
            "bad",
            super::super::config::ConceptTypeConfig::new("Bad", vec!["[invalid".to_string()]),
        );

        let result = BasicConceptExtractor::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_extraction() {
        let text = "The quick brown fox jumps over the lazy dog";
        let context = BasicConceptExtractor::extract_context(text, 4, 9, 20);
        assert!(context.contains("brown"));
    }
}
