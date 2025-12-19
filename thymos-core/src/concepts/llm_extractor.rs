//! LLM-enhanced concept extractor that combines regex patterns with LLM validation and extraction

use std::collections::HashSet;
use std::sync::Arc;

use crate::config::ThymosConfig;
use crate::error::{Result, ThymosError};
use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};

use super::traits::ConceptExtractor;
use super::types::{Concept, Context};

/// Configuration for LLM-enhanced concept extraction
#[derive(Debug, Clone)]
pub struct LLMExtractionConfig {
    /// Use LLM for validation (recommended)
    pub use_llm_validation: bool,

    /// Use LLM for extraction (slower, more accurate)
    pub use_llm_extraction: bool,

    /// Temperature for LLM extraction
    pub temperature: f32,
}

impl Default for LLMExtractionConfig {
    fn default() -> Self {
        Self {
            use_llm_validation: true,
            use_llm_extraction: false,
            temperature: 0.3, // Lower temperature for more consistent extraction
        }
    }
}

impl LLMExtractionConfig {
    /// Create a new config with validation only (recommended)
    pub fn validation_only() -> Self {
        Self {
            use_llm_validation: true,
            use_llm_extraction: false,
            temperature: 0.3,
        }
    }

    /// Create a config with both validation and extraction
    pub fn full_extraction() -> Self {
        Self {
            use_llm_validation: true,
            use_llm_extraction: true,
            temperature: 0.3,
        }
    }
}

/// LLM-enhanced concept extractor that combines regex patterns with LLM validation/enhancement
pub struct LLMConceptExtractor {
    base_extractor: Arc<dyn ConceptExtractor>,
    llm: Option<Arc<dyn LLMProvider>>,
    config: LLMExtractionConfig,
}

impl LLMConceptExtractor {
    /// Create with LLM provider
    pub fn new(
        base_extractor: Arc<dyn ConceptExtractor>,
        llm: Arc<dyn LLMProvider>,
        config: LLMExtractionConfig,
    ) -> Self {
        Self {
            base_extractor,
            llm: Some(llm),
            config,
        }
    }

    /// Create without LLM (falls back to base extractor)
    pub fn without_llm(base_extractor: Arc<dyn ConceptExtractor>) -> Self {
        Self {
            base_extractor,
            llm: None,
            config: LLMExtractionConfig::default(),
        }
    }

    /// Create with auto-detected LLM from config
    pub async fn from_config(
        base_extractor: Arc<dyn ConceptExtractor>,
        config: &ThymosConfig,
    ) -> Result<Self> {
        if let Some(llm_config) = &config.llm {
            let llm = crate::llm::LLMProviderFactory::from_config(Some(llm_config))
                .await?
                .ok_or_else(|| {
                    ThymosError::Configuration(
                        "LLM config present but provider creation failed".to_string(),
                    )
                })?;
            Ok(Self::new(
                base_extractor,
                llm,
                LLMExtractionConfig::default(),
            ))
        } else {
            Ok(Self::without_llm(base_extractor))
        }
    }

    /// Validate concepts with LLM (filter false positives, adjust significance)
    async fn validate_with_llm(
        &self,
        concepts: Vec<Concept>,
        text: &str,
        _context: Option<&Context>,
    ) -> Result<Vec<Concept>> {
        let llm = self.llm.as_ref().ok_or_else(|| {
            ThymosError::Configuration("LLM not available for validation".to_string())
        })?;

        if concepts.is_empty() {
            return Ok(concepts);
        }

        // Build validation prompt
        let concepts_json = serde_json::to_string(&concepts).map_err(|e| {
            ThymosError::Configuration(format!("Failed to serialize concepts: {}", e))
        })?;

        let prompt = format!(
            r#"You are validating extracted concepts from text. Review each concept and determine if it's a valid entity/concept.

Text: {}
Extracted concepts: {}

For each concept, return a JSON array with objects containing:
- "text": the concept text (exact match)
- "valid": boolean (true if it's a valid concept)
- "significance": float 0.0-1.0 (adjusted significance score)
- "reason": string (brief reason for validation decision)

Return ONLY valid JSON, no markdown formatting."#,
            text, concepts_json
        );

        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content:
                        "You are a concept validation assistant. Return only valid JSON arrays."
                            .to_string(),
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(self.config.temperature),
            max_tokens: Some(2000),
            stop_sequences: Vec::new(),
        };

        let response = llm.generate_structured(&request, None).await?;

        // Parse validation results
        #[derive(serde::Deserialize)]
        struct ValidationResult {
            text: String,
            valid: bool,
            significance: f64,
            #[serde(default)]
            #[allow(dead_code)]
            reason: String,
        }

        let validations: Vec<ValidationResult> = serde_json::from_value(response).map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse validation results: {}", e))
        })?;

        // Apply validations to concepts
        let mut validated_concepts = Vec::new();
        for concept in concepts {
            if let Some(validation) = validations.iter().find(|v| v.text == concept.text) {
                if validation.valid {
                    let mut validated = concept.clone();
                    validated.significance = validation.significance.clamp(0.0, 1.0);
                    validated.is_significant = validated.significance >= 0.5;
                    validated_concepts.push(validated);
                }
                // Skip invalid concepts
            } else {
                // If LLM didn't return validation for this concept, keep it but lower significance
                let mut kept = concept.clone();
                kept.significance = (kept.significance * 0.7).clamp(0.0, 1.0);
                kept.is_significant = kept.significance >= 0.5;
                validated_concepts.push(kept);
            }
        }

        Ok(validated_concepts)
    }

    /// Extract concepts using LLM only (for things regex might miss)
    async fn extract_with_llm(
        &self,
        text: &str,
        _context: Option<&Context>,
    ) -> Result<Vec<Concept>> {
        let llm = self.llm.as_ref().ok_or_else(|| {
            ThymosError::Configuration("LLM not available for extraction".to_string())
        })?;

        let prompt = format!(
            r#"Extract all important concepts/entities from the following text. Include:
- Characters/people (names, titles, roles)
- Locations (places, regions, landmarks)
- Organizations (groups, companies, institutions)
- Items/objects (important items, artifacts, tools)

Text: {}

Return a JSON array of concepts, each with:
- "text": the extracted text
- "concept_type": one of "character", "location", "organization", "item", or other appropriate type
- "significance": float 0.0-1.0 (how important this concept is)
- "context": a short snippet showing where it appeared

Return ONLY valid JSON, no markdown formatting."#,
            text
        );

        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are a concept extraction assistant. Extract entities and concepts from text. Return only valid JSON arrays.".to_string(),
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(self.config.temperature),
            max_tokens: Some(2000),
            stop_sequences: Vec::new(),
        };

        let response = llm.generate_structured(&request, None).await?;

        #[derive(serde::Deserialize)]
        struct LLMConcept {
            text: String,
            concept_type: String,
            significance: f64,
            context: String,
        }

        let llm_concepts: Vec<LLMConcept> = serde_json::from_value(response).map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse LLM extraction results: {}", e))
        })?;

        let concepts: Vec<Concept> = llm_concepts
            .into_iter()
            .map(|c| {
                Concept::with_threshold(
                    c.text,
                    c.concept_type,
                    c.context,
                    c.significance.clamp(0.0, 1.0),
                    0.5,
                )
            })
            .collect();

        Ok(concepts)
    }

    /// Deduplicate concepts and sort by significance
    fn deduplicate_and_sort(&self, mut concepts: Vec<Concept>) -> Result<Vec<Concept>> {
        // Deduplicate by text (case-insensitive)
        let mut seen = HashSet::new();
        let mut unique_concepts = Vec::new();

        for concept in concepts.drain(..) {
            let key = concept.text.to_lowercase();
            if !seen.contains(&key) {
                seen.insert(key);
                unique_concepts.push(concept);
            } else if let Some(existing) = unique_concepts
                .iter_mut()
                .find(|c| c.text.to_lowercase() == concept.text.to_lowercase())
            {
                // If duplicate, keep the one with higher significance
                if concept.significance > existing.significance {
                    *existing = concept;
                }
            }
        }

        // Sort by significance (highest first)
        unique_concepts.sort_by(|a, b| {
            b.significance
                .partial_cmp(&a.significance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(unique_concepts)
    }
}

#[async_trait::async_trait]
impl ConceptExtractor for LLMConceptExtractor {
    async fn extract(&self, text: &str, context: Option<&Context>) -> Result<Vec<Concept>> {
        // Step 1: Use base extractor (fast regex)
        let mut concepts = self.base_extractor.extract(text, context).await?;

        // Step 2: If LLM available, validate and enhance
        if let Some(_llm) = &self.llm {
            if self.config.use_llm_validation {
                concepts = self.validate_with_llm(concepts, text, context).await?;
            }

            if self.config.use_llm_extraction {
                // Also try LLM-only extraction for things regex might miss
                let llm_concepts = self.extract_with_llm(text, context).await?;
                concepts.extend(llm_concepts);
            }
        }

        // Deduplicate and sort by significance
        self.deduplicate_and_sort(concepts)
    }
}
