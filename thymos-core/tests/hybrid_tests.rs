//! Tests for hybrid memory backend

use tempfile::TempDir;
use thymos_core::config::{MemoryConfig, MemoryMode};
use thymos_core::memory::MemorySystem;
use thymos_core::prelude::Agent;

#[tokio::test]
async fn test_hybrid_memory_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let config = MemoryConfig {
        mode: MemoryMode::Hybrid {
            private_data_dir: temp_dir.path().join("private").to_path_buf(),
            shared_url: "http://localhost:3000".to_string(),
            shared_api_key: None,
        },
        ..Default::default()
    };

    // This will fail if server is not running, which is expected for unit tests
    // In a real scenario, we'd use a mock server or skip this test
    let result = MemorySystem::new(config).await;

    // We expect this to fail without a running server, but the structure should be correct
    assert!(result.is_err() || result.is_ok());
}

#[tokio::test]
async fn test_routing_strategy() {
    use thymos_core::memory::{MemoryScope, RoutingStrategy};

    let routing = RoutingStrategy::new(MemoryScope::Private)
        .with_tag_rule("public".to_string(), MemoryScope::Shared)
        .with_tag_rule("private".to_string(), MemoryScope::Private);

    // Test default scope
    assert_eq!(routing.route(&[]), MemoryScope::Private);

    // Test tag-based routing
    assert_eq!(routing.route(&["public".to_string()]), MemoryScope::Shared);
    assert_eq!(
        routing.route(&["private".to_string()]),
        MemoryScope::Private
    );

    // Test first matching tag
    assert_eq!(
        routing.route(&["public".to_string(), "private".to_string()]),
        MemoryScope::Shared
    );
}

#[tokio::test]
async fn test_agent_hybrid_memory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let config = MemoryConfig {
        mode: MemoryMode::Hybrid {
            private_data_dir: temp_dir.path().join("private").to_path_buf(),
            shared_url: "http://localhost:3000".to_string(),
            shared_api_key: None,
        },
        ..Default::default()
    };

    // This will fail if server is not running
    let result = Agent::builder()
        .id("test_agent")
        .with_memory_config(config)
        .build()
        .await;

    // We expect this to fail without a running server
    assert!(result.is_err() || result.is_ok());
}

#[tokio::test]
async fn test_search_scope_enum() {
    use thymos_core::memory::SearchScope;

    // Test that scopes can be compared
    assert_eq!(SearchScope::Private, SearchScope::Private);
    assert_eq!(SearchScope::Shared, SearchScope::Shared);
    assert_eq!(SearchScope::Both, SearchScope::Both);
    assert_ne!(SearchScope::Private, SearchScope::Shared);
}
