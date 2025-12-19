use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::Result;

use super::alias::{Alias, AliasProvenance, AliasType};

/// Cache for alias extraction patterns.
static ALIAS_PATTERNS: Lazy<Vec<AliasPattern>> = Lazy::new(|| {
    vec![
        // Explicit alias patterns: "known as", "called", etc.
        AliasPattern {
            pattern: r#"(?:known as|called|nicknamed|aka)\s+['"]?([^'",\.]+)['"]?"#,
            confidence_boost: 0.85,
            alias_type: AliasType::Alias,
        },
        // Self-reference patterns: "I am", "I'm"
        AliasPattern {
            pattern: r#"(?:I am|I'm)\s+['"]?([^'",\.]+)['"]?"#,
            confidence_boost: 0.95,
            alias_type: AliasType::Alias,
        },
        // Epithet patterns: "the adjective noun"
        AliasPattern {
            pattern: r#"(?:the\s+(?:\w+\s+)*\w+(?:,|\s+(?:was|were|is)))"#,
            confidence_boost: 0.65,
            alias_type: AliasType::Epithet,
        },
        // Title patterns: "Dr.", "Captain", "King", etc.
        AliasPattern {
            pattern: r#"(?:Dr\.?|Professor|Captain|King|Queen|Lord|Lady|Sir|Dame|Mr\.?|Mrs\.?|Ms\.?)\s+([A-Z][a-z]+)"#,
            confidence_boost: 0.80,
            alias_type: AliasType::Title,
        },
    ]
});

/// Pattern configuration for alias extraction.
struct AliasPattern {
    pattern: &'static str,
    confidence_boost: f64,
    alias_type: AliasType,
}

/// Extract potential aliases from text for a given concept.
pub struct AliasExtractor;

impl AliasExtractor {
    /// Extract aliases for a concept from text.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to extract aliases from
    /// * `canonical_name` - The canonical name of the concept
    ///
    /// # Returns
    ///
    /// A vector of extracted aliases with confidence scores
    pub fn extract_aliases(text: &str, canonical_name: &str) -> Result<Vec<Alias>> {
        let mut aliases = Vec::new();

        for pattern in ALIAS_PATTERNS.iter() {
            if let Ok(regex) = Regex::new(pattern.pattern) {
                for mat in regex.captures_iter(text) {
                    let alias_text = mat
                        .get(1)
                        .map(|m| m.as_str())
                        .or_else(|| mat.get(0).map(|m| m.as_str()))
                        .unwrap_or("");

                    if alias_text.is_empty() || alias_text == canonical_name {
                        continue;
                    }

                    let confidence = Self::calculate_confidence(
                        alias_text,
                        canonical_name,
                        pattern.confidence_boost,
                    );

                    let alias = Alias::new(
                        alias_text,
                        pattern.alias_type,
                        AliasProvenance::Narrator,
                        confidence,
                    );

                    aliases.push(alias);
                }
            }
        }

        // Sort by confidence (highest first) and remove duplicates
        aliases.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        aliases.dedup_by(|a, b| a.text == b.text);

        Ok(aliases)
    }

    /// Calculate confidence score for an alias.
    fn calculate_confidence(alias_text: &str, canonical_name: &str, base_confidence: f64) -> f64 {
        let mut confidence = base_confidence;

        // Boost if alias contains canonical name
        if canonical_name
            .to_lowercase()
            .contains(&alias_text.to_lowercase())
        {
            confidence += 0.1;
        }

        // Length-based adjustments
        let length_ratio = alias_text.len() as f64 / canonical_name.len().max(1) as f64;
        if (0.3..=3.0).contains(&length_ratio) {
            confidence += 0.05;
        }

        confidence.clamp(0.0, 1.0)
    }

    /// Resolve an alias to its canonical form by checking similarity.
    ///
    /// Returns (canonical_name, confidence) if a reasonable match is found.
    pub fn resolve_alias(alias_text: &str, candidates: &[&str]) -> Option<(String, f64)> {
        let mut best_match: Option<(String, f64)> = None;

        for candidate in candidates {
            let similarity = Self::string_similarity(alias_text, candidate);

            // Accept matches with >60% similarity
            if similarity > 0.6 {
                if let Some((_, best_sim)) = &best_match {
                    if similarity > *best_sim {
                        best_match = Some((candidate.to_string(), similarity));
                    }
                } else {
                    best_match = Some((candidate.to_string(), similarity));
                }
            }
        }

        best_match
    }

    /// Calculate similarity between two strings (0.0-1.0) using simple edit distance.
    fn string_similarity(a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        // Exact match
        if a_lower == b_lower {
            return 1.0;
        }

        // Substring match
        if a_lower.contains(&b_lower) || b_lower.contains(&a_lower) {
            return 0.9;
        }

        // Simple character overlap metric
        let a_chars: std::collections::HashSet<_> = a_lower.chars().collect();
        let b_chars: std::collections::HashSet<_> = b_lower.chars().collect();
        let intersection = a_chars.intersection(&b_chars).count();
        let union = a_chars.union(&b_chars).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_extraction() {
        let text = "Elder Rowan, known as the old badger, lived in peace.";
        let aliases = AliasExtractor::extract_aliases(text, "Elder Rowan").unwrap();

        assert!(!aliases.is_empty());
        // Should find "the old badger" as an epithet
        assert!(aliases.iter().any(|a| a.text.contains("badger")));
    }

    #[test]
    fn test_alias_resolution() {
        let candidates = vec!["Elder Rowan", "Rowan", "The Badger"];
        let result = AliasExtractor::resolve_alias("rowan", &candidates);

        assert!(result.is_some());
        let (canonical, confidence) = result.unwrap();
        assert!(canonical == "Elder Rowan" || canonical == "Rowan");
        assert!(confidence > 0.6);
    }

    #[test]
    fn test_string_similarity() {
        let sim_exact = AliasExtractor::string_similarity("test", "test");
        assert_eq!(sim_exact, 1.0);

        let sim_partial = AliasExtractor::string_similarity("test", "tes");
        assert!(sim_partial > 0.5);

        let sim_different = AliasExtractor::string_similarity("abc", "xyz");
        assert!(sim_different < 0.5);
    }

    #[test]
    fn test_confidence_calculation() {
        let conf1 = AliasExtractor::calculate_confidence("Rowan", "Elder Rowan", 0.85);
        assert!(conf1 > 0.85); // Should get boost for containing canonical name

        let conf2 = AliasExtractor::calculate_confidence("Badger", "Elder Rowan", 0.65);
        // Should be clamped to 1.0 at most, actual value depends on calculation
        assert!(conf2 >= 0.0 && conf2 <= 1.0);
    }
}
