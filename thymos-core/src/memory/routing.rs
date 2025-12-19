//! Routing strategy for hybrid memory mode

use super::scope::MemoryScope;
use std::collections::HashMap;

/// Routing strategy determines which backend to use for memories
#[derive(Debug, Clone)]
pub struct RoutingStrategy {
    /// Default scope when no tag matches
    pub default_scope: MemoryScope,
    /// Tag-based routing rules
    pub tag_rules: HashMap<String, MemoryScope>,
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self {
            default_scope: MemoryScope::Private,
            tag_rules: HashMap::new(),
        }
    }
}

impl RoutingStrategy {
    /// Create a new routing strategy with default scope
    pub fn new(default_scope: MemoryScope) -> Self {
        Self {
            default_scope,
            tag_rules: HashMap::new(),
        }
    }

    /// Add a tag-based routing rule
    pub fn with_tag_rule(mut self, tag: impl Into<String>, scope: MemoryScope) -> Self {
        self.tag_rules.insert(tag.into(), scope);
        self
    }

    /// Determine scope for a memory based on its tags
    pub fn route(&self, tags: &[String]) -> MemoryScope {
        for tag in tags {
            if let Some(scope) = self.tag_rules.get(tag) {
                return *scope;
            }
        }
        self.default_scope
    }
}
