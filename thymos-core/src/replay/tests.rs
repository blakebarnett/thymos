//! Integration tests for replay module

use super::*;
use crate::tools::{ToolProvenance, ToolResultEnvelope};
use serde_json::json;

#[tokio::test]
async fn test_full_replay_workflow() {
    // Create a capture session
    let capture = ReplayCapture::new("workflow_test");
    capture.set_agent_id("test_agent").await;

    // Start session
    capture.start_session().await;

    // Record a tool call
    let provenance = ToolProvenance::new("web_search", "query_hash_123")
        .with_duration(std::time::Duration::from_millis(250));
    let envelope = ToolResultEnvelope::success(
        json!({
            "results": [
                {"title": "Result 1", "url": "https://example.com/1"},
                {"title": "Result 2", "url": "https://example.com/2"}
            ]
        }),
        provenance,
    );

    capture
        .record_tool_call(
            "web_search",
            json!({"query": "rust async programming"}),
            &envelope,
        )
        .await;

    // Record an LLM call
    capture
        .record_llm_call(LlmCallEvent {
            provider: "groq".to_string(),
            model: "llama-3-70b".to_string(),
            input_tokens: Some(500),
            output_tokens: Some(200),
            prompt_hash: "prompt_hash_456".to_string(),
            streaming: false,
            latency_ms: 1200,
            cost_usd: Some(0.002),
            trace_id: None,
            temperature: Some(0.7),
            stop_reason: Some("stop".to_string()),
        })
        .await;

    // Record memory retrieval
    capture
        .record_memory_retrieval(
            "async programming patterns",
            vec!["mem_abc".to_string(), "mem_def".to_string()],
            45,
            None,
        )
        .await;

    // Record versioning
    capture
        .record_versioning(
            VersioningOperation::Commit,
            Some("main".to_string()),
            Some("commit_789".to_string()),
            None,
            true,
            None,
            None,
        )
        .await;

    // End session
    let record = capture.end_session().await;

    // Verify record contents
    assert_eq!(record.session_id, "workflow_test");
    assert_eq!(record.agent_id, Some("test_agent".to_string()));
    assert!(record.ended_at.is_some());

    // Check event counts
    assert_eq!(record.tool_calls().count(), 1);
    assert_eq!(record.llm_calls().count(), 1);
    assert_eq!(record.memory_retrievals().count(), 1);

    // Verify tool call details
    let tool_call = record.tool_calls().next().unwrap();
    assert_eq!(tool_call.tool_name, "web_search");
    assert_eq!(tool_call.status, ToolCallStatus::Success);
    assert_eq!(tool_call.duration_ms, 250);

    // Verify LLM call details
    let llm_call = record.llm_calls().next().unwrap();
    assert_eq!(llm_call.model, "llama-3-70b");
    assert_eq!(llm_call.input_tokens, Some(500));
    assert_eq!(llm_call.latency_ms, 1200);

    // Verify memory retrieval details
    let mem_retrieval = record.memory_retrievals().next().unwrap();
    assert_eq!(mem_retrieval.result_count, 2);
}

#[tokio::test]
async fn test_record_persistence_roundtrip() {
    let mut record = ReplayRecord::new("persistence_test")
        .with_agent_id("agent_persist");

    record.metadata.description = Some("Testing persistence".to_string());
    record.metadata.secrets_redacted = true;
    record.metadata.tags = vec!["test".to_string(), "persistence".to_string()];

    // Add various event types
    record.push_event(ReplayEvent::SessionStart(SessionEvent {
        session_id: "persistence_test".to_string(),
        agent_id: Some("agent_persist".to_string()),
        context: json!({"test": true}),
    }));

    record.push_event(ReplayEvent::ToolCall(ToolCallEvent {
        tool_name: "echo".to_string(),
        tool_version: Some("1.0.0".to_string()),
        args: json!({"message": "hello"}),
        args_hash: "hash_abc".to_string(),
        status: ToolCallStatus::Success,
        result: Some(json!("hello")),
        error: None,
        duration_ms: 5,
        trace_id: Some("trace_1".to_string()),
    }));

    record.push_event(ReplayEvent::LlmCall(LlmCallEvent {
        provider: "test".to_string(),
        model: "test-model".to_string(),
        input_tokens: Some(100),
        output_tokens: Some(50),
        prompt_hash: "prompt_hash".to_string(),
        streaming: true,
        latency_ms: 500,
        cost_usd: None,
        trace_id: None,
        temperature: None,
        stop_reason: None,
    }));

    record.finish();

    // Save to temp file
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("persistence_test.jsonl");

    record.save(&path).unwrap();

    // Load back and verify
    let loaded = ReplayRecord::load(&path).unwrap();

    assert_eq!(loaded.schema_version, record.schema_version);
    assert_eq!(loaded.session_id, record.session_id);
    assert_eq!(loaded.agent_id, record.agent_id);
    assert_eq!(loaded.events.len(), record.events.len());

    // Verify metadata
    assert_eq!(loaded.metadata.description, record.metadata.description);
    assert_eq!(loaded.metadata.tags, record.metadata.tags);
    assert_eq!(loaded.metadata.secrets_redacted, record.metadata.secrets_redacted);

    // Verify events are correct type
    assert!(matches!(loaded.events[0].event, ReplayEvent::SessionStart(_)));
    assert!(matches!(loaded.events[1].event, ReplayEvent::ToolCall(_)));
    assert!(matches!(loaded.events[2].event, ReplayEvent::LlmCall(_)));
}

#[tokio::test]
async fn test_error_event_recording() {
    let capture = ReplayCapture::new("error_test");

    capture.start_session().await;

    // Record a failed tool call
    let provenance = ToolProvenance::new("failing_tool", "fail_hash");
    let error = crate::tools::ToolError::timeout(std::time::Duration::from_secs(30));
    let envelope = ToolResultEnvelope::error(error, provenance);

    capture
        .record_tool_call("failing_tool", json!({"param": "value"}), &envelope)
        .await;

    let record = capture.end_session().await;

    let tool_call = record.tool_calls().next().unwrap();
    assert_eq!(tool_call.status, ToolCallStatus::Error);
    assert!(tool_call.error.is_some());

    let error = tool_call.error.as_ref().unwrap();
    assert_eq!(error.kind, crate::tools::ToolErrorKind::Timeout);
}



