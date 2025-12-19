//! Integration tests for eval module

use super::*;
use serde_json::json;
use std::collections::HashMap;

/// Golden test: echo tool returns input
#[tokio::test]
async fn golden_echo_tool() {
    let fixture = Fixture {
        name: "golden_echo".to_string(),
        description: Some("Echo tool returns input unchanged".to_string()),
        schema_version: 1,
        tags: vec!["golden".to_string(), "echo".to_string()],
        input: FixtureInput::with_query("test message")
            .with_args(json!({"message": "hello world"})),
        expected: FixtureExpectation {
            output: Some(json!("hello world")),
            output_contains: vec!["hello".to_string()],
            tool_calls: vec![fixture::ExpectedToolCall::new("echo")],
            should_succeed: true,
            ..Default::default()
        },
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let harness = EvalHarness::new()
        .with_stub(StubTool::new("echo", StubResponse::success(json!("hello world"))));

    let result = harness.run(&fixture).await;

    assert!(result.passed(), "Golden echo test failed: {:?}", result.failure_messages());
    assert_eq!(result.tool_calls.len(), 1);
    assert!(result.duration.as_millis() < 1000); // Should be fast
}

/// Golden test: error handling
#[tokio::test]
async fn golden_error_handling() {
    let mut expected_call = fixture::ExpectedToolCall::new("failing_tool");
    expected_call.should_succeed = false;

    let fixture = Fixture {
        name: "golden_error".to_string(),
        description: Some("Verify error handling works correctly".to_string()),
        schema_version: 1,
        tags: vec!["golden".to_string(), "error".to_string()],
        input: FixtureInput::default(),
        expected: FixtureExpectation {
            should_succeed: false,
            tool_calls: vec![expected_call],
            ..Default::default()
        },
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let harness = EvalHarness::new().with_stub(StubTool::new(
        "failing_tool",
        StubResponse::error(crate::tools::ToolErrorKind::Timeout, "Simulated timeout"),
    ));

    let result = harness.run(&fixture).await;

    assert!(result.passed(), "Golden error test failed: {:?}", result.failure_messages());
    assert!(!result.tool_calls[0].success);
}

/// Golden test: tool sequence
#[tokio::test]
async fn golden_tool_sequence() {
    let fixture = Fixture {
        name: "golden_sequence".to_string(),
        description: Some("Multiple tools called in sequence".to_string()),
        schema_version: 1,
        tags: vec!["golden".to_string(), "sequence".to_string()],
        input: FixtureInput::default(),
        expected: FixtureExpectation {
            min_tool_calls: Some(2),
            max_tool_calls: Some(3),
            should_succeed: true,
            ..Default::default()
        },
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let harness = EvalHarness::new()
        .with_stub(StubTool::new("tool1", StubResponse::success(json!({"step": 1}))))
        .with_stub(StubTool::new("tool2", StubResponse::success(json!({"step": 2}))));

    let result = harness.run(&fixture).await;

    assert!(result.passed(), "Golden sequence test failed: {:?}", result.failure_messages());
    assert_eq!(result.tool_calls.len(), 2);
}

/// Test fixture file operations
#[tokio::test]
async fn test_fixture_save_load() {
    let mut stubs = HashMap::new();
    stubs.insert(
        "search".to_string(),
        vec![fixture::StubResponseDef {
            call_index: None,
            match_args: None,
            response: fixture::StubResponseValue::Success(json!({"results": ["a", "b"]})),
            delay_ms: 10,
        }],
    );

    let fixture = Fixture {
        name: "persistence_test".to_string(),
        description: Some("Test fixture persistence".to_string()),
        schema_version: 1,
        tags: vec!["test".to_string()],
        input: FixtureInput::with_query("search query")
            .with_args(json!({"query": "test"})),
        expected: FixtureExpectation {
            output: Some(json!({"results": ["a", "b"]})),
            tool_calls: vec![fixture::ExpectedToolCall::new("search")],
            should_succeed: true,
            ..Default::default()
        },
        tool_stubs: stubs,
        memory_snapshot: Some(MemorySnapshot::new("test_snapshot")
            .with_memory(fixture::SnapshotMemory::new("mem_1", "Test memory"))),
    };

    // Save to temp file
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("fixture.json");

    fixture.save(&path).unwrap();

    // Load back and verify
    let loaded = Fixture::load(&path).unwrap();

    assert_eq!(loaded.name, fixture.name);
    assert_eq!(loaded.input.query, fixture.input.query);
    assert!(loaded.tool_stubs.contains_key("search"));
    assert!(loaded.memory_snapshot.is_some());
}

/// Test JSON path validation
#[tokio::test]
async fn test_json_path_validation() {
    let fixture = Fixture {
        name: "json_path_test".to_string(),
        description: None,
        schema_version: 1,
        tags: vec![],
        input: FixtureInput::default(),
        expected: FixtureExpectation {
            json_path_checks: vec![
                fixture::JsonPathCheck {
                    path: "data.value".to_string(),
                    value: json!(42),
                    operator: fixture::CheckOperator::Equals,
                },
                fixture::JsonPathCheck {
                    path: "data.name".to_string(),
                    value: json!("test"),
                    operator: fixture::CheckOperator::Contains,
                },
            ],
            should_succeed: true,
            ..Default::default()
        },
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let harness = EvalHarness::new().with_stub(StubTool::new(
        "test",
        StubResponse::success(json!({
            "data": {
                "value": 42,
                "name": "test_name"
            }
        })),
    ));

    let result = harness.run(&fixture).await;

    assert!(result.passed(), "JSON path test failed: {:?}", result.failure_messages());
}

/// Test output contains validation
#[tokio::test]
async fn test_output_contains() {
    let fixture = Fixture {
        name: "contains_test".to_string(),
        description: None,
        schema_version: 1,
        tags: vec![],
        input: FixtureInput::default(),
        expected: FixtureExpectation {
            output_contains: vec![
                "success".to_string(),
                "result".to_string(),
            ],
            should_succeed: true,
            ..Default::default()
        },
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let harness = EvalHarness::new().with_stub(StubTool::new(
        "test",
        StubResponse::success(json!({
            "status": "success",
            "result": "completed"
        })),
    ));

    let result = harness.run(&fixture).await;

    assert!(result.passed(), "Contains test failed: {:?}", result.failure_messages());
}

/// Test replay integration
#[tokio::test]
async fn test_eval_with_replay() {
    let fixture = Fixture {
        name: "replay_integration".to_string(),
        description: None,
        schema_version: 1,
        tags: vec![],
        input: FixtureInput::with_query("test"),
        expected: FixtureExpectation::default(),
        tool_stubs: HashMap::new(),
        memory_snapshot: None,
    };

    let config = EvalRunConfig {
        capture_replay: true,
        ..Default::default()
    };

    let harness = EvalHarness::with_config(config)
        .with_stub(StubTool::new("tool1", StubResponse::success(json!("result1"))))
        .with_stub(StubTool::new("tool2", StubResponse::success(json!("result2"))));

    let result = harness.run(&fixture).await;

    assert!(result.replay.is_some());

    let replay = result.replay.unwrap();
    // Should have: SessionStart, ToolCall x2, SessionEnd
    assert!(replay.events.len() >= 4);

    // Verify tool calls are recorded
    let tool_calls: Vec<_> = replay.tool_calls().collect();
    assert_eq!(tool_calls.len(), 2);
}

