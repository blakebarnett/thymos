//! Embedding provider implementations for generating text embeddings.

pub mod factory;
pub mod providers;

pub use factory::EmbeddingProviderFactory;
pub use providers::EmbeddingProvider;

pub mod prelude {
    pub use crate::embeddings::EmbeddingProvider;
    #[cfg(feature = "embeddings-local")]
    pub use crate::embeddings::providers::LocalEmbeddings;
}
