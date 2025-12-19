//! Local embedding provider using fastembed

use crate::error::{Result, ThymosError};
use async_trait::async_trait;
use std::sync::Mutex;

use super::EmbeddingProvider;

/// Local embedding provider using fastembed (runs locally, no API needed).
pub struct LocalEmbeddings {
    #[cfg(feature = "embeddings-local")]
    model: Mutex<fastembed::TextEmbedding>,
    dimension: usize,
}

impl LocalEmbeddings {
    /// Create a new local embeddings provider with the specified model.
    ///
    /// # Arguments
    ///
    /// * `model_name` - Name of the model (e.g., "all-MiniLM-L6-v2")
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    #[cfg(feature = "embeddings-local")]
    pub fn new(model_name: impl Into<String>) -> Result<Self> {
        let model_name_str = model_name.into();
        
        // Map common model name strings to EmbeddingModel enum variants
        // fastembed::EmbeddingModel::try_from expects specific formats
        let embedding_model = match model_name_str.to_lowercase().as_str() {
            "all-minilm-l6-v2" | "allminimll6v2" | "all_minilm_l6_v2" => {
                fastembed::EmbeddingModel::AllMiniLML6V2
            }
            "all-minilm-l12-v2" | "allminimll12v2" | "all_minilm_l12_v2" => {
                fastembed::EmbeddingModel::AllMiniLML12V2
            }
            "bge-base-en-v1.5" | "bgebaseenv15" | "bge_base_en_v15" => {
                fastembed::EmbeddingModel::BGEBaseENV15
            }
            "bge-small-en-v1.5" | "bgesmallen15" | "bge_small_en_v15" => {
                fastembed::EmbeddingModel::BGESmallENV15
            }
            "bge-large-en-v1.5" | "bgelargeenv15" | "bge_large_en_v15" => {
                fastembed::EmbeddingModel::BGELargeENV15
            }
            "multilingual-e5-base" | "multilinguale5base" | "multilingual_e5_base" => {
                fastembed::EmbeddingModel::MultilingualE5Base
            }
            "multilingual-e5-large" | "multilinguale5large" | "multilingual_e5_large" => {
                fastembed::EmbeddingModel::MultilingualE5Large
            }
            "multilingual-e5-small" | "multilinguale5small" | "multilingual_e5_small" => {
                fastembed::EmbeddingModel::MultilingualE5Small
            }
            _ => {
                // Try direct conversion as fallback
                fastembed::EmbeddingModel::try_from(model_name_str.clone()).map_err(|e| {
                    ThymosError::Configuration(format!(
                        "Invalid model name '{}'. Supported formats: all-MiniLM-L6-v2, all-MiniLM-L12-v2, bge-base-en-v1.5, bge-small-en-v1.5, bge-large-en-v1.5, multilingual-e5-base, multilingual-e5-large, multilingual-e5-small. Error: {}",
                        model_name_str, e
                    ))
                })?
            }
        };
        
        // Get dimension from model enum
        let dimension = match embedding_model {
            fastembed::EmbeddingModel::AllMiniLML6V2 => 384,
            fastembed::EmbeddingModel::AllMiniLML12V2 => 384,
            fastembed::EmbeddingModel::BGEBaseENV15 => 768,
            fastembed::EmbeddingModel::BGESmallENV15 => 384,
            fastembed::EmbeddingModel::BGELargeENV15 => 1024,
            fastembed::EmbeddingModel::MultilingualE5Base => 768,
            fastembed::EmbeddingModel::MultilingualE5Large => 1024,
            fastembed::EmbeddingModel::MultilingualE5Small => 384,
            _ => 384, // Default fallback for any other models
        };
        
        // Create InitOptions - use Default and then set the model_name field
        // InitOptions is non-exhaustive, so we can't use struct literal syntax
        let mut init_options = fastembed::InitOptions::default();
        init_options.model_name = embedding_model;
        
        let model = fastembed::TextEmbedding::try_new(init_options)
            .map_err(|e| {
                ThymosError::Configuration(format!(
                    "Failed to load embedding model '{}': {}",
                    model_name_str, e
                ))
            })?;

        Ok(Self {
            model: Mutex::new(model),
            dimension,
        })
    }

    /// Create with default model: AllMiniLML6V2 (fast, small, good quality).
    ///
    /// Dimension: 384
    #[cfg(feature = "embeddings-local")]
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Result<Self> {
        // Use the enum variant name directly instead of string
        let mut init_options = fastembed::InitOptions::default();
        init_options.model_name = fastembed::EmbeddingModel::AllMiniLML6V2;
        
        let model = fastembed::TextEmbedding::try_new(init_options)
            .map_err(|e| {
                ThymosError::Configuration(format!(
                    "Failed to load default embedding model: {}",
                    e
                ))
            })?;

        Ok(Self {
            model: Mutex::new(model),
            dimension: 384,
        })
    }

    /// Create a stub provider (when feature is disabled).
    #[cfg(not(feature = "embeddings-local"))]
    pub fn new(_model_name: impl Into<String>) -> Result<Self> {
        Err(ThymosError::Configuration(
            "Local embeddings require the 'embeddings-local' feature".to_string(),
        ))
    }

    #[cfg(not(feature = "embeddings-local"))]
    pub fn default() -> Result<Self> {
        Self::new("")
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddings {
    #[cfg(feature = "embeddings-local")]
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut model = self.model.lock().map_err(|e| {
            ThymosError::Configuration(format!("Failed to lock embedding model: {}", e))
        })?;
        
        // fastembed::embed returns Vec<Embedding> where Embedding is Vec<f32>
        let embeddings = model.embed(vec![text.to_string()], None).map_err(|e| {
            ThymosError::Configuration(format!("Failed to generate embedding: {}", e))
        })?;

        if embeddings.is_empty() {
            return Err(ThymosError::Configuration(
                "Embedding generation returned empty result".to_string(),
            ));
        }

        // Convert Embedding (which is Vec<f32>) to Vec<f32>
        Ok(embeddings[0].clone())
    }

    #[cfg(not(feature = "embeddings-local"))]
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(ThymosError::Configuration(
            "Local embeddings require the 'embeddings-local' feature".to_string(),
        ))
    }

    #[cfg(feature = "embeddings-local")]
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let texts_vec: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let mut model = self.model.lock().map_err(|e| {
            ThymosError::Configuration(format!("Failed to lock embedding model: {}", e))
        })?;
        
        // fastembed::embed returns Vec<Embedding> where Embedding is Vec<f32>
        let embeddings = model.embed(texts_vec, None).map_err(|e| {
            ThymosError::Configuration(format!("Failed to generate batch embeddings: {}", e))
        })?;

        // Convert Vec<Embedding> to Vec<Vec<f32>>
        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
