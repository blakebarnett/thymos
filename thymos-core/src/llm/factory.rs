//! Factory for creating LLM providers from configuration

use crate::config::{LLMProvider as LLMProviderType, LLMProviderConfig};
use crate::error::Result;
use crate::llm::LLMProvider;
use std::sync::Arc;

#[cfg(feature = "llm-groq")]
use crate::llm::providers::groq::GroqProvider;

#[cfg(feature = "llm-ollama")]
use crate::llm::providers::ollama::OllamaProvider;

#[cfg(feature = "llm-openai")]
use crate::llm::providers::openai::OpenAIProvider;

#[cfg(feature = "llm-anthropic")]
use crate::llm::providers::anthropic::AnthropicProvider;

/// Factory for creating LLM providers
pub struct LLMProviderFactory;

impl LLMProviderFactory {
    /// Create an LLM provider from configuration
    ///
    /// # Arguments
    ///
    /// * `config` - LLM provider configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the provider cannot be created (e.g., missing API key)
    pub async fn create(config: &LLMProviderConfig) -> Result<Arc<dyn LLMProvider>> {
        match config.provider {
            #[cfg(feature = "llm-groq")]
            LLMProviderType::Groq => {
                // Use model from config, or GROQ_MODEL env var, or default
                let model = if !config.model.is_empty() {
                    Some(config.model.clone())
                } else {
                    None
                };
                
                // If api_key is provided in config, use GroqProvider::new, otherwise use from_env
                let provider = if let Some(api_key) = &config.api_key {
                    let model_str = model
                        .or_else(|| std::env::var("GROQ_MODEL").ok())
                        .unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());
                    GroqProvider::new(api_key.clone(), model_str)
                } else {
                    GroqProvider::from_env(model)?
                };
                
                Ok(Arc::new(provider))
            }

            #[cfg(not(feature = "llm-groq"))]
            LLMProviderType::Groq => Err(crate::error::ThymosError::Configuration(
                "Groq provider requires 'llm-groq' feature".to_string(),
            )),

            #[cfg(feature = "llm-openai")]
            LLMProviderType::OpenAI => {
                let model = if !config.model.is_empty() {
                    Some(config.model.clone())
                } else {
                    None
                };

                let provider = if let Some(api_key) = &config.api_key {
                    let model_str = model
                        .or_else(|| std::env::var("OPENAI_MODEL").ok())
                        .unwrap_or_else(|| "gpt-4o".to_string());

                    if let Some(base_url) = &config.base_url {
                        OpenAIProvider::with_base_url(api_key.clone(), model_str, base_url.clone())
                    } else {
                        OpenAIProvider::new(api_key.clone(), model_str)
                    }
                } else {
                    OpenAIProvider::from_env(model)?
                };

                Ok(Arc::new(provider))
            }

            #[cfg(not(feature = "llm-openai"))]
            LLMProviderType::OpenAI => Err(crate::error::ThymosError::Configuration(
                "OpenAI provider requires 'llm-openai' feature".to_string(),
            )),

            #[cfg(feature = "llm-ollama")]
            LLMProviderType::Ollama => {
                let model = if !config.model.is_empty() {
                    Some(config.model.clone())
                } else {
                    None
                };

                let base_url = config.base_url.clone();

                let provider = if let Some(url) = base_url {
                    OllamaProvider::new(
                        model.unwrap_or_else(|| "qwen3:14b".to_string()),
                        Some(url),
                    )
                } else {
                    OllamaProvider::from_env(model)?
                };

                Ok(Arc::new(provider))
            }

            #[cfg(not(feature = "llm-ollama"))]
            LLMProviderType::Ollama => Err(crate::error::ThymosError::Configuration(
                "Ollama provider requires 'llm-ollama' feature".to_string(),
            )),

            #[cfg(feature = "llm-anthropic")]
            LLMProviderType::Anthropic => {
                let model = if !config.model.is_empty() {
                    Some(config.model.clone())
                } else {
                    None
                };

                let provider = if let Some(api_key) = &config.api_key {
                    let model_str = model
                        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
                        .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

                    if let Some(base_url) = &config.base_url {
                        AnthropicProvider::with_base_url(api_key.clone(), model_str, base_url.clone())
                    } else {
                        AnthropicProvider::new(api_key.clone(), model_str)
                    }
                } else {
                    AnthropicProvider::from_env(model)?
                };

                Ok(Arc::new(provider))
            }

            #[cfg(not(feature = "llm-anthropic"))]
            LLMProviderType::Anthropic => Err(crate::error::ThymosError::Configuration(
                "Anthropic provider requires 'llm-anthropic' feature".to_string(),
            )),
        }
    }

    /// Create from ThymosConfig (if LLM config is present)
    pub async fn from_config(
        config: Option<&LLMProviderConfig>,
    ) -> Result<Option<Arc<dyn LLMProvider>>> {
        match config {
            Some(cfg) => Ok(Some(Self::create(cfg).await?)),
            None => Ok(None),
        }
    }
}
