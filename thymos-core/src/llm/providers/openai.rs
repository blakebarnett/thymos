//! OpenAI LLM provider implementation

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_stream::StreamExt;

use crate::error::{Result, ThymosError};
use crate::llm::{
    LLMConfig, LLMProvider, LLMRequest, LLMResponse, Message, MessageRole, ModelInfo, TokenUsage,
};

/// OpenAI LLM provider (GPT-4, GPT-4 Turbo, etc.).
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `model` - Model name (e.g., "gpt-4o", "gpt-4-turbo")
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    /// Create with a custom base URL (for Azure OpenAI or compatible APIs).
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `model` - Model name
    /// * `base_url` - Custom API base URL
    pub fn with_base_url(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
        }
    }

    /// Create from environment variables.
    ///
    /// Reads from:
    /// - `OPENAI_API_KEY` - API key (required)
    /// - `OPENAI_MODEL` - Model name (optional, defaults to "gpt-4o")
    /// - `OPENAI_BASE_URL` - Custom base URL (optional)
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (overrides OPENAI_MODEL if provided)
    ///
    /// # Errors
    ///
    /// Returns an error if OPENAI_API_KEY is not set.
    pub fn from_env(model: Option<impl Into<String>>) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            ThymosError::Configuration("OPENAI_API_KEY environment variable not set".to_string())
        })?;

        let model = model
            .map(|m| m.into())
            .or_else(|| std::env::var("OPENAI_MODEL").ok())
            .unwrap_or_else(|| "gpt-4o".to_string());

        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        })
    }

    /// Create from environment variables only (no model parameter).
    ///
    /// Reads from:
    /// - `OPENAI_API_KEY` - API key (required)
    /// - `OPENAI_MODEL` - Model name (optional, defaults to "gpt-4o")
    ///
    /// # Errors
    ///
    /// Returns an error if OPENAI_API_KEY is not set.
    pub fn from_env_default() -> Result<Self> {
        Self::from_env(None::<String>)
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Option<OpenAIMessageResponse>,
    delta: Option<OpenAIDelta>,
}

#[derive(Deserialize)]
struct OpenAIMessageResponse {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIError {
    error: OpenAIErrorDetail,
}

#[derive(Deserialize)]
struct OpenAIErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

fn convert_messages(messages: &[Message]) -> Vec<OpenAIMessage> {
    messages
        .iter()
        .map(|m| OpenAIMessage {
            role: match m.role {
                MessageRole::System => "system".to_string(),
                MessageRole::User => "user".to_string(),
                MessageRole::Assistant => "assistant".to_string(),
            },
            content: m.content.clone(),
        })
        .collect()
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
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
        let openai_request = OpenAIRequest {
            model: self.model.clone(),
            messages: convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stop: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to send request to OpenAI: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse as OpenAI error format
            if let Ok(error) = serde_json::from_str::<OpenAIError>(&text) {
                return Err(ThymosError::Configuration(format!(
                    "OpenAI API error ({}): {}",
                    error.error.error_type.unwrap_or_else(|| status.to_string()),
                    error.error.message
                )));
            }

            return Err(ThymosError::Configuration(format!(
                "OpenAI API error ({}): {}",
                status, text
            )));
        }

        let openai_response: OpenAIResponse = response.json().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse OpenAI response: {}", e))
        })?;

        if openai_response.choices.is_empty() {
            return Err(ThymosError::Configuration(
                "OpenAI API returned no choices".to_string(),
            ));
        }

        let content = openai_response.choices[0]
            .message
            .as_ref()
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

        let usage = openai_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(LLMResponse { content, usage })
    }

    async fn generate_stream(
        &self,
        request: &LLMRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let openai_request = OpenAIRequest {
            model: self.model.clone(),
            messages: convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stop: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
            stream: true,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to send request to OpenAI: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(ThymosError::Configuration(format!(
                "OpenAI API error ({}): {}",
                status, text
            )));
        }

        // Convert response bytes to a stream of lines
        let bytes_stream = response.bytes_stream();
        let reader = tokio_util::io::StreamReader::new(
            bytes_stream.map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))),
        );
        let lines = tokio::io::BufReader::new(reader).lines();
        let lines_stream = LinesStream::new(lines);

        // Process SSE lines
        let stream = lines_stream.filter_map(|line_result| {
            match line_result {
                Ok(line) => {
                    // SSE format: "data: {...}"
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return None;
                        }

                        match serde_json::from_str::<OpenAIStreamChunk>(data) {
                            Ok(chunk) => {
                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(delta) = &choice.delta {
                                        if let Some(content) = &delta.content {
                                            return Some(Ok(content.clone()));
                                        }
                                    }
                                }
                                None
                            }
                            Err(e) => Some(Err(ThymosError::Configuration(format!(
                                "Failed to parse stream chunk: {}",
                                e
                            )))),
                        }
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(ThymosError::Configuration(format!(
                    "Stream read error: {}",
                    e
                )))),
            }
        });

        Ok(Box::pin(stream))
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            provider: "openai".to_string(),
            model_name: self.model.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new("test-key", "gpt-4o");
        assert_eq!(provider.model(), "gpt-4o");
        assert_eq!(provider.base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn test_openai_provider_custom_base_url() {
        let provider =
            OpenAIProvider::with_base_url("test-key", "gpt-4", "https://custom.openai.azure.com");
        assert_eq!(provider.model(), "gpt-4");
        assert_eq!(provider.base_url(), "https://custom.openai.azure.com");
    }

    #[test]
    fn test_openai_from_env_missing() {
        // Clear env var if set
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENAI_MODEL");
        }
        let result = OpenAIProvider::from_env(Some("test-model"));
        assert!(result.is_err());
    }

    #[test]
    fn test_openai_from_env_with_key() {
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "test-key");
            std::env::remove_var("OPENAI_MODEL");
        }

        let provider = OpenAIProvider::from_env_default().unwrap();
        assert_eq!(provider.model(), "gpt-4o"); // Default model

        // Cleanup
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn test_openai_from_env_with_model_env_var() {
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "test-key");
            std::env::set_var("OPENAI_MODEL", "gpt-4-turbo");
        }

        let provider = OpenAIProvider::from_env(None::<String>).unwrap();
        assert_eq!(provider.model(), "gpt-4-turbo");

        // Cleanup
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENAI_MODEL");
        }
    }

    #[test]
    fn test_openai_from_env_model_override() {
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "test-key");
            std::env::set_var("OPENAI_MODEL", "env-model");
        }

        // Provided model should override env var
        let provider = OpenAIProvider::from_env(Some("override-model")).unwrap();
        assert_eq!(provider.model(), "override-model");

        // Cleanup
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENAI_MODEL");
        }
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are helpful".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Hi there!".to_string(),
            },
        ];

        let converted = convert_messages(&messages);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, "system");
        assert_eq!(converted[1].role, "user");
        assert_eq!(converted[2].role, "assistant");
    }

    #[test]
    fn test_model_info() {
        let provider = OpenAIProvider::new("test-key", "gpt-4o-mini");
        let info = provider.model_info();
        assert_eq!(info.provider, "openai");
        assert_eq!(info.model_name, "gpt-4o-mini");
    }
}
