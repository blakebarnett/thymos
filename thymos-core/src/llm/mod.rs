use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::pin::Pin;

use crate::error::Result;

/// Helper function to generate structured output and deserialize it.
///
/// This is a convenience wrapper around `generate_structured` that handles deserialization.
pub async fn generate_structured_output<T: DeserializeOwned>(
    provider: &dyn LLMProvider,
    request: &LLMRequest,
    schema: Option<serde_json::Value>,
) -> Result<T> {
    let json = provider.generate_structured(request, schema).await?;
    serde_json::from_value(json).map_err(|e| {
        crate::error::ThymosError::Configuration(format!(
            "Failed to deserialize structured output: {}",
            e
        ))
    })
}

/// Configuration for LLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Temperature for generation (0.0-2.0, default: 0.7)
    pub temperature: f32,

    /// Maximum tokens to generate (default: 500)
    pub max_tokens: usize,

    /// System prompt for context
    pub system_prompt: Option<String>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 500,
            system_prompt: None,
        }
    }
}

impl LLMConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.0, 2.0);
        self
    }

    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Request to an LLM provider
#[derive(Debug, Clone)]
pub struct LLMRequest {
    /// Messages in the conversation
    pub messages: Vec<Message>,

    /// Temperature for generation (0.0-2.0)
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,

    /// Stop sequences
    pub stop_sequences: Vec<String>,
}

impl LLMRequest {
    /// Create a simple request from a single prompt
    pub fn from_prompt(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt.into(),
            }],
            temperature: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
        }
    }

    /// Create a request with system prompt
    pub fn with_system_prompt(
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
    ) -> Self {
        Self {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: system_prompt.into(),
                },
                Message {
                    role: MessageRole::User,
                    content: user_prompt.into(),
                },
            ],
            temperature: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
        }
    }
}

/// Response from an LLM provider
#[derive(Debug, Clone)]
pub struct LLMResponse {
    /// Generated content
    pub content: String,

    /// Token usage information
    pub usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Trait for LLM provider implementations.
///
/// Consolidation engine supports custom LLM implementations through this trait.
/// Implementors should handle actual LLM calls (OpenAI, Claude, etc.).
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate text using the LLM (legacy method for backward compatibility).
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to send to the LLM
    /// * `config` - LLM configuration options
    ///
    /// # Returns
    ///
    /// The generated text
    async fn generate(&self, prompt: &str, config: &LLMConfig) -> Result<String> {
        // Default implementation converts to new API
        let request = LLMRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt.to_string(),
            }],
            temperature: Some(config.temperature),
            max_tokens: Some(config.max_tokens),
            stop_sequences: Vec::new(),
        };

        let response = self.generate_request(&request).await?;
        Ok(response.content)
    }

    /// Generate text from a structured request (new API).
    ///
    /// # Arguments
    ///
    /// * `request` - The LLM request with messages and configuration
    ///
    /// # Returns
    ///
    /// The LLM response
    async fn generate_request(&self, request: &LLMRequest) -> Result<LLMResponse> {
        // Default implementation converts to legacy API
        let prompt = request
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let config = LLMConfig {
            temperature: request.temperature.unwrap_or(0.7),
            max_tokens: request.max_tokens.unwrap_or(500),
            system_prompt: request
                .messages
                .iter()
                .find(|m| m.role == MessageRole::System)
                .map(|m| m.content.clone()),
        };

        let content = self.generate(&prompt, &config).await?;
        Ok(LLMResponse {
            content,
            usage: None,
        })
    }

    /// Generate structured output (JSON) as a JSON value.
    ///
    /// # Arguments
    ///
    /// * `request` - The LLM request
    /// * `schema` - Optional JSON schema for structured output
    ///
    /// # Returns
    ///
    /// JSON value that can be deserialized
    async fn generate_structured(
        &self,
        request: &LLMRequest,
        _schema: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let response = self.generate_request(request).await?;
        serde_json::from_str(&response.content).map_err(|e| {
            crate::error::ThymosError::Configuration(format!(
                "Failed to parse structured output: {}",
                e
            ))
        })
    }

    /// Generate with streaming response (optional).
    ///
    /// # Arguments
    ///
    /// * `request` - The LLM request
    ///
    /// # Returns
    ///
    /// Stream of text chunks
    async fn generate_stream(
        &self,
        _request: &LLMRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        Err(crate::error::ThymosError::Configuration(
            "Streaming not supported by this provider".to_string(),
        ))
    }

    /// Get model information
    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            provider: "unknown".to_string(),
            model_name: "unknown".to_string(),
        }
    }
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub provider: String,
    pub model_name: String,
}

/// Stub LLM provider for MVP (returns error).
///
/// This is provided for the MVP to allow consolidation engine to compile.
/// Users must bring their own LLM implementation for actual consolidation.
pub struct StubLLMProvider;

#[async_trait]
impl LLMProvider for StubLLMProvider {
    async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> Result<String> {
        Err(crate::error::ThymosError::Configuration(
            "LLM provider not configured. Implement the LLMProvider trait for your LLM".to_string(),
        ))
    }

    async fn generate_request(&self, _request: &LLMRequest) -> Result<LLMResponse> {
        Err(crate::error::ThymosError::Configuration(
            "LLM provider not configured. Implement the LLMProvider trait for your LLM".to_string(),
        ))
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            provider: "stub".to_string(),
            model_name: "none".to_string(),
        }
    }
}

pub mod circuit_breaker;
pub mod factory;
pub mod providers;
pub mod retry;

pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState,
};
pub use retry::{with_retry, RetryConfig, RetryState};

pub use factory::LLMProviderFactory;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config() {
        let config = LLMConfig::new()
            .with_temperature(1.5)
            .with_max_tokens(1000)
            .with_system_prompt("You are helpful");

        assert_eq!(config.temperature, 1.5);
        assert_eq!(config.max_tokens, 1000);
        assert!(config.system_prompt.is_some());
    }

    #[test]
    fn test_temperature_clamping() {
        let config = LLMConfig::new().with_temperature(5.0);
        assert_eq!(config.temperature, 2.0);

        let config = LLMConfig::new().with_temperature(-1.0);
        assert_eq!(config.temperature, 0.0);
    }

    #[tokio::test]
    async fn test_stub_provider() {
        let provider = StubLLMProvider;
        let result = provider.generate("test", &LLMConfig::default()).await;
        assert!(result.is_err());
    }
}
