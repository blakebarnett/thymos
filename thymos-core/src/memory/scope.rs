//! Memory scope types for hybrid memory mode and named scopes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory scope determines where a memory is stored (backend selection)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    /// Private memory stored in embedded backend
    Private,
    /// Shared memory stored in server backend
    Shared,
}

/// Search scope determines which backends to search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchScope {
    /// Search only private memories
    Private,
    /// Search only shared memories
    Shared,
    /// Search both backends and merge results
    Both,
}

/// Named scope with configurable behavior
///
/// Named scopes allow organizing memories by domain with different
/// decay rates, importance multipliers, and search weights.
///
/// # Example
///
/// ```rust
/// use thymos_core::memory::MemoryScopeConfig;
///
/// let personal_scope = MemoryScopeConfig::new("personal")
///     .with_decay_hours(336.0)  // 2 weeks
///     .with_importance_multiplier(1.0)
///     .with_search_weight(1.0);
///
/// let observations_scope = MemoryScopeConfig::new("observations")
///     .with_decay_hours(24.0)   // 1 day
///     .with_importance_multiplier(0.5)
///     .with_search_weight(0.3)
///     .with_max_memories(1000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScopeConfig {
    /// Scope name (unique identifier)
    pub name: String,

    /// Decay rate in hours (for forgetting curve)
    /// Lower values = faster decay
    pub decay_hours: f64,

    /// Importance multiplier for this scope's memories
    /// Higher values = more resistant to forgetting
    pub importance_multiplier: f64,

    /// Weight when searching across scopes (0.0-1.0)
    /// Higher values = prioritized in merged results
    pub search_weight: f64,

    /// Optional: Max memories before pruning oldest
    pub max_memories: Option<usize>,
}

impl MemoryScopeConfig {
    /// Create a new scope configuration with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the decay rate in hours
    pub fn with_decay_hours(mut self, hours: f64) -> Self {
        self.decay_hours = hours;
        self
    }

    /// Set the importance multiplier
    pub fn with_importance_multiplier(mut self, multiplier: f64) -> Self {
        self.importance_multiplier = multiplier;
        self
    }

    /// Set the search weight (0.0-1.0)
    pub fn with_search_weight(mut self, weight: f64) -> Self {
        self.search_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Set the maximum number of memories in this scope
    pub fn with_max_memories(mut self, max: usize) -> Self {
        self.max_memories = Some(max);
        self
    }

    /// Get the scope tag used for storage
    pub fn scope_tag(&self) -> String {
        format!("_scope:{}", self.name)
    }
}

impl Default for MemoryScopeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            decay_hours: 168.0, // 1 week
            importance_multiplier: 1.0,
            search_weight: 1.0,
            max_memories: None,
        }
    }
}

/// Registry for managing named memory scopes
#[derive(Debug)]
pub struct ScopeRegistry {
    scopes: Arc<RwLock<HashMap<String, MemoryScopeConfig>>>,
}

impl ScopeRegistry {
    /// Create a new scope registry with a default scope
    pub fn new() -> Self {
        let mut scopes = HashMap::new();
        scopes.insert("default".to_string(), MemoryScopeConfig::default());
        Self {
            scopes: Arc::new(RwLock::new(scopes)),
        }
    }

    /// Define a new scope or update an existing one
    pub async fn define_scope(&self, config: MemoryScopeConfig) -> crate::error::Result<()> {
        let mut scopes = self.scopes.write().await;
        scopes.insert(config.name.clone(), config);
        Ok(())
    }

    /// Get a scope configuration by name
    pub async fn get_scope(&self, name: &str) -> Option<MemoryScopeConfig> {
        let scopes = self.scopes.read().await;
        scopes.get(name).cloned()
    }

    /// Check if a scope exists
    pub async fn has_scope(&self, name: &str) -> bool {
        let scopes = self.scopes.read().await;
        scopes.contains_key(name)
    }

    /// List all defined scopes
    pub async fn list_scopes(&self) -> Vec<MemoryScopeConfig> {
        let scopes = self.scopes.read().await;
        scopes.values().cloned().collect()
    }

    /// Remove a scope (cannot remove "default")
    pub async fn remove_scope(&self, name: &str) -> crate::error::Result<()> {
        if name == "default" {
            return Err(crate::error::ThymosError::Configuration(
                "Cannot remove the default scope".to_string(),
            ));
        }
        let mut scopes = self.scopes.write().await;
        scopes.remove(name);
        Ok(())
    }

    /// Get the default scope configuration
    pub async fn default_scope(&self) -> MemoryScopeConfig {
        self.get_scope("default")
            .await
            .unwrap_or_else(MemoryScopeConfig::default)
    }

    /// Extract scope name from memory tags
    pub fn extract_scope_from_tags(tags: &[String]) -> Option<String> {
        for tag in tags {
            if let Some(scope_name) = tag.strip_prefix("_scope:") {
                return Some(scope_name.to_string());
            }
        }
        None
    }
}

impl Default for ScopeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ScopeRegistry {
    fn clone(&self) -> Self {
        Self {
            scopes: Arc::clone(&self.scopes),
        }
    }
}

/// A memory with its score and scope information
#[derive(Debug, Clone)]
pub struct ScopedMemory {
    /// The memory
    pub memory: locai::models::Memory,
    /// Search relevance score (0.0-1.0)
    pub score: f64,
    /// Scope this memory belongs to
    pub scope: String,
    /// Weighted score (score * scope.search_weight)
    pub weighted_score: f64,
}

impl ScopedMemory {
    /// Create a new scoped memory
    pub fn new(memory: locai::models::Memory, score: f64, scope: String, weight: f64) -> Self {
        Self {
            memory,
            score,
            weighted_score: score * weight,
            scope,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_config_builder() {
        let config = MemoryScopeConfig::new("personal")
            .with_decay_hours(336.0)
            .with_importance_multiplier(1.5)
            .with_search_weight(0.8)
            .with_max_memories(500);

        assert_eq!(config.name, "personal");
        assert_eq!(config.decay_hours, 336.0);
        assert_eq!(config.importance_multiplier, 1.5);
        assert_eq!(config.search_weight, 0.8);
        assert_eq!(config.max_memories, Some(500));
    }

    #[test]
    fn test_scope_config_default() {
        let config = MemoryScopeConfig::default();

        assert_eq!(config.name, "default");
        assert_eq!(config.decay_hours, 168.0);
        assert_eq!(config.importance_multiplier, 1.0);
        assert_eq!(config.search_weight, 1.0);
        assert!(config.max_memories.is_none());
    }

    #[test]
    fn test_scope_tag() {
        let config = MemoryScopeConfig::new("personal");
        assert_eq!(config.scope_tag(), "_scope:personal");
    }

    #[test]
    fn test_search_weight_clamping() {
        let config = MemoryScopeConfig::new("test").with_search_weight(1.5);
        assert_eq!(config.search_weight, 1.0);

        let config = MemoryScopeConfig::new("test").with_search_weight(-0.5);
        assert_eq!(config.search_weight, 0.0);
    }

    #[tokio::test]
    async fn test_scope_registry() {
        let registry = ScopeRegistry::new();

        // Default scope exists
        assert!(registry.has_scope("default").await);

        // Define new scope
        let personal = MemoryScopeConfig::new("personal").with_decay_hours(336.0);
        registry.define_scope(personal).await.unwrap();

        assert!(registry.has_scope("personal").await);

        let config = registry.get_scope("personal").await.unwrap();
        assert_eq!(config.decay_hours, 336.0);
    }

    #[tokio::test]
    async fn test_scope_registry_cannot_remove_default() {
        let registry = ScopeRegistry::new();

        let result = registry.remove_scope("default").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_scope_from_tags() {
        let tags = vec![
            "important".to_string(),
            "_scope:personal".to_string(),
            "project-x".to_string(),
        ];

        let scope = ScopeRegistry::extract_scope_from_tags(&tags);
        assert_eq!(scope, Some("personal".to_string()));

        let no_scope_tags = vec!["tag1".to_string(), "tag2".to_string()];
        let scope = ScopeRegistry::extract_scope_from_tags(&no_scope_tags);
        assert!(scope.is_none());
    }
}
