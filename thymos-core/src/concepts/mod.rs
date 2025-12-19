//! Concept extraction and tracking system for domain-agnostic entity identification.
//!
//! This module provides traits and implementations for extracting concepts (entities,
//! characters, locations, items, etc.) from text in a domain-agnostic way. Concepts
//! are automatically tracked, promoted based on significance, and integrated with the
//! memory system.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use thymos_core::concepts::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> thymos_core::error::Result<()> {
//!     let config = ConceptExtractionConfig::default();
//!     let extractor = BasicConceptExtractor::new(config)?;
//!
//!     let text = "Elder Rowan lives in the village of Oakshire";
//!     let concepts = extractor.extract(text, None).await?;
//!
//!     for concept in concepts {
//!         println!("Found {:?}: {}", concept.concept_type, concept.text);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod alias;
pub mod alias_extractor;
pub mod basic_extractor;
pub mod config;
#[cfg(feature = "llm-groq")]
pub mod llm_extractor;
pub mod promotion;
pub mod tier;
pub mod traits;
pub mod types;

pub use alias::{Alias, AliasProvenance, AliasType};
pub use alias_extractor::AliasExtractor;
pub use basic_extractor::BasicConceptExtractor;
pub use config::ConceptExtractionConfig;
#[cfg(feature = "llm-groq")]
pub use llm_extractor::{LLMConceptExtractor, LLMExtractionConfig};
pub use promotion::{ConceptMention, ConceptPromotionPipeline, PromotionConfig, PromotionStats};
pub use tier::ConceptTier;
pub use traits::ConceptExtractor;
pub use types::{Concept, Context};

/// Create a ready-to-use concept extractor with sensible defaults
///
/// This function creates a concept extractor that:
/// - Uses regex patterns for fast initial extraction
/// - Optionally enhances with LLM validation/extraction if LLM is configured
///
/// # Arguments
///
/// * `config` - Optional Thymos configuration (if provided, will use LLM if configured)
///
/// # Returns
///
/// A concept extractor ready to use
pub async fn create_default_extractor(
    config: Option<&crate::config::ThymosConfig>,
) -> crate::error::Result<std::sync::Arc<dyn ConceptExtractor>> {
    use std::sync::Arc;

    // Create base regex extractor with common patterns
    let base_config = ConceptExtractionConfig::default();
    let base_extractor = Arc::new(BasicConceptExtractor::new(base_config)?);

    // If LLM config available, enhance with LLM
    #[cfg(feature = "llm-groq")]
    {
        if let Some(config) = config {
            return Ok(Arc::new(
                LLMConceptExtractor::from_config(base_extractor, config).await?,
            ));
        }
        // No config provided, use base extractor wrapped in LLM extractor
        Ok(Arc::new(LLMConceptExtractor::without_llm(base_extractor)))
    }
    #[cfg(not(feature = "llm-groq"))]
    {
        // LLM feature not enabled, just return base extractor
        let _ = config; // Acknowledge parameter even if unused
        Ok(base_extractor)
    }
}

pub mod prelude {
    pub use crate::concepts::{
        Alias, AliasExtractor, AliasProvenance, AliasType, BasicConceptExtractor, Concept,
        ConceptExtractionConfig, ConceptExtractor, ConceptMention, ConceptPromotionPipeline,
        ConceptTier, Context, PromotionConfig, PromotionStats,
    };
    #[cfg(feature = "llm-groq")]
    pub use crate::concepts::{LLMConceptExtractor, LLMExtractionConfig};
}
