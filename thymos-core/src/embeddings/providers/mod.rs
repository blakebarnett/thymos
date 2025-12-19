//! Embedding provider implementations

use crate::error::Result;
use async_trait::async_trait;

/// Trait for embedding provider implementations.
///
/// Embedding providers generate vector embeddings from text for semantic search.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single text.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to embed
    ///
    /// # Returns
    ///
    /// Vector embedding as Vec<f32>
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for a batch of texts.
    ///
    /// # Arguments
    ///
    /// * `texts` - Slice of texts to embed
    ///
    /// # Returns
    ///
    /// Vector of embeddings, one per input text
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Default implementation: embed each text sequentially
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }

    /// Get the dimension of embeddings produced by this provider.
    fn dimension(&self) -> usize;
}

#[cfg(feature = "embeddings-local")]
pub mod local;

#[cfg(feature = "embeddings-local")]
pub use local::LocalEmbeddings;
