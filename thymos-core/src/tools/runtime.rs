//! Tool runtime with safety enforcement
//!
//! The runtime wraps tool execution with:
//! - Capability policy enforcement
//! - Timeout and cancellation
//! - Rate limiting
//! - Concurrency control

use super::capability::CapabilityPolicy;
use super::result::{PolicyDecision, ToolError, ToolErrorKind, ToolProvenance, ToolResultEnvelope};
use super::tool::{Tool, ToolExecutionContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;

/// Context for tool execution (passed through runtime)
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Agent ID making the call
    pub agent_id: Option<String>,

    /// Trace ID for correlation
    pub trace_id: Option<String>,

    /// Whether to redact secrets from output
    pub redact_secrets: bool,

    /// Cancellation token
    pub cancellation: Option<tokio_util::sync::CancellationToken>,

    /// Additional context values
    pub extra: Value,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            agent_id: None,
            trace_id: None,
            redact_secrets: true,
            cancellation: None,
            extra: Value::Null,
        }
    }
}

impl ToolContext {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set agent ID
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set trace ID
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Set redaction policy
    pub fn with_redact_secrets(mut self, redact: bool) -> Self {
        self.redact_secrets = redact;
        self
    }

    /// Set cancellation token
    pub fn with_cancellation(mut self, token: tokio_util::sync::CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Add extra context
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    /// Convert to tool execution context
    pub fn to_execution_context(&self) -> ToolExecutionContext {
        ToolExecutionContext {
            agent_id: self.agent_id.clone(),
            trace_id: self.trace_id.clone(),
            redact_secrets: self.redact_secrets,
            extra: self.extra.clone(),
        }
    }
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u64,

    /// Window duration
    #[serde(with = "humantime_serde")]
    pub window: Duration,

    /// Maximum concurrent executions
    pub max_concurrent: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            max_concurrent: 10,
        }
    }
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRuntimeConfig {
    /// Default timeout for tool execution
    #[serde(with = "humantime_serde")]
    pub default_timeout: Duration,

    /// Per-tool timeout overrides
    #[serde(default)]
    pub tool_timeouts: HashMap<String, Duration>,

    /// Global rate limit
    pub rate_limit: Option<RateLimitConfig>,

    /// Per-tool rate limits
    #[serde(default)]
    pub tool_rate_limits: HashMap<String, RateLimitConfig>,
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            tool_timeouts: HashMap::new(),
            rate_limit: None,
            tool_rate_limits: HashMap::new(),
        }
    }
}

impl ToolRuntimeConfig {
    /// Create a config with a specific timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Add a tool-specific timeout
    pub fn with_tool_timeout(mut self, tool_name: impl Into<String>, timeout: Duration) -> Self {
        self.tool_timeouts.insert(tool_name.into(), timeout);
        self
    }

    /// Set global rate limit
    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit = Some(config);
        self
    }
}

/// Rate limiter state
struct RateLimiterState {
    requests: AtomicU64,
    window_start: RwLock<Instant>,
    config: RateLimitConfig,
    semaphore: Semaphore,
}

impl RateLimiterState {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            requests: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
            semaphore: Semaphore::new(config.max_concurrent),
            config,
        }
    }

    async fn check_and_acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>, Duration> {
        // Check window reset
        {
            let mut window_start = self.window_start.write().await;
            if window_start.elapsed() >= self.config.window {
                self.requests.store(0, Ordering::SeqCst);
                *window_start = Instant::now();
            }
        }

        // Check rate limit
        let current = self.requests.fetch_add(1, Ordering::SeqCst);
        if current >= self.config.max_requests {
            let window_start = self.window_start.read().await;
            let remaining = self.config.window.saturating_sub(window_start.elapsed());
            return Err(remaining);
        }

        // Acquire semaphore permit for concurrency control
        match self.semaphore.try_acquire() {
            Ok(permit) => Ok(permit),
            Err(_) => {
                // Semaphore full, estimate wait time
                Err(Duration::from_millis(100))
            }
        }
    }
}

/// Tool runtime with safety enforcement
pub struct ToolRuntime {
    policy: CapabilityPolicy,
    config: ToolRuntimeConfig,
    global_rate_limiter: Option<RateLimiterState>,
    tool_rate_limiters: RwLock<HashMap<String, Arc<RateLimiterState>>>,
}

impl ToolRuntime {
    /// Create a new runtime with the given policy
    pub fn new(policy: CapabilityPolicy) -> Self {
        Self::with_config(policy, ToolRuntimeConfig::default())
    }

    /// Create a runtime with custom configuration
    pub fn with_config(policy: CapabilityPolicy, config: ToolRuntimeConfig) -> Self {
        let global_rate_limiter = config.rate_limit.clone().map(RateLimiterState::new);

        Self {
            policy,
            config,
            global_rate_limiter,
            tool_rate_limiters: RwLock::new(HashMap::new()),
        }
    }

    /// Get the capability policy
    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    /// Get the runtime config
    pub fn config(&self) -> &ToolRuntimeConfig {
        &self.config
    }

    /// Get timeout for a specific tool
    fn get_timeout(&self, tool_name: &str) -> Duration {
        self.config
            .tool_timeouts
            .get(tool_name)
            .copied()
            .unwrap_or(self.config.default_timeout)
    }

    /// Get or create rate limiter for a tool
    async fn get_tool_rate_limiter(&self, tool_name: &str) -> Option<Arc<RateLimiterState>> {
        if let Some(config) = self.config.tool_rate_limits.get(tool_name) {
            let mut limiters = self.tool_rate_limiters.write().await;
            let limiter = limiters
                .entry(tool_name.to_string())
                .or_insert_with(|| Arc::new(RateLimiterState::new(config.clone())));
            Some(limiter.clone())
        } else {
            None
        }
    }

    /// Execute a tool with all safety checks
    pub async fn execute(
        &self,
        tool: &dyn Tool,
        args: Value,
        ctx: &ToolContext,
    ) -> ToolResultEnvelope {
        let started_at = chrono::Utc::now();
        let tool_name = tool.name().to_string();

        // Compute args hash for provenance
        let args_json = serde_json::to_string(&args).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(args_json.as_bytes());
        let args_hash = format!("{:x}", hasher.finalize());

        // Build provenance
        let mut provenance = ToolProvenance::new(&tool_name, &args_hash[..16]);
        if let Some(ref agent_id) = ctx.agent_id {
            provenance = provenance.with_agent_id(agent_id);
        }
        if let Some(ref trace_id) = ctx.trace_id {
            provenance = provenance.with_trace_id(trace_id);
        }

        // Check cancellation first
        if ctx.is_cancelled() {
            provenance = provenance.with_duration(Duration::ZERO);
            return ToolResultEnvelope::cancelled("Cancelled before execution".to_string(), provenance);
        }

        // Step 1: Check capability policy
        let required = tool.required_capabilities();
        match self.policy.check_all(&required) {
            Ok(()) => {
                provenance = provenance.with_policy_decision(
                    PolicyDecision::new("capability_check", true)
                        .with_reason("All required capabilities allowed"),
                );
            }
            Err(denied) => {
                let denied_caps: Vec<_> = denied.iter().copied().collect();
                provenance = provenance.with_policy_decision(
                    PolicyDecision::new("capability_check", false)
                        .with_reason(format!("Denied capabilities: {:?}", denied_caps)),
                );
                let error = ToolError::capability_denied(&denied_caps);
                return ToolResultEnvelope::error(error, provenance);
            }
        }

        // Step 2: Check rate limits
        // Global rate limit
        if let Some(ref limiter) = self.global_rate_limiter {
            match limiter.check_and_acquire().await {
                Ok(_permit) => {
                    provenance = provenance.with_policy_decision(
                        PolicyDecision::new("global_rate_limit", true),
                    );
                }
                Err(retry_after) => {
                    provenance = provenance.with_policy_decision(
                        PolicyDecision::new("global_rate_limit", false)
                            .with_reason("Rate limit exceeded"),
                    );
                    let error = ToolError::rate_limited(retry_after);
                    return ToolResultEnvelope::error(error, provenance);
                }
            }
        }

        // Per-tool rate limit
        if let Some(limiter) = self.get_tool_rate_limiter(&tool_name).await {
            match limiter.check_and_acquire().await {
                Ok(_permit) => {
                    provenance = provenance.with_policy_decision(
                        PolicyDecision::new("tool_rate_limit", true),
                    );
                }
                Err(retry_after) => {
                    provenance = provenance.with_policy_decision(
                        PolicyDecision::new("tool_rate_limit", false)
                            .with_reason("Per-tool rate limit exceeded"),
                    );
                    let error = ToolError::rate_limited(retry_after);
                    return ToolResultEnvelope::error(error, provenance);
                }
            }
        }

        // Step 3: Validate arguments
        if let Err(validation_errors) = tool.validate(&args) {
            let error = ToolError::validation(validation_errors);
            let duration = (chrono::Utc::now() - started_at)
                .to_std()
                .unwrap_or_default();
            provenance = provenance.with_duration(duration);
            return ToolResultEnvelope::error(error, provenance);
        }

        // Step 4: Execute with timeout and cancellation
        let tool_timeout = self.get_timeout(&tool_name);
        let exec_ctx = ctx.to_execution_context();

        let execution = async {
            tool.execute(args, &exec_ctx).await
        };

        // Combine timeout with cancellation
        let result = if let Some(ref cancel_token) = ctx.cancellation {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    Err(ToolError::new(ToolErrorKind::Cancelled, "Execution cancelled"))
                }
                res = timeout(tool_timeout, execution) => {
                    match res {
                        Ok(Ok(envelope)) => Ok(envelope),
                        Ok(Err(error)) => Err(error),
                        Err(_) => Err(ToolError::timeout(tool_timeout)),
                    }
                }
            }
        } else {
            match timeout(tool_timeout, execution).await {
                Ok(Ok(envelope)) => Ok(envelope),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(ToolError::timeout(tool_timeout)),
            }
        };

        // Finalize provenance
        let duration = (chrono::Utc::now() - started_at)
            .to_std()
            .unwrap_or_default();
        provenance = provenance.with_duration(duration);

        match result {
            Ok(mut envelope) => {
                // Merge our provenance with the tool's provenance
                // Runtime-level provenance takes precedence (we have the full context)
                envelope.provenance.policy_decisions.extend(provenance.policy_decisions);
                envelope.provenance.started_at = provenance.started_at;
                envelope.provenance.duration = duration;
                envelope.provenance.agent_id = provenance.agent_id;
                envelope.provenance.trace_id = provenance.trace_id;
                envelope
            }
            Err(error) => ToolResultEnvelope::error(error, provenance),
        }
    }

    /// Execute a tool and return just the value on success
    pub async fn execute_simple(
        &self,
        tool: &dyn Tool,
        args: Value,
        ctx: &ToolContext,
    ) -> Result<Value, ToolError> {
        let envelope = self.execute(tool, args, ctx).await;
        match envelope.result {
            crate::tools::ToolResult::Success { value } => Ok(value),
            crate::tools::ToolResult::Error { error } => Err(error),
            crate::tools::ToolResult::Cancelled { reason } => {
                Err(ToolError::new(ToolErrorKind::Cancelled, reason))
            }
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use crate::tools::{Capability, CapabilitySet, Tool, ToolMetadata, ToolSchema};
    use async_trait::async_trait;

    struct SlowTool {
        metadata: ToolMetadata,
        delay: Duration,
    }

    impl SlowTool {
        fn new(delay: Duration) -> Self {
            Self {
                metadata: ToolMetadata::new("slow_tool", "A deliberately slow tool"),
                delay,
            }
        }
    }

    #[async_trait]
    impl Tool for SlowTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::empty()
        }

        async fn execute(
            &self,
            _args: Value,
            ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, ToolError> {
            tokio::time::sleep(self.delay).await;
            let provenance = ToolProvenance::new("slow_tool", "test")
                .with_agent_id(ctx.agent_id.clone().unwrap_or_default());
            Ok(ToolResultEnvelope::success(
                serde_json::json!({"completed": true}),
                provenance,
            ))
        }
    }

    struct PrivilegedTool {
        metadata: ToolMetadata,
    }

    impl PrivilegedTool {
        fn new() -> Self {
            Self {
                metadata: ToolMetadata::new("privileged_tool", "Requires network access"),
            }
        }
    }

    #[async_trait]
    impl Tool for PrivilegedTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        fn schema(&self) -> ToolSchema {
            ToolSchema::empty()
        }

        fn required_capabilities(&self) -> CapabilitySet {
            CapabilitySet::from_capabilities([Capability::Network])
        }

        async fn execute(
            &self,
            _args: Value,
            ctx: &ToolExecutionContext,
        ) -> Result<ToolResultEnvelope, ToolError> {
            let provenance = ToolProvenance::new("privileged_tool", "test")
                .with_agent_id(ctx.agent_id.clone().unwrap_or_default());
            Ok(ToolResultEnvelope::success(
                serde_json::json!({"network_accessed": true}),
                provenance,
            ))
        }
    }

    #[tokio::test]
    async fn test_deny_by_default() {
        let runtime = ToolRuntime::new(CapabilityPolicy::deny_all());
        let tool = PrivilegedTool::new();
        let ctx = ToolContext::new();

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_error());
        let error = result.get_error().unwrap();
        assert_eq!(error.kind, ToolErrorKind::CapabilityDenied);
    }

    #[tokio::test]
    async fn test_allowed_capability() {
        let policy = CapabilityPolicy::deny_all().allow(Capability::Network);
        let runtime = ToolRuntime::new(policy);
        let tool = PrivilegedTool::new();
        let ctx = ToolContext::new();

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_timeout_enforcement() {
        let config = ToolRuntimeConfig::default().with_timeout(Duration::from_millis(50));
        let runtime = ToolRuntime::with_config(CapabilityPolicy::allow_all(), config);
        let tool = SlowTool::new(Duration::from_secs(5));
        let ctx = ToolContext::new();

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_error());
        let error = result.get_error().unwrap();
        assert_eq!(error.kind, ToolErrorKind::Timeout);
    }

    #[tokio::test]
    async fn test_cancellation() {
        let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
        let tool = SlowTool::new(Duration::from_secs(5));
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let ctx = ToolContext::new().with_cancellation(cancel_token.clone());

        // Cancel immediately
        cancel_token.cancel();

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_cancelled());
    }

    #[tokio::test]
    async fn test_provenance_tracking() {
        let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
        let tool = SlowTool::new(Duration::from_millis(10));
        let ctx = ToolContext::new()
            .with_agent_id("test_agent")
            .with_trace_id("trace_123");

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_success());
        assert_eq!(result.provenance.agent_id, Some("test_agent".to_string()));
        assert_eq!(result.provenance.trace_id, Some("trace_123".to_string()));
        assert!(result.provenance.duration > Duration::ZERO);
        assert!(!result.provenance.policy_decisions.is_empty());
    }

    #[tokio::test]
    async fn test_successful_execution() {
        let runtime = ToolRuntime::new(CapabilityPolicy::allow_all());
        let tool = SlowTool::new(Duration::from_millis(1));
        let ctx = ToolContext::new();

        let result = runtime.execute(&tool, Value::Null, &ctx).await;

        assert!(result.is_success());
        assert_eq!(
            result.value().unwrap(),
            &serde_json::json!({"completed": true})
        );
    }
}

