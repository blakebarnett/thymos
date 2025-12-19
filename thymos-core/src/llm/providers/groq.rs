//! Groq LLM provider implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ThymosError};
use crate::llm::{
    LLMConfig, LLMProvider, LLMRequest, LLMResponse, Message, MessageRole, ModelInfo, TokenUsage,
};

/// Groq LLM provider (fast, cost-effective, recommended for most use cases).
pub struct GroqProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl GroqProvider {
    /// Create a new Groq provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Groq API key
    /// * `model` - Model name (e.g., "llama-3.3-70b-versatile")
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
        }
    }

    /// Create from environment variables.
    ///
    /// Reads from:
    /// - `GROQ_API_KEY` - API key (required)
    /// - `GROQ_MODEL` - Model name (optional, defaults to "llama-3.3-70b-versatile")
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (overrides GROQ_MODEL if provided)
    ///
    /// # Errors
    ///
    /// Returns an error if GROQ_API_KEY is not set.
    pub fn from_env(model: Option<impl Into<String>>) -> Result<Self> {
        let api_key = std::env::var("GROQ_API_KEY").map_err(|_| {
            ThymosError::Configuration("GROQ_API_KEY environment variable not set".to_string())
        })?;
        
        // Use provided model, or GROQ_MODEL env var, or default
        let model = model
            .map(|m| m.into())
            .or_else(|| std::env::var("GROQ_MODEL").ok())
            .unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());
        
        Ok(Self::new(api_key, model))
    }
    
    /// Create from environment variables only (no model parameter).
    ///
    /// Reads from:
    /// - `GROQ_API_KEY` - API key (required)
    /// - `GROQ_MODEL` - Model name (optional, defaults to "llama-3.3-70b-versatile")
    ///
    /// # Errors
    ///
    /// Returns an error if GROQ_API_KEY is not set.
    pub fn from_env_default() -> Result<Self> {
        Self::from_env(None::<String>)
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Serialize)]
struct GroqRequest {
    model: String,
    messages: Vec<GroqMessage>,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
    stop: Option<Vec<String>>,
}

#[derive(Serialize)]
struct GroqMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
    usage: Option<GroqUsage>,
}

#[derive(Deserialize)]
struct GroqChoice {
    message: GroqMessageResponse,
}

#[derive(Deserialize)]
struct GroqMessageResponse {
    content: String,
}

#[derive(Deserialize)]
struct GroqUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[async_trait]
impl LLMProvider for GroqProvider {
    async fn generate(&self, prompt: &str, config: &LLMConfig) -> Result<String> {
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

    async fn generate_request(&self, request: &LLMRequest) -> Result<LLMResponse> {
        let groq_messages: Vec<GroqMessage> = request
            .messages
            .iter()
            .map(|m| GroqMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        let groq_request = GroqRequest {
            model: self.model.clone(),
            messages: groq_messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stop: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&groq_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to send request to Groq: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Configuration(format!(
                "Groq API error ({}): {}",
                status, text
            )));
        }

        let groq_response: GroqResponse = response.json().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse Groq response: {}", e))
        })?;

        if groq_response.choices.is_empty() {
            return Err(ThymosError::Configuration(
                "Groq API returned no choices".to_string(),
            ));
        }

        let content = groq_response.choices[0].message.content.clone();
        let usage = groq_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(LLMResponse { content, usage })
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            provider: "groq".to_string(),
            model_name: self.model.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_provider_creation() {
        let provider = GroqProvider::new("test-key", "llama-3.3-70b-versatile");
        assert_eq!(provider.model(), "llama-3.3-70b-versatile");
    }

    #[test]
    fn test_groq_from_env_missing() {
        // Clear env var if set
        unsafe {
            std::env::remove_var("GROQ_API_KEY");
            std::env::remove_var("GROQ_MODEL");
        }
        let result = GroqProvider::from_env(Some("test-model"));
        assert!(result.is_err());
    }
    
    #[test]
    fn test_groq_from_env_with_model_env_var() {
        unsafe {
            std::env::set_var("GROQ_API_KEY", "test-key");
            std::env::set_var("GROQ_MODEL", "custom-model");
        }
        
        let provider = GroqProvider::from_env(None::<String>).unwrap();
        assert_eq!(provider.model(), "custom-model");
        
        // Cleanup
        unsafe {
            std::env::remove_var("GROQ_API_KEY");
            std::env::remove_var("GROQ_MODEL");
        }
    }
    
    #[test]
    fn test_groq_from_env_model_override() {
        unsafe {
            std::env::set_var("GROQ_API_KEY", "test-key");
            std::env::set_var("GROQ_MODEL", "env-model");
        }
        
        // Provided model should override env var
        let provider = GroqProvider::from_env(Some("override-model")).unwrap();
        assert_eq!(provider.model(), "override-model");
        
        // Cleanup
        unsafe {
            std::env::remove_var("GROQ_API_KEY");
            std::env::remove_var("GROQ_MODEL");
        }
    }
    
    #[test]
    fn test_groq_from_env_default() {
        unsafe {
            std::env::set_var("GROQ_API_KEY", "test-key");
            std::env::remove_var("GROQ_MODEL");
        }
        
        // Should use default model when GROQ_MODEL not set
        let provider = GroqProvider::from_env_default().unwrap();
        assert_eq!(provider.model(), "llama-3.3-70b-versatile");
        
        // Cleanup
        unsafe {
            std::env::remove_var("GROQ_API_KEY");
        }
    }
}
