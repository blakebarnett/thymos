//! RPG concept configuration for Zera NPCs

use thymos_core::concepts::ConceptExtractionConfig;
use thymos_core::concepts::config::ConceptTypeConfig;

/// Create RPG-specific concept extraction configuration
#[allow(dead_code)]
pub fn create_rpg_concept_config() -> ConceptExtractionConfig {
    ConceptExtractionConfig::new()
        .with_concept_type(
            "character",
            ConceptTypeConfig::new(
                "NPCs and player characters",
                vec![
                    r"(?:Elder|Sir|Lady|Captain)\s+(\w+)".to_string(),
                    r"(\w+)\s+(?:the|of)\s+(\w+)".to_string(),
                ],
            )
            .with_base_significance(0.9),
        )
        .with_concept_type(
            "location",
            ConceptTypeConfig::new(
                "Places in the game world",
                vec![
                    r"(?:village|town|city)\s+of\s+(\w+)".to_string(),
                    r"the\s+(\w+)\s+(?:forest|dungeon|castle)".to_string(),
                ],
            )
            .with_base_significance(0.8),
        )
        .with_concept_type(
            "item",
            ConceptTypeConfig::new(
                "Objects and equipment",
                vec![r"(?:legendary|magical|ancient)\s+(\w+)".to_string()],
            )
            .with_base_significance(0.5),
        )
        .with_threshold(0.6)
}
