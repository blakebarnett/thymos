//! Factory for creating embedding providers from configuration

use crate::config::{EmbeddingProvider as EmbeddingProviderType, EmbeddingsConfig};
use crate::embeddings::providers::EmbeddingProvider;
use crate::error::Result;
use std::sync::Arc;

#[cfg(feature = "embeddings-local")]
use crate::embeddings::providers::local::LocalEmbeddings;

/// Factory for creating embedding providers
pub struct EmbeddingProviderFactory;

impl EmbeddingProviderFactory {
    /// Create an embedding provider from configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Embeddings configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the provider cannot be created
    pub async fn create(config: &EmbeddingsConfig) -> Result<Arc<dyn EmbeddingProvider>> {
        match config.provider {
            #[cfg(feature = "embeddings-local")]
            EmbeddingProviderType::Local => {
                let provider = LocalEmbeddings::new(config.model.clone())?;
                Ok(Arc::new(provider))
            }

            #[cfg(not(feature = "embeddings-local"))]
            EmbeddingProviderType::Local => Err(crate::error::ThymosError::Configuration(
                "Local embeddings require 'embeddings-local' feature".to_string(),
            )),

            EmbeddingProviderType::OpenAI => Err(crate::error::ThymosError::Configuration(
                "OpenAI embeddings not yet implemented".to_string(),
            )),

            EmbeddingProviderType::Ollama => Err(crate::error::ThymosError::Configuration(
                "Ollama embeddings not yet implemented".to_string(),
            )),
        }
    }

    /// Create from ThymosConfig (if embeddings config is present)
    pub async fn from_config(
        config: Option<&EmbeddingsConfig>,
    ) -> Result<Option<Arc<dyn EmbeddingProvider>>> {
        match config {
            Some(cfg) => Ok(Some(Self::create(cfg).await?)),
            None => Ok(None),
        }
    }
}
