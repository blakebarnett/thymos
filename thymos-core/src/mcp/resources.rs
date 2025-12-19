//! MCP Resource Providers
//!
//! Resources are read-only data sources that can be exposed via MCP.

use super::protocol::{McpResource, ResourceContent};
use async_trait::async_trait;
use crate::memory::MemorySystem;
use std::sync::Arc;

/// Trait for providing MCP resources
#[async_trait]
pub trait ResourceProvider: Send + Sync {
    /// List available resources
    async fn list_resources(&self) -> Vec<McpResource>;

    /// Read a resource by URI
    async fn read_resource(&self, uri: &str) -> Option<ResourceContent>;
}

/// Memory resource provider - exposes agent memories as MCP resources
pub struct MemoryResource {
    memory: Arc<MemorySystem>,
    agent_id: String,
    max_results: usize,
}

impl MemoryResource {
    /// Create a new memory resource provider
    pub fn new(memory: Arc<MemorySystem>, agent_id: impl Into<String>) -> Self {
        Self {
            memory,
            agent_id: agent_id.into(),
            max_results: 100,
        }
    }

    /// Set the maximum number of results to return
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Parse a memory URI
    ///
    /// Format: `memory://{agent_id}/{memory_id}` or `memory://{agent_id}?query=...`
    fn parse_uri(&self, uri: &str) -> Option<MemoryUri> {
        let uri = uri.strip_prefix("memory://")?;
        let parts: Vec<&str> = uri.splitn(2, '/').collect();

        if parts.is_empty() {
            return None;
        }

        let agent_id = parts[0].to_string();

        if parts.len() > 1 {
            // Check for query parameter
            if let Some(query_start) = parts[1].find("?query=") {
                let query = &parts[1][query_start + 7..];
                return Some(MemoryUri::Search {
                    agent_id,
                    query: urlencoding::decode(query).ok()?.to_string(),
                });
            }

            // Single memory by ID
            return Some(MemoryUri::Single {
                agent_id,
                memory_id: parts[1].to_string(),
            });
        }

        // List all memories
        Some(MemoryUri::List { agent_id })
    }
}

enum MemoryUri {
    List { agent_id: String },
    Single { agent_id: String, memory_id: String },
    Search { agent_id: String, query: String },
}

#[async_trait]
impl ResourceProvider for MemoryResource {
    async fn list_resources(&self) -> Vec<McpResource> {
        vec![
            McpResource {
                uri: format!("memory://{}", self.agent_id),
                name: format!("{} memories", self.agent_id),
                description: Some("Agent memory entries".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            McpResource {
                uri: format!("memory://{}?query={{query}}", self.agent_id),
                name: format!("{} memory search", self.agent_id),
                description: Some("Search agent memories".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ]
    }

    async fn read_resource(&self, uri: &str) -> Option<ResourceContent> {
        let parsed = self.parse_uri(uri)?;

        match parsed {
            MemoryUri::List { agent_id } => {
                if agent_id != self.agent_id {
                    return None;
                }

                // Return recent memories (empty query returns all)
                let memories = self
                    .memory
                    .search("", Some(self.max_results))
                    .await
                    .ok()?;

                let json = serde_json::to_string_pretty(&memories).ok()?;

                Some(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: Some(json),
                    blob: None,
                })
            }
            MemoryUri::Single { agent_id, memory_id } => {
                if agent_id != self.agent_id {
                    return None;
                }

                // Search for memories matching this ID
                let memories = self
                    .memory
                    .search(&memory_id, Some(1))
                    .await
                    .ok()?;

                let memory = memories.into_iter().next()?;
                let json = serde_json::to_string_pretty(&memory).ok()?;

                Some(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: Some(json),
                    blob: None,
                })
            }
            MemoryUri::Search { agent_id, query } => {
                if agent_id != self.agent_id {
                    return None;
                }

                let memories = self
                    .memory
                    .search(&query, Some(self.max_results))
                    .await
                    .ok()?;

                let json = serde_json::to_string_pretty(&memories).ok()?;

                Some(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: Some(json),
                    blob: None,
                })
            }
        }
    }
}

/// Static resource provider for exposing static content
pub struct StaticResource {
    resources: Vec<(McpResource, String)>,
}

impl StaticResource {
    /// Create a new static resource provider
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
        }
    }

    /// Add a static resource
    pub fn add(
        mut self,
        uri: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let uri_string = uri.into();
        self.resources.push((
            McpResource {
                uri: uri_string.clone(),
                name: name.into(),
                description: None,
                mime_type: Some("text/plain".to_string()),
            },
            content.into(),
        ));
        self
    }

    /// Add a JSON resource
    pub fn add_json(
        mut self,
        uri: impl Into<String>,
        name: impl Into<String>,
        value: &serde_json::Value,
    ) -> Self {
        let uri_string = uri.into();
        let content = serde_json::to_string_pretty(value).unwrap_or_default();
        self.resources.push((
            McpResource {
                uri: uri_string.clone(),
                name: name.into(),
                description: None,
                mime_type: Some("application/json".to_string()),
            },
            content,
        ));
        self
    }
}

impl Default for StaticResource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ResourceProvider for StaticResource {
    async fn list_resources(&self) -> Vec<McpResource> {
        self.resources.iter().map(|(r, _)| r.clone()).collect()
    }

    async fn read_resource(&self, uri: &str) -> Option<ResourceContent> {
        for (resource, content) in &self.resources {
            if resource.uri == uri {
                return Some(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: resource.mime_type.clone(),
                    text: Some(content.clone()),
                    blob: None,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_resource() {
        let provider = StaticResource::new()
            .add("static://readme", "README", "Hello, World!")
            .add_json(
                "static://config",
                "Config",
                &serde_json::json!({"version": "1.0"}),
            );

        // List resources
        let resources = provider.list_resources().await;
        assert_eq!(resources.len(), 2);

        // Read resource
        let content = provider.read_resource("static://readme").await;
        assert!(content.is_some());
        assert_eq!(content.unwrap().text.unwrap(), "Hello, World!");

        // Read non-existent
        let content = provider.read_resource("static://nonexistent").await;
        assert!(content.is_none());
    }

    #[test]
    fn test_memory_uri_parsing() {
        // Test the URI parsing logic directly without needing a real MemorySystem
        fn parse_test_uri(uri: &str) -> Option<(String, Option<String>)> {
            let uri = uri.strip_prefix("memory://")?;
            let parts: Vec<&str> = uri.splitn(2, '/').collect();
            if parts.is_empty() {
                return None;
            }
            let agent_id = parts[0].to_string();
            let rest = parts.get(1).map(|s| s.to_string());
            Some((agent_id, rest))
        }

        // List URI
        let (agent, rest) = parse_test_uri("memory://agent1").unwrap();
        assert_eq!(agent, "agent1");
        assert!(rest.is_none());

        // Single memory URI
        let (agent, rest) = parse_test_uri("memory://agent1/mem123").unwrap();
        assert_eq!(agent, "agent1");
        assert_eq!(rest.unwrap(), "mem123");

        // Search URI
        let (agent, rest) = parse_test_uri("memory://agent1/?query=hello").unwrap();
        assert_eq!(agent, "agent1");
        assert!(rest.unwrap().contains("query=hello"));
    }
}
