use async_trait::async_trait;

use crate::error::Result;

use super::types::{Concept, Context};

/// Core trait for concept extraction implementations.
///
/// Implementors should extract concepts/entities from text in a domain-specific way.
/// The trait supports both pattern-based and LLM-based extraction strategies.
#[async_trait]
pub trait ConceptExtractor: Send + Sync {
    /// Extract concepts from the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - The input text to extract concepts from
    /// * `context` - Optional context information for domain-specific extraction
    ///
    /// # Returns
    ///
    /// A vector of extracted concepts, sorted by significance (highest first)
    async fn extract(&self, text: &str, context: Option<&Context>) -> Result<Vec<Concept>>;
}
