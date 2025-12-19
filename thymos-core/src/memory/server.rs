//! Server-based memory backend for connecting to Locai server via HTTP
//!
//! This backend connects to a running Locai server and provides:
//! - Full semantic search with embeddings
//! - SurrealDB-backed persistence
//! - Multi-agent shared memory support
//!
//! For WASM targets, use the wasi:http based implementation in thymos-wasm.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::backend::{MemoryBackend, MemoryRecord, QueryOptions, StoreOptions};
use crate::error::{Result, ThymosError};

/// Configuration for server mode memory connections.
#[derive(Debug, Clone)]
pub struct ServerMemoryConfig {
    /// Base URL of the Locai server (e.g., "http://localhost:3000")
    pub base_url: String,

    /// Optional API key for authentication
    pub api_key: Option<String>,

    /// Connection timeout (default: 30 seconds)
    pub timeout: Duration,

    /// Maximum number of retries for failed requests (default: 3)
    pub max_retries: u32,

    /// Initial retry backoff duration (default: 100ms)
    pub initial_backoff: Duration,
}

impl Default for ServerMemoryConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            api_key: None,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
        }
    }
}

impl ServerMemoryConfig {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            ..Default::default()
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }
}

/// Server-based memory backend for connecting to Locai server.
pub struct ServerMemoryBackend {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    #[allow(dead_code)]
    max_retries: u32,
    #[allow(dead_code)]
    initial_backoff: Duration,
}

impl ServerMemoryBackend {
    /// Create a new server memory backend with the given configuration.
    pub async fn new(config: ServerMemoryConfig) -> Result<Self> {
        let client = Client::builder().timeout(config.timeout).build().map_err(|e| {
            ThymosError::Configuration(format!("Failed to create HTTP client: {}", e))
        })?;

        let backend = Self {
            client,
            base_url: config.base_url,
            api_key: config.api_key,
            max_retries: config.max_retries,
            initial_backoff: config.initial_backoff,
        };

        // Verify connection with health check
        backend.health_check().await?;

        Ok(backend)
    }

    /// Create without health check (for lazy initialization)
    pub fn new_lazy(config: ServerMemoryConfig) -> Result<Self> {
        let client = Client::builder().timeout(config.timeout).build().map_err(|e| {
            ThymosError::Configuration(format!("Failed to create HTTP client: {}", e))
        })?;

        Ok(Self {
            client,
            base_url: config.base_url,
            api_key: config.api_key,
            max_retries: config.max_retries,
            initial_backoff: config.initial_backoff,
        })
    }

    /// Add authorization header if API key is configured
    fn add_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(api_key) = &self.api_key {
            request.header("Authorization", format!("Bearer {}", api_key))
        } else {
            request
        }
    }

    /// Parse a Locai memory response into a MemoryRecord
    fn parse_memory(value: &serde_json::Value) -> Option<MemoryRecord> {
        Some(MemoryRecord {
            id: value.get("id")?.as_str()?.to_string(),
            content: value.get("content")?.as_str()?.to_string(),
            created_at: value
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("1970-01-01T00:00:00Z")
                .to_string(),
            last_accessed: value
                .get("last_accessed")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            properties: value.get("properties").cloned().unwrap_or(serde_json::json!({})),
            score: value.get("score").and_then(|v| v.as_f64()),
        })
    }
}

#[async_trait]
impl MemoryBackend for ServerMemoryBackend {
    async fn store(&self, content: String, options: Option<StoreOptions>) -> Result<String> {
        let url = format!("{}/api/memories", self.base_url);

        let mut json_body = serde_json::json!({
            "content": content,
        });

        if let Some(opts) = options {
            if let Some(memory_type) = opts.memory_type {
                json_body["memory_type"] = serde_json::json!(memory_type);
            }
            if !opts.tags.is_empty() {
                json_body["tags"] = serde_json::json!(opts.tags);
            }
            if let Some(priority) = opts.priority {
                json_body["priority"] = serde_json::json!(priority);
            }
            if let Some(embedding) = opts.embedding {
                json_body["embedding"] = serde_json::Value::Array(
                    embedding
                        .into_iter()
                        .map(|v| {
                            serde_json::Value::Number(
                                serde_json::Number::from_f64(v as f64)
                                    .unwrap_or_else(|| serde_json::Number::from(0)),
                            )
                        })
                        .collect(),
                );
            }
        }

        let request = self.add_auth(self.client.post(&url).json(&json_body));

        let response = request
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to store memory: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Memory(format!(
                "Store memory failed with status {}: {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ThymosError::Other(format!("Failed to parse JSON: {}", e)))?;

        json.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ThymosError::Memory("No ID in response".to_string()))
    }

    async fn search(
        &self,
        query: &str,
        options: Option<QueryOptions>,
    ) -> Result<Vec<MemoryRecord>> {
        let url = format!("{}/api/memories/search", self.base_url);
        let opts = options.unwrap_or_default();
        let limit_str = opts.limit.unwrap_or(10).to_string();

        let mut query_params = vec![("q", query.to_string()), ("limit", limit_str)];

        if let Some(weight) = opts.semantic_weight {
            query_params.push(("semantic_weight", weight.to_string()));
        }

        if let Some(strategy) = opts.strategy {
            query_params.push(("strategy", strategy));
        }

        let request = self.add_auth(self.client.get(&url).query(&query_params));

        let response = request
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to search memories: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Memory(format!(
                "Search failed with status {}: {}",
                status, error_text
            )));
        }

        let search_results: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| ThymosError::Other(format!("Failed to parse response: {}", e)))?;

        // Locai server returns: [{"memory": {...}, "score": ...}, ...]
        let memories: Vec<MemoryRecord> = search_results
            .into_iter()
            .filter_map(|result| {
                let memory = result.get("memory")?;
                let score = result.get("score").and_then(|v| v.as_f64());
                let mut record = Self::parse_memory(memory)?;
                record.score = score;
                Some(record)
            })
            .collect();

        Ok(memories)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryRecord>> {
        let url = format!("{}/api/memories/{}", self.base_url, urlencoding::encode(id));

        let request = self.add_auth(self.client.get(&url));

        let response = request
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to get memory: {}", e)))?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Memory(format!(
                "Get memory failed with status {}: {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ThymosError::Other(format!("Failed to parse JSON: {}", e)))?;

        Ok(Self::parse_memory(&json))
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let url = format!("{}/api/memories/{}", self.base_url, urlencoding::encode(id));

        let request = self.add_auth(self.client.delete(&url));

        let response = request
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to delete memory: {}", e)))?;

        if response.status().as_u16() == 404 {
            return Ok(false);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Memory(format!(
                "Delete memory failed with status {}: {}",
                status, error_text
            )));
        }

        Ok(true)
    }

    async fn count(&self) -> Result<u64> {
        let url = format!("{}/api/memories/count", self.base_url);

        let request = self.add_auth(self.client.get(&url));

        let response = request
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Failed to count memories: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ThymosError::Memory(format!(
                "Count failed with status {}: {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ThymosError::Other(format!("Failed to parse JSON: {}", e)))?;

        json.get("count")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ThymosError::Memory("No count in response".to_string()))
    }

    async fn health_check(&self) -> Result<()> {
        let url = format!("{}/api/health", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ThymosError::Memory(format!("Health check failed: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ThymosError::Memory(format!(
                "Server health check failed with status: {}",
                response.status()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config() {
        let config = ServerMemoryConfig::new("http://localhost:3000")
            .with_api_key("test_key")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.base_url, "http://localhost:3000");
        assert_eq!(config.api_key, Some("test_key".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_default_config() {
        let config = ServerMemoryConfig::default();
        assert_eq!(config.base_url, "http://localhost:3000");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_health_check_unreachable() {
        let config = ServerMemoryConfig::new("http://127.0.0.1:1");
        let result = ServerMemoryBackend::new(config).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_memory() {
        let json = serde_json::json!({
            "id": "mem_1",
            "content": "Test content",
            "created_at": "2024-01-01T00:00:00Z",
            "properties": {"type": "fact"}
        });

        let record = ServerMemoryBackend::parse_memory(&json).unwrap();
        assert_eq!(record.id, "mem_1");
        assert_eq!(record.content, "Test content");
        assert_eq!(record.properties["type"], "fact");
    }
}
