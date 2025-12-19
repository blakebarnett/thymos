//! Ollama LLM provider implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ThymosError};
use crate::llm::{
    LLMConfig, LLMProvider, LLMRequest, LLMResponse, Message, MessageRole, ModelInfo, TokenUsage,
};

/// Ollama LLM provider (local, free, runs on your machine).
pub struct OllamaProvider {
    client: reqwest::Client,
    model: String,
    base_url: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (e.g., "qwen3:14b")
    /// * `base_url` - Base URL for Ollama API (defaults to "http://localhost:11434")
    pub fn new(model: impl Into<String>, base_url: Option<impl Into<String>>) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.into(),
            base_url: base_url
                .map(|u| u.into())
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
        }
    }

    /// Create from environment variables.
    ///
    /// Reads from:
    /// - `OLLAMA_MODEL` - Model name (optional, defaults to "qwen3:14b")
    /// - `OLLAMA_BASE_URL` - Base URL (optional, defaults to "http://localhost:11434")
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (overrides OLLAMA_MODEL if provided)
    ///
    /// # Errors
    ///
    /// Returns an error if Ollama is not accessible.
    pub fn from_env(model: Option<impl Into<String>>) -> Result<Self> {
        let model = model
            .map(|m| m.into())
            .or_else(|| std::env::var("OLLAMA_MODEL").ok())
            .unwrap_or_else(|| "qwen3:14b".to_string());

        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        Ok(Self::new(model, Some(base_url)))
    }

    /// Create with default settings (qwen3:14b, localhost:11434).
    pub fn with_defaults() -> Result<Self> {
        Self::from_env(None::<String>)
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: Option<f32>,
    num_predict: Option<usize>,
    stop: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaMessageResponse,
    #[serde(default)]
    #[allow(dead_code)]
    done: bool,
    #[serde(default)]
    #[allow(dead_code)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

#[derive(Deserialize)]
struct OllamaMessageResponse {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    thinking: Option<String>,
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn generate(&self, prompt: &str, config: &LLMConfig) -> Result<String> {
        let mut messages = Vec::new();
        
        if let Some(ref system_prompt) = config.system_prompt {
            messages.push(Message {
                role: MessageRole::System,
                content: system_prompt.clone(),
            });
        }
        
        messages.push(Message {
            role: MessageRole::User,
            content: prompt.to_string(),
        });

        let request = LLMRequest {
            messages,
            temperature: Some(config.temperature),
            max_tokens: Some(config.max_tokens),
            stop_sequences: Vec::new(),
        };

        let response = self.generate_request(&request).await?;
        Ok(response.content)
    }

    async fn generate_request(&self, request: &LLMRequest) -> Result<LLMResponse> {
        let ollama_messages: Vec<OllamaMessage> = request
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        let options = OllamaOptions {
            temperature: request.temperature,
            num_predict: request.max_tokens,
            stop: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
        };

        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            options: Some(options),
        };

        let url = format!("{}/api/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!(
                    "Failed to send request to Ollama: {}. Make sure Ollama is running.",
                    e
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Configuration(format!(
                "Ollama API error ({}): {}",
                status, text
            )));
        }

        let response_text = response.text().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to read Ollama response: {}", e))
        })?;

        let ollama_response: OllamaResponse = serde_json::from_str(&response_text).map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse Ollama response: {}", e))
        })?;

        let mut content = ollama_response.message.content.trim().to_string();

        if content.is_empty() && ollama_response.message.thinking.is_some() {
            content = ollama_response
                .message
                .thinking
                .unwrap()
                .trim()
                .to_string();
        }
        let usage = if let (Some(prompt_tokens), Some(completion_tokens)) = (
            ollama_response.prompt_eval_count,
            ollama_response.eval_count,
        ) {
            Some(TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            })
        } else {
            None
        };

        Ok(LLMResponse { content, usage })
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            provider: "ollama".to_string(),
            model_name: self.model.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::new("qwen3:14b", None::<String>);
        assert_eq!(provider.model(), "qwen3:14b");
    }

    #[test]
    fn test_ollama_from_env_default() {
        unsafe {
            std::env::remove_var("OLLAMA_MODEL");
            std::env::remove_var("OLLAMA_BASE_URL");
        }

        let provider = OllamaProvider::from_env(None::<String>).unwrap();
        assert_eq!(provider.model(), "qwen3:14b");
    }

    #[test]
    fn test_ollama_from_env_with_model() {
        unsafe {
            std::env::set_var("OLLAMA_MODEL", "test-model");
        }

        let provider = OllamaProvider::from_env(None::<String>).unwrap();
        assert_eq!(provider.model(), "test-model");

        unsafe {
            std::env::remove_var("OLLAMA_MODEL");
        }
    }
}

