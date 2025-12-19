//! Evaluation harness for running fixtures
//!
//! The harness coordinates:
//! - Loading fixtures
//! - Setting up stub tools
//! - Running workflows
//! - Validating outputs against expectations

use super::fixture::{CheckOperator, ExpectedToolCall, Fixture, FixtureExpectation, JsonPathCheck};
use super::stub::{StubRegistry, StubTool};
use crate::replay::{ReplayCapture, ReplayRecord};
use crate::tools::{CapabilityPolicy, ToolContext, ToolRuntime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};

/// Configuration for an evaluation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRunConfig {
    /// Timeout for the entire evaluation
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Whether to capture replay events
    pub capture_replay: bool,

    /// Whether to fail fast on first assertion failure
    pub fail_fast: bool,

    /// Policy for tool execution
    pub capability_policy: CapabilityPolicyConfig,
}

impl Default for EvalRunConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            capture_replay: true,
            fail_fast: false,
            capability_policy: CapabilityPolicyConfig::AllowAll,
        }
    }
}

/// Capability policy configuration for eval
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityPolicyConfig {
    /// Allow all capabilities
    #[default]
    AllowAll,
    /// Deny all capabilities
    DenyAll,
    /// Safe only (non-privileged)
    SafeOnly,
}

impl CapabilityPolicyConfig {
    fn to_policy(&self) -> CapabilityPolicy {
        match self {
            CapabilityPolicyConfig::AllowAll => CapabilityPolicy::allow_all(),
            CapabilityPolicyConfig::DenyAll => CapabilityPolicy::deny_all(),
            CapabilityPolicyConfig::SafeOnly => CapabilityPolicy::safe_only(),
        }
    }
}

/// Result of an evaluation run
#[derive(Debug, Clone)]
pub struct EvalResult {
    /// Fixture that was evaluated
    pub fixture_name: String,

    /// Whether evaluation passed
    pub passed: bool,

    /// Assertion failures (if any)
    pub failures: Vec<AssertionFailure>,

    /// Actual output
    pub output: Option<Value>,

    /// Tool calls that were made
    pub tool_calls: Vec<RecordedToolCall>,

    /// Execution duration
    pub duration: Duration,

    /// Replay record (if captured)
    pub replay: Option<ReplayRecord>,

    /// Error (if evaluation itself failed)
    pub error: Option<String>,
}

impl EvalResult {
    /// Check if evaluation passed
    pub fn passed(&self) -> bool {
        self.passed
    }

    /// Get failure messages
    pub fn failure_messages(&self) -> Vec<&str> {
        self.failures.iter().map(|f| f.message.as_str()).collect()
    }
}

/// An assertion failure
#[derive(Debug, Clone)]
pub struct AssertionFailure {
    /// What was being checked
    pub check: String,

    /// Failure message
    pub message: String,

    /// Expected value
    pub expected: Option<Value>,

    /// Actual value
    pub actual: Option<Value>,
}

impl AssertionFailure {
    fn new(check: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            check: check.into(),
            message: message.into(),
            expected: None,
            actual: None,
        }
    }

    fn with_values(mut self, expected: Value, actual: Value) -> Self {
        self.expected = Some(expected);
        self.actual = Some(actual);
        self
    }
}

/// Recorded tool call during evaluation
#[derive(Debug, Clone)]
pub struct RecordedToolCall {
    /// Tool name
    pub tool_name: String,

    /// Arguments passed
    pub args: Value,

    /// Whether call succeeded
    pub success: bool,

    /// Result (if successful)
    pub result: Option<Value>,
}

/// Evaluation harness
pub struct EvalHarness {
    config: EvalRunConfig,
    stubs: StubRegistry,
    runtime: ToolRuntime,
}

impl EvalHarness {
    /// Create a new harness with default configuration
    pub fn new() -> Self {
        Self::with_config(EvalRunConfig::default())
    }

    /// Create a harness with custom configuration
    pub fn with_config(config: EvalRunConfig) -> Self {
        let runtime = ToolRuntime::new(config.capability_policy.to_policy());
        Self {
            config,
            stubs: StubRegistry::new(),
            runtime,
        }
    }

    /// Register a stub tool
    pub fn with_stub(mut self, stub: StubTool) -> Self {
        self.stubs.register(stub);
        self
    }

    /// Register multiple stub tools
    pub fn with_stubs(mut self, stubs: impl IntoIterator<Item = StubTool>) -> Self {
        for stub in stubs {
            self.stubs.register(stub);
        }
        self
    }

    /// Set up stubs from fixture
    pub fn with_fixture_stubs(mut self, fixture: &Fixture) -> Self {
        self.stubs = StubRegistry::from_fixture_stubs(&fixture.tool_stubs);
        self
    }

    /// Run a fixture and return the result
    pub async fn run(&self, fixture: &Fixture) -> EvalResult {
        let start = Instant::now();
        let mut failures = Vec::new();
        let mut tool_calls = Vec::new();
        let mut output: Option<Value> = None;

        // Set up replay capture
        let capture = if self.config.capture_replay {
            Some(ReplayCapture::new(format!("eval_{}", fixture.name)))
        } else {
            None
        };

        if let Some(ref cap) = capture {
            cap.start_session().await;
        }

        // Create tool context
        let ctx = ToolContext::new()
            .with_agent_id(fixture.input.agent_id.clone().unwrap_or_else(|| "eval_agent".to_string()));

        // Execute all registered stubs with the input args
        // In a real scenario, this would be a workflow execution
        // For now, we simulate by calling each stubbed tool once
        for tool_name in self.stubs.tool_names() {
            if let Some(stub) = self.stubs.get(tool_name) {
                let args = fixture.input.args.clone();
                let result = self.runtime.execute(stub.as_ref(), args.clone(), &ctx).await;

                let success = result.is_success();
                let result_value = result.value().cloned();

                tool_calls.push(RecordedToolCall {
                    tool_name: tool_name.clone(),
                    args,
                    success,
                    result: result_value.clone(),
                });

                // If we got an output, use the last successful one
                if let Some(val) = result_value {
                    output = Some(val);
                }

                // Record to replay if configured
                if let Some(ref cap) = capture {
                    cap.record_tool_call(tool_name, fixture.input.args.clone(), &result).await;
                }
            }
        }

        // Validate against expectations
        self.validate_expectations(&fixture.expected, &output, &tool_calls, &mut failures);

        // Check duration
        let duration = start.elapsed();
        if fixture
            .expected
            .max_duration_ms
            .is_some_and(|max_ms| duration.as_millis() > max_ms as u128)
        {
            let max_ms = fixture.expected.max_duration_ms.unwrap();
            failures.push(AssertionFailure::new(
                "max_duration_ms",
                format!(
                    "Execution took {}ms, expected max {}ms",
                    duration.as_millis(),
                    max_ms
                ),
            ));
        }

        // Get replay record
        let replay = if let Some(cap) = capture {
            Some(cap.end_session().await)
        } else {
            None
        };

        EvalResult {
            fixture_name: fixture.name.clone(),
            passed: failures.is_empty(),
            failures,
            output,
            tool_calls,
            duration,
            replay,
            error: None,
        }
    }

    /// Run multiple fixtures
    pub async fn run_all(&self, fixtures: &[Fixture]) -> Vec<EvalResult> {
        let mut results = Vec::new();
        for fixture in fixtures {
            // Reset stubs between runs
            self.stubs.reset_all().await;
            results.push(self.run(fixture).await);
        }
        results
    }

    /// Validate expectations against actual results
    fn validate_expectations(
        &self,
        expected: &FixtureExpectation,
        output: &Option<Value>,
        tool_calls: &[RecordedToolCall],
        failures: &mut Vec<AssertionFailure>,
    ) {
        // Check output exact match
        if let Some(ref expected_output) = expected.output {
            match output {
                Some(actual) if actual == expected_output => {}
                Some(actual) => {
                    failures.push(
                        AssertionFailure::new("output", "Output mismatch")
                            .with_values(expected_output.clone(), actual.clone()),
                    );
                }
                None => {
                    failures.push(AssertionFailure::new("output", "Expected output but got none"));
                }
            }
        }

        // Check output contains patterns
        for pattern in &expected.output_contains {
            let output_str = output
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default())
                .unwrap_or_default();

            if !output_str.contains(pattern) {
                failures.push(AssertionFailure::new(
                    "output_contains",
                    format!("Output does not contain expected pattern: '{}'", pattern),
                ));
            }
        }

        // Check tool call count bounds
        if expected
            .min_tool_calls
            .is_some_and(|min| tool_calls.len() < min)
        {
            let min = expected.min_tool_calls.unwrap();
            failures.push(AssertionFailure::new(
                "min_tool_calls",
                format!(
                    "Expected at least {} tool calls, got {}",
                    min,
                    tool_calls.len()
                ),
            ));
        }

        if expected
            .max_tool_calls
            .is_some_and(|max| tool_calls.len() > max)
        {
            let max = expected.max_tool_calls.unwrap();
            failures.push(AssertionFailure::new(
                "max_tool_calls",
                format!(
                    "Expected at most {} tool calls, got {}",
                    max,
                    tool_calls.len()
                ),
            ));
        }

        // Check expected tool calls
        for expected_call in &expected.tool_calls {
            self.validate_tool_call(expected_call, tool_calls, failures);
        }

        // Check JSON path assertions
        for check in &expected.json_path_checks {
            self.validate_json_path(check, output, failures);
        }

        // Check success/failure expectation
        let any_tool_failed = tool_calls.iter().any(|c| !c.success);
        if expected.should_succeed && any_tool_failed {
            failures.push(AssertionFailure::new(
                "should_succeed",
                "Expected success but tool call(s) failed",
            ));
        }
        if !expected.should_succeed && !any_tool_failed && !tool_calls.is_empty() {
            failures.push(AssertionFailure::new(
                "should_succeed",
                "Expected failure but all tool calls succeeded",
            ));
        }
    }

    fn validate_tool_call(
        &self,
        expected: &ExpectedToolCall,
        actual_calls: &[RecordedToolCall],
        failures: &mut Vec<AssertionFailure>,
    ) {
        let matching_call = actual_calls.iter().find(|c| c.tool_name == expected.tool_name);

        match matching_call {
            Some(call) => {
                // Check args if specified
                if expected
                    .args
                    .as_ref()
                    .is_some_and(|expected_args| !self.values_match(expected_args, &call.args))
                {
                    let expected_args = expected.args.as_ref().unwrap();
                    failures.push(
                        AssertionFailure::new(
                            format!("tool_call[{}].args", expected.tool_name),
                            "Arguments mismatch",
                        )
                        .with_values(expected_args.clone(), call.args.clone()),
                    );
                }

                // Check success
                if expected.should_succeed != call.success {
                    failures.push(AssertionFailure::new(
                        format!("tool_call[{}].success", expected.tool_name),
                        format!(
                            "Expected tool to {}, but it {}",
                            if expected.should_succeed { "succeed" } else { "fail" },
                            if call.success { "succeeded" } else { "failed" }
                        ),
                    ));
                }
            }
            None => {
                failures.push(AssertionFailure::new(
                    format!("tool_call[{}]", expected.tool_name),
                    format!("Expected call to tool '{}' not found", expected.tool_name),
                ));
            }
        }
    }

    fn validate_json_path(
        &self,
        check: &JsonPathCheck,
        output: &Option<Value>,
        failures: &mut Vec<AssertionFailure>,
    ) {
        let value = output.as_ref().and_then(|v| self.get_json_path(v, &check.path));

        match check.operator {
            CheckOperator::Equals => {
                if value != Some(&check.value) {
                    failures.push(
                        AssertionFailure::new(
                            format!("json_path[{}]", check.path),
                            "Value mismatch",
                        )
                        .with_values(
                            check.value.clone(),
                            value.cloned().unwrap_or(Value::Null),
                        ),
                    );
                }
            }
            CheckOperator::NotEquals => {
                if value == Some(&check.value) {
                    failures.push(AssertionFailure::new(
                        format!("json_path[{}]", check.path),
                        "Value should not equal expected",
                    ));
                }
            }
            CheckOperator::Contains => {
                let contains = match (value, &check.value) {
                    (Some(Value::String(s)), Value::String(pattern)) => s.contains(pattern.as_str()),
                    (Some(Value::Array(arr)), val) => arr.contains(val),
                    _ => false,
                };
                if !contains {
                    failures.push(AssertionFailure::new(
                        format!("json_path[{}]", check.path),
                        "Value does not contain expected",
                    ));
                }
            }
            CheckOperator::GreaterThan => {
                let passes = match (value, &check.value) {
                    (Some(Value::Number(n1)), Value::Number(n2)) => {
                        n1.as_f64().unwrap_or(0.0) > n2.as_f64().unwrap_or(0.0)
                    }
                    _ => false,
                };
                if !passes {
                    failures.push(AssertionFailure::new(
                        format!("json_path[{}]", check.path),
                        "Value not greater than expected",
                    ));
                }
            }
            CheckOperator::LessThan => {
                let passes = match (value, &check.value) {
                    (Some(Value::Number(n1)), Value::Number(n2)) => {
                        n1.as_f64().unwrap_or(0.0) < n2.as_f64().unwrap_or(0.0)
                    }
                    _ => false,
                };
                if !passes {
                    failures.push(AssertionFailure::new(
                        format!("json_path[{}]", check.path),
                        "Value not less than expected",
                    ));
                }
            }
            CheckOperator::Exists => {
                if value.is_none() || value == Some(&Value::Null) {
                    failures.push(AssertionFailure::new(
                        format!("json_path[{}]", check.path),
                        "Value does not exist",
                    ));
                }
            }
        }
    }

    /// Simple JSON path getter (supports dot notation only)
    fn get_json_path<'a>(&self, value: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = value;
        for key in path.split('.') {
            match current {
                Value::Object(obj) => {
                    current = obj.get(key)?;
                }
                Value::Array(arr) => {
                    let idx: usize = key.parse().ok()?;
                    current = arr.get(idx)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Check if expected value matches actual (partial match for objects)
    #[allow(clippy::only_used_in_recursion)]
    fn values_match(&self, expected: &Value, actual: &Value) -> bool {
        match (expected, actual) {
            (Value::Object(exp_obj), Value::Object(act_obj)) => {
                // Expected is a subset of actual
                exp_obj.iter().all(|(k, v)| {
                    act_obj.get(k).map(|av| self.values_match(v, av)).unwrap_or(false)
                })
            }
            (Value::Array(exp_arr), Value::Array(act_arr)) => {
                exp_arr.len() == act_arr.len()
                    && exp_arr
                        .iter()
                        .zip(act_arr.iter())
                        .all(|(e, a)| self.values_match(e, a))
            }
            _ => expected == actual,
        }
    }
}

impl Default for EvalHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod harness_tests {
    use super::*;
    use crate::eval::fixture::{FixtureInput, StubResponseDef, StubResponseValue};
    use crate::eval::stub::StubResponse;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_harness_simple_pass() {
        let fixture = Fixture {
            name: "simple_pass".to_string(),
            description: None,
            schema_version: 1,
            tags: vec![],
            input: FixtureInput::default(),
            expected: FixtureExpectation {
                output: Some(serde_json::json!({"result": 42})),
                should_succeed: true,
                ..Default::default()
            },
            tool_stubs: HashMap::new(),
            memory_snapshot: None,
        };

        let harness = EvalHarness::new()
            .with_stub(StubTool::new("test", StubResponse::success(serde_json::json!({"result": 42}))));

        let result = harness.run(&fixture).await;

        assert!(result.passed());
        assert!(result.failures.is_empty());
    }

    #[tokio::test]
    async fn test_harness_output_mismatch() {
        let fixture = Fixture {
            name: "output_mismatch".to_string(),
            description: None,
            schema_version: 1,
            tags: vec![],
            input: FixtureInput::default(),
            expected: FixtureExpectation {
                output: Some(serde_json::json!({"result": 42})),
                should_succeed: true,
                ..Default::default()
            },
            tool_stubs: HashMap::new(),
            memory_snapshot: None,
        };

        let harness = EvalHarness::new()
            .with_stub(StubTool::new("test", StubResponse::success(serde_json::json!({"result": 99}))));

        let result = harness.run(&fixture).await;

        assert!(!result.passed());
        assert!(!result.failures.is_empty());
        assert!(result.failures[0].check.contains("output"));
    }

    #[tokio::test]
    async fn test_harness_tool_call_validation() {
        let mut stubs = HashMap::new();
        stubs.insert(
            "search".to_string(),
            vec![StubResponseDef {
                call_index: None,
                match_args: None,
                response: StubResponseValue::Success(serde_json::json!({"results": []})),
                delay_ms: 0,
            }],
        );

        let fixture = Fixture {
            name: "tool_call_check".to_string(),
            description: None,
            schema_version: 1,
            tags: vec![],
            input: FixtureInput::default(),
            expected: FixtureExpectation {
                tool_calls: vec![crate::eval::fixture::ExpectedToolCall::new("search")],
                should_succeed: true,
                ..Default::default()
            },
            tool_stubs: stubs,
            memory_snapshot: None,
        };

        let harness = EvalHarness::new().with_fixture_stubs(&fixture);
        let result = harness.run(&fixture).await;

        assert!(result.passed());
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].tool_name, "search");
    }

    #[tokio::test]
    async fn test_harness_missing_tool_call() {
        let fixture = Fixture {
            name: "missing_tool".to_string(),
            description: None,
            schema_version: 1,
            tags: vec![],
            input: FixtureInput::default(),
            expected: FixtureExpectation {
                tool_calls: vec![crate::eval::fixture::ExpectedToolCall::new("expected_tool")],
                should_succeed: true,
                ..Default::default()
            },
            tool_stubs: HashMap::new(),
            memory_snapshot: None,
        };

        let harness = EvalHarness::new()
            .with_stub(StubTool::new("other_tool", StubResponse::success(Value::Null)));

        let result = harness.run(&fixture).await;

        assert!(!result.passed());
        assert!(result.failures.iter().any(|f| f.message.contains("expected_tool")));
    }

    #[tokio::test]
    async fn test_harness_with_replay() {
        let fixture = Fixture {
            name: "replay_test".to_string(),
            description: None,
            schema_version: 1,
            tags: vec![],
            input: FixtureInput::default(),
            expected: FixtureExpectation::default(),
            tool_stubs: HashMap::new(),
            memory_snapshot: None,
        };

        let config = EvalRunConfig {
            capture_replay: true,
            ..Default::default()
        };

        let harness = EvalHarness::with_config(config)
            .with_stub(StubTool::new("test", StubResponse::success(Value::Null)));

        let result = harness.run(&fixture).await;

        assert!(result.replay.is_some());
        let replay = result.replay.unwrap();
        assert!(replay.events.len() >= 2); // Start + End at minimum
    }
}

