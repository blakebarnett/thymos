//! Fixture definitions for evaluation
//!
//! Fixtures define the inputs, expected outputs, and stubbed responses
//! for deterministic evaluation runs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// A complete evaluation fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    /// Fixture name/identifier
    pub name: String,

    /// Description of what this fixture tests
    pub description: Option<String>,

    /// Schema version
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Input for the evaluation
    pub input: FixtureInput,

    /// Expected outputs
    pub expected: FixtureExpectation,

    /// Stubbed tool responses
    #[serde(default)]
    pub tool_stubs: HashMap<String, Vec<StubResponseDef>>,

    /// Memory snapshot to load before evaluation
    pub memory_snapshot: Option<MemorySnapshot>,
}

fn default_schema_version() -> u32 {
    1
}

impl Fixture {
    /// Create a new empty fixture
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            schema_version: 1,
            tags: Vec::new(),
            input: FixtureInput::default(),
            expected: FixtureExpectation::default(),
            tool_stubs: HashMap::new(),
            memory_snapshot: None,
        }
    }

    /// Load a fixture from a JSON file
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    /// Save fixture to a JSON file
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
    }

    /// Add a tool stub
    pub fn with_tool_stub(
        mut self,
        tool_name: impl Into<String>,
        responses: Vec<StubResponseDef>,
    ) -> Self {
        self.tool_stubs.insert(tool_name.into(), responses);
        self
    }

    /// Set memory snapshot
    pub fn with_memory_snapshot(mut self, snapshot: MemorySnapshot) -> Self {
        self.memory_snapshot = Some(snapshot);
        self
    }
}

/// Input for an evaluation run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixtureInput {
    /// Primary query/prompt
    pub query: Option<String>,

    /// Arguments to pass
    #[serde(default)]
    pub args: Value,

    /// Context values
    #[serde(default)]
    pub context: Value,

    /// Agent ID to use
    pub agent_id: Option<String>,
}

impl FixtureInput {
    /// Create input with a query
    pub fn with_query(query: impl Into<String>) -> Self {
        Self {
            query: Some(query.into()),
            args: Value::Null,
            context: Value::Null,
            agent_id: None,
        }
    }

    /// Set arguments
    pub fn with_args(mut self, args: Value) -> Self {
        self.args = args;
        self
    }

    /// Set context
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = context;
        self
    }
}

/// Expected outputs for validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixtureExpectation {
    /// Expected final output (exact match)
    pub output: Option<Value>,

    /// Patterns that must appear in output (substring match)
    #[serde(default)]
    pub output_contains: Vec<String>,

    /// Tool calls that should have been made
    #[serde(default)]
    pub tool_calls: Vec<ExpectedToolCall>,

    /// Minimum number of tool calls expected
    pub min_tool_calls: Option<usize>,

    /// Maximum number of tool calls expected
    pub max_tool_calls: Option<usize>,

    /// Expected execution time bounds (ms)
    pub max_duration_ms: Option<u64>,

    /// Should the run succeed?
    #[serde(default = "default_true")]
    pub should_succeed: bool,

    /// Error type expected (if should_succeed is false)
    pub expected_error: Option<String>,

    /// Custom validators (JSON paths to check)
    #[serde(default)]
    pub json_path_checks: Vec<JsonPathCheck>,
}

fn default_true() -> bool {
    true
}

impl FixtureExpectation {
    /// Expect a specific output value
    pub fn output(value: Value) -> Self {
        Self {
            output: Some(value),
            ..Default::default()
        }
    }

    /// Add a substring that must appear in output
    pub fn with_contains(mut self, pattern: impl Into<String>) -> Self {
        self.output_contains.push(pattern.into());
        self
    }

    /// Add an expected tool call
    pub fn with_tool_call(mut self, call: ExpectedToolCall) -> Self {
        self.tool_calls.push(call);
        self
    }

    /// Set as expecting failure
    pub fn expect_failure(mut self) -> Self {
        self.should_succeed = false;
        self
    }
}

/// Expected tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedToolCall {
    /// Tool name
    pub tool_name: String,

    /// Expected arguments (partial match)
    pub args: Option<Value>,

    /// Whether this call should succeed
    #[serde(default = "default_true")]
    pub should_succeed: bool,
}

impl ExpectedToolCall {
    /// Create a new expected tool call
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            args: None,
            should_succeed: true,
        }
    }

    /// With expected arguments
    pub fn with_args(mut self, args: Value) -> Self {
        self.args = Some(args);
        self
    }
}

/// JSON path check for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPathCheck {
    /// JSON path expression
    pub path: String,

    /// Expected value at path
    pub value: Value,

    /// Comparison operator
    #[serde(default)]
    pub operator: CheckOperator,
}

/// Comparison operators for JSON path checks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOperator {
    /// Exact match
    #[default]
    Equals,
    /// Not equal
    NotEquals,
    /// Contains (for strings/arrays)
    Contains,
    /// Greater than (for numbers)
    GreaterThan,
    /// Less than (for numbers)
    LessThan,
    /// Value exists (not null)
    Exists,
}

/// Stub response definition in fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubResponseDef {
    /// When to return this response (call index, default 0 = always)
    #[serde(default)]
    pub call_index: Option<usize>,

    /// Args pattern to match (if specified, only match when args match)
    pub match_args: Option<Value>,

    /// Response to return
    pub response: StubResponseValue,

    /// Delay to simulate (ms)
    #[serde(default)]
    pub delay_ms: u64,
}

/// Stub response value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StubResponseValue {
    /// Success with value
    Success(Value),

    /// Error response
    Error {
        /// Error kind
        error_kind: String,
        /// Error message
        message: String,
    },
}

/// Memory snapshot for deterministic evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Snapshot identifier
    pub id: String,

    /// Memories to pre-load
    pub memories: Vec<SnapshotMemory>,

    /// Concepts to pre-load
    #[serde(default)]
    pub concepts: Vec<SnapshotConcept>,
}

impl MemorySnapshot {
    /// Create a new empty snapshot
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            memories: Vec::new(),
            concepts: Vec::new(),
        }
    }

    /// Add a memory
    pub fn with_memory(mut self, memory: SnapshotMemory) -> Self {
        self.memories.push(memory);
        self
    }
}

/// Memory entry in snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMemory {
    /// Memory ID
    pub id: String,

    /// Content
    pub content: String,

    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Importance score (0.0-1.0)
    #[serde(default = "default_importance")]
    pub importance: f32,

    /// Embedding (optional, for semantic search testing)
    pub embedding: Option<Vec<f32>>,
}

fn default_importance() -> f32 {
    0.5
}

impl SnapshotMemory {
    /// Create a new memory entry
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            tags: Vec::new(),
            importance: 0.5,
            embedding: None,
        }
    }
}

/// Concept entry in snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConcept {
    /// Concept ID
    pub id: String,

    /// Concept name
    pub name: String,

    /// Concept type
    pub concept_type: Option<String>,

    /// Aliases
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[test]
    fn test_fixture_creation() {
        let fixture = Fixture::new("test_fixture")
            .with_tool_stub("echo", vec![StubResponseDef {
                call_index: None,
                match_args: None,
                response: StubResponseValue::Success(serde_json::json!("hello")),
                delay_ms: 0,
            }]);

        assert_eq!(fixture.name, "test_fixture");
        assert!(fixture.tool_stubs.contains_key("echo"));
    }

    #[test]
    fn test_fixture_serialization() {
        let fixture = Fixture {
            name: "serialization_test".to_string(),
            description: Some("Test fixture serialization".to_string()),
            schema_version: 1,
            tags: vec!["test".to_string()],
            input: FixtureInput::with_query("test query")
                .with_args(serde_json::json!({"key": "value"})),
            expected: FixtureExpectation::output(serde_json::json!({"result": 42}))
                .with_contains("result")
                .with_tool_call(ExpectedToolCall::new("test_tool")),
            tool_stubs: HashMap::new(),
            memory_snapshot: Some(MemorySnapshot::new("snapshot_1")
                .with_memory(SnapshotMemory::new("mem_1", "Test memory content"))),
        };

        let json = serde_json::to_string(&fixture).unwrap();
        let parsed: Fixture = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, fixture.name);
        assert_eq!(parsed.input.query, fixture.input.query);
        assert!(parsed.memory_snapshot.is_some());
    }
}



