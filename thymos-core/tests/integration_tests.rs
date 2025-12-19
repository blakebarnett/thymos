//! Integration tests for AutoAgents integration components
//!
//! These tests verify that the various integration components work together correctly,
//! including the event system, tool registry, and skills.

#[cfg(feature = "pubsub")]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;
    use thymos_core::config::{MemoryConfig, MemoryMode};
    use thymos_core::integration::{
        event_channel, AgentEvent, ThymosAgentConfig, ThymosAgentCore,
    };
    use thymos_core::memory::MemorySystem;
    use thymos_core::prelude::*;

    async fn create_test_memory() -> (Arc<MemorySystem>, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MemoryConfig {
            mode: MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };
        let memory = MemorySystem::new(config)
            .await
            .expect("Failed to create memory system");
        (Arc::new(memory), temp_dir)
    }

    #[tokio::test]
    async fn test_tool_registry_with_skills() {
        // Create a skill with memory tools
        let (memory, _temp_dir) = create_test_memory().await;
        let skill = create_memory_skill(memory);

        // Convert skill to registry
        let registry = skill.to_registry();

        // Verify tools are registered
        assert!(registry.get("memory_search").is_some());
        assert!(registry.get("memory_store").is_some());

        // Test discovery
        let results = registry.discover("search");
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.tool.name() == "memory_search"));
    }

    #[tokio::test]
    async fn test_event_streaming_lifecycle() {
        let (memory, _temp_dir) = create_test_memory().await;
        let (tx, mut rx) = event_channel(100);

        // Create agent with event emitter
        let agent = ThymosAgentCore::builder()
            .name("event_test_agent")
            .description("An agent for testing events")
            .memory(memory)
            .event_sender(tx)
            .policy(CapabilityPolicy::allow_all())
            .build()
            .expect("Failed to build agent");

        // Verify event emitter is configured
        let emitter = agent.event_emitter().expect("Event emitter should be configured");

        // Simulate task lifecycle by emitting events directly
        emitter.task_started("test-task-1", "Test prompt").await;
        emitter.turn_started(0).await;
        emitter
            .tool_call_started("test_tool", serde_json::json!({"arg": "value"}))
            .await;
        emitter.tool_call_completed("test_tool", true, 100, None).await;
        emitter.turn_completed(0, true, true).await;
        emitter
            .task_completed(
                "test-task-1",
                true,
                Some("Response".to_string()),
                None,
                1,
                1,
                500,
            )
            .await;

        // Collect events with timeout
        let mut events = Vec::new();
        let timeout = tokio::time::timeout(Duration::from_millis(100), async {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        });
        let _ = timeout.await;

        // Verify event sequence
        assert!(events.len() >= 6, "Expected at least 6 events, got {}", events.len());

        // Verify event types
        let event_types: Vec<&str> = events.iter().map(|e| e.event_type()).collect();
        assert!(event_types.contains(&"task_started"));
        assert!(event_types.contains(&"turn_started"));
        assert!(event_types.contains(&"tool_call_started"));
        assert!(event_types.contains(&"tool_call_completed"));
        assert!(event_types.contains(&"turn_completed"));
        assert!(event_types.contains(&"task_completed"));
    }

    #[tokio::test]
    async fn test_agent_event_serialization_roundtrip() {
        let event = AgentEvent::TaskCompleted {
            task_id: "task-123".to_string(),
            agent_name: "test_agent".to_string(),
            success: true,
            response: Some("The answer is 42".to_string()),
            error: None,
            turns: 3,
            tool_calls: 2,
            duration_ms: 1500,
            timestamp: chrono::Utc::now(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&event).expect("Serialization failed");

        // Deserialize back
        let parsed: AgentEvent = serde_json::from_str(&json).expect("Deserialization failed");

        // Verify
        assert_eq!(parsed.event_type(), "task_completed");
        assert_eq!(parsed.task_id(), Some("task-123"));
        assert_eq!(parsed.agent_name(), Some("test_agent"));
    }

    #[tokio::test]
    async fn test_skill_tools_access() {
        let (memory, _temp_dir) = create_test_memory().await;
        let skill = create_memory_skill(memory);

        // Get allowed tools from skill
        let allowed = skill.allowed_tools();
        assert_eq!(allowed.len(), 2);

        // Verify tool names
        let tool_names: Vec<&str> = allowed.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"memory_search"));
        assert!(tool_names.contains(&"memory_store"));
    }

    #[tokio::test]
    async fn test_registry_mcp_tool_export() {
        let (memory, _temp_dir) = create_test_memory().await;
        let skill = create_memory_skill(memory);
        let registry = skill.to_registry();

        // Get MCP-compatible tool info
        let mcp_tools = registry.mcp_tools();

        assert_eq!(mcp_tools.len(), 2);
        for tool_info in &mcp_tools {
            assert!(!tool_info.name.is_empty());
            assert!(!tool_info.description.is_empty());
            assert!(!tool_info.input_schema.is_null());
        }
    }

    #[tokio::test]
    async fn test_agent_with_replay_and_events() {
        let (memory, _temp_dir) = create_test_memory().await;
        let (tx, _rx) = event_channel(100);

        // Create agent with both replay and events enabled
        let agent = ThymosAgentCore::builder()
            .name("dual_capture_agent")
            .description("An agent with replay and event capture")
            .memory(memory)
            .event_sender(tx)
            .with_replay(true)
            .config(ThymosAgentConfig::new().with_verbose(true))
            .build()
            .expect("Failed to build agent");

        // Verify both are configured
        assert!(agent.event_emitter().is_some());
        assert!(agent.replay_capture().is_some());
    }

    #[tokio::test]
    async fn test_memory_skill_execution() {
        let (memory, _temp_dir) = create_test_memory().await;

        // Store something in memory first
        memory
            .remember("The capital of France is Paris".to_string())
            .await
            .expect("Failed to store memory");

        // Create skill
        let skill = create_memory_skill(Arc::clone(&memory));

        // Get the store tool and verify it exists
        let registry = skill.to_registry();
        let store_tool = registry.get("memory_store");
        assert!(store_tool.is_some(), "memory_store tool should exist");

        let search_tool = registry.get("memory_search");
        assert!(search_tool.is_some(), "memory_search tool should exist");
    }
}

