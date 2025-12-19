//! Anthropic (Claude) LLM provider implementation

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

/// Anthropic API version header value
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic (Claude) LLM provider.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    /// * `model` - Model name (e.g., "claude-3-5-sonnet-20241022")
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://api.anthropic.com/v1".to_string(),
        }
    }

    /// Create with a custom base URL.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
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
    /// - `ANTHROPIC_API_KEY` - API key (required)
    /// - `ANTHROPIC_MODEL` - Model name (optional, defaults to "claude-3-5-sonnet-20241022")
    /// - `ANTHROPIC_BASE_URL` - Custom base URL (optional)
    ///
    /// # Arguments
    ///
    /// * `model` - Model name (overrides ANTHROPIC_MODEL if provided)
    ///
    /// # Errors
    ///
    /// Returns an error if ANTHROPIC_API_KEY is not set.
    pub fn from_env(model: Option<impl Into<String>>) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            ThymosError::Configuration("ANTHROPIC_API_KEY environment variable not set".to_string())
        })?;

        let model = model
            .map(|m| m.into())
            .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        })
    }

    /// Create from environment variables only (no model parameter).
    ///
    /// # Errors
    ///
    /// Returns an error if ANTHROPIC_API_KEY is not set.
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

/// Anthropic API request format
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic API response format
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    usage: Option<AnthropicUsage>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: usize,
    output_tokens: usize,
}

/// Anthropic streaming event
#[derive(Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

/// Anthropic error response
#[derive(Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

#[derive(Deserialize)]
struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Convert messages to Anthropic format, extracting system prompt
fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_prompt = None;
    let mut anthropic_messages = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                // Anthropic uses a separate system field
                system_prompt = Some(msg.content.clone());
            }
            MessageRole::User => {
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: msg.content.clone(),
                });
            }
            MessageRole::Assistant => {
                anthropic_messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: msg.content.clone(),
                });
            }
        }
    }

    (system_prompt, anthropic_messages)
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
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
        let (system, messages) = convert_messages(&request.messages);

        let anthropic_request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            system,
            max_tokens: request.max_tokens.unwrap_or(1024),
            temperature: request.temperature,
            stop_sequences: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
            stream: false,
        };

        let url = format!("{}/messages", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to send request to Anthropic: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse as Anthropic error format
            if let Ok(error) = serde_json::from_str::<AnthropicError>(&text) {
                return Err(ThymosError::Configuration(format!(
                    "Anthropic API error ({}): {}",
                    error.error.error_type, error.error.message
                )));
            }

            return Err(ThymosError::Configuration(format!(
                "Anthropic API error ({}): {}",
                status, text
            )));
        }

        let anthropic_response: AnthropicResponse = response.json().await.map_err(|e| {
            ThymosError::Configuration(format!("Failed to parse Anthropic response: {}", e))
        })?;

        // Extract text from content blocks
        let content = anthropic_response
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .filter_map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("");

        let usage = anthropic_response.usage.map(|u| TokenUsage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        });

        Ok(LLMResponse { content, usage })
    }

    async fn generate_stream(
        &self,
        request: &LLMRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let (system, messages) = convert_messages(&request.messages);

        let anthropic_request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            system,
            max_tokens: request.max_tokens.unwrap_or(1024),
            temperature: request.temperature,
            stop_sequences: if request.stop_sequences.is_empty() {
                None
            } else {
                Some(request.stop_sequences.clone())
            },
            stream: true,
        };

        let url = format!("{}/messages", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| {
                ThymosError::Configuration(format!("Failed to send request to Anthropic: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(ThymosError::Configuration(format!(
                "Anthropic API error ({}): {}",
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
                    // SSE format: "data: {...}" or "event: ..."
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return None;
                        }

                        match serde_json::from_str::<AnthropicStreamEvent>(data) {
                            Ok(event) => {
                                // Look for content_block_delta events with text
                                if event.event_type == "content_block_delta" {
                                    if let Some(delta) = event.delta {
                                        if delta.delta_type.as_deref() == Some("text_delta") {
                                            if let Some(text) = delta.text {
                                                return Some(Ok(text));
                                            }
                                        }
                                    }
                                }
                                None
                            }
                            Err(_) => None, // Skip unparseable events
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
            provider: "anthropic".to_string(),
            model_name: self.model.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::new("test-key", "claude-3-5-sonnet-20241022");
        assert_eq!(provider.model(), "claude-3-5-sonnet-20241022");
        assert_eq!(provider.base_url(), "https://api.anthropic.com/v1");
    }

    #[test]
    fn test_anthropic_provider_custom_base_url() {
        let provider = AnthropicProvider::with_base_url(
            "test-key",
            "claude-3-opus-20240229",
            "https://custom.anthropic.com",
        );
        assert_eq!(provider.model(), "claude-3-opus-20240229");
        assert_eq!(provider.base_url(), "https://custom.anthropic.com");
    }

    #[test]
    fn test_anthropic_from_env_missing() {
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("ANTHROPIC_MODEL");
        }
        let result = AnthropicProvider::from_env(Some("test-model"));
        assert!(result.is_err());
    }

    #[test]
    fn test_anthropic_from_env_with_key() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "test-key");
            std::env::remove_var("ANTHROPIC_MODEL");
        }

        let provider = AnthropicProvider::from_env_default().unwrap();
        assert_eq!(provider.model(), "claude-3-5-sonnet-20241022"); // Default model

        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn test_anthropic_from_env_with_model_env_var() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "test-key");
            std::env::set_var("ANTHROPIC_MODEL", "claude-3-opus-20240229");
        }

        let provider = AnthropicProvider::from_env(None::<String>).unwrap();
        assert_eq!(provider.model(), "claude-3-opus-20240229");

        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("ANTHROPIC_MODEL");
        }
    }

    #[test]
    fn test_anthropic_from_env_model_override() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "test-key");
            std::env::set_var("ANTHROPIC_MODEL", "env-model");
        }

        let provider = AnthropicProvider::from_env(Some("override-model")).unwrap();
        assert_eq!(provider.model(), "override-model");

        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("ANTHROPIC_MODEL");
        }
    }

    #[test]
    fn test_convert_messages_with_system() {
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

        let (system, converted) = convert_messages(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    #[test]
    fn test_convert_messages_without_system() {
        let messages = vec![Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }];

        let (system, converted) = convert_messages(&messages);

        assert!(system.is_none());
        assert_eq!(converted.len(), 1);
    }

    #[test]
    fn test_model_info() {
        let provider = AnthropicProvider::new("test-key", "claude-3-haiku-20240307");
        let info = provider.model_info();
        assert_eq!(info.provider, "anthropic");
        assert_eq!(info.model_name, "claude-3-haiku-20240307");
    }
}
