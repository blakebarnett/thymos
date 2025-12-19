//! Router Workflow Pattern
//!
//! Routes input to specialized handlers based on classification.

use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::LLMProvider;

use super::chain::Chain;
use super::classifier::{Classification, Classifier};
use super::execution::{ExecutionTrace, StepTrace, WorkflowError, WorkflowResult};
use super::step::{Step, StepOutput};

/// Handler for a route - can be a single step or a chain
pub enum RouteHandler {
    /// Single step handler
    Step(Step),
    /// Chain handler
    Chain(Chain),
    /// Custom async handler
    Custom(Arc<dyn Fn(serde_json::Value, &dyn LLMProvider) -> std::pin::Pin<Box<dyn std::future::Future<Output = WorkflowResult<StepOutput>> + Send>> + Send + Sync>),
}

impl std::fmt::Debug for RouteHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteHandler::Step(step) => f.debug_tuple("Step").field(step).finish(),
            RouteHandler::Chain(chain) => f.debug_tuple("Chain").field(chain).finish(),
            RouteHandler::Custom(_) => f.debug_tuple("Custom").finish(),
        }
    }
}

impl RouteHandler {
    /// Execute the handler
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, Option<ExecutionTrace>)> {
        match self {
            RouteHandler::Step(step) => {
                let (output, trace) = step.execute(input, provider).await?;
                let mut exec_trace = ExecutionTrace::new("route_step");
                exec_trace.add_step(trace);
                Ok((output, Some(exec_trace)))
            }
            RouteHandler::Chain(chain) => {
                let (output, trace) = chain.execute(input, provider).await?;
                Ok((output, Some(trace)))
            }
            RouteHandler::Custom(handler) => {
                let output = handler(input, provider).await?;
                Ok((output, None))
            }
        }
    }
}

/// A route definition
pub struct Route {
    /// Route label (matches classification label)
    pub label: String,
    /// Route handler
    pub handler: RouteHandler,
    /// Minimum confidence required for this route
    pub min_confidence: f32,
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("label", &self.label)
            .field("min_confidence", &self.min_confidence)
            .finish()
    }
}

impl Route {
    /// Create a new route with a step handler
    pub fn step(label: impl Into<String>, step: Step) -> Self {
        Self {
            label: label.into(),
            handler: RouteHandler::Step(step),
            min_confidence: 0.0,
        }
    }

    /// Create a new route with a chain handler
    pub fn chain(label: impl Into<String>, chain: Chain) -> Self {
        Self {
            label: label.into(),
            handler: RouteHandler::Chain(chain),
            min_confidence: 0.0,
        }
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Router workflow that directs input to handlers based on classification
pub struct Router {
    /// Router name
    name: String,
    /// Routes by label
    routes: HashMap<String, Route>,
    /// Default/fallback route
    fallback: Option<Route>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("name", &self.name)
            .field("route_count", &self.routes.len())
            .field("has_fallback", &self.fallback.is_some())
            .finish()
    }
}

impl Router {
    /// Create a new router builder
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    /// Get the router name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of routes
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Route and execute with a classifier
    pub async fn execute<C: Classifier>(
        &self,
        input: serde_json::Value,
        classifier: &C,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, RouterExecutionTrace)> {
        let start = std::time::Instant::now();

        // Classify the input
        let classification = classifier.classify(&input).await?;

        let classification_ms = start.elapsed().as_millis() as u64;

        // Find matching route
        let (selected_label, route) = self.find_route(&classification)?;

        // Execute the handler
        let (output, handler_trace) = route.handler.execute(input.clone(), provider).await?;

        let total_ms = start.elapsed().as_millis() as u64;

        let trace = RouterExecutionTrace {
            router_name: self.name.clone(),
            classification: classification.clone(),
            selected_route: selected_label,
            classification_duration_ms: classification_ms,
            handler_trace,
            total_duration_ms: total_ms,
            success: true,
        };

        Ok((output, trace))
    }

    /// Execute with a pre-determined classification (skip classifier)
    pub async fn execute_with_label(
        &self,
        input: serde_json::Value,
        label: &str,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, Option<ExecutionTrace>)> {
        let route = self
            .routes
            .get(label)
            .or(self.fallback.as_ref())
            .ok_or_else(|| WorkflowError::InvalidConfig(format!("No route for label: {}", label)))?;

        route.handler.execute(input, provider).await
    }

    fn find_route(&self, classification: &Classification) -> WorkflowResult<(String, &Route)> {
        // Try to find exact match with sufficient confidence
        if let Some(route) = self.routes.get(&classification.label) {
            if classification.confidence >= route.min_confidence {
                return Ok((classification.label.clone(), route));
            }
        }

        // Fall back to default route
        if let Some(fallback) = &self.fallback {
            return Ok(("fallback".to_string(), fallback));
        }

        Err(WorkflowError::InvalidConfig(format!(
            "No route found for classification '{}' (confidence: {})",
            classification.label, classification.confidence
        )))
    }
}

/// Router execution trace
#[derive(Debug, Clone)]
pub struct RouterExecutionTrace {
    /// Router name
    pub router_name: String,
    /// Classification result
    pub classification: Classification,
    /// Selected route label
    pub selected_route: String,
    /// Classification duration
    pub classification_duration_ms: u64,
    /// Handler execution trace
    pub handler_trace: Option<ExecutionTrace>,
    /// Total duration
    pub total_duration_ms: u64,
    /// Success
    pub success: bool,
}

/// Builder for Router
pub struct RouterBuilder {
    name: String,
    routes: HashMap<String, Route>,
    fallback: Option<Route>,
}

impl RouterBuilder {
    /// Create a new router builder
    pub fn new() -> Self {
        Self {
            name: "router".to_string(),
            routes: HashMap::new(),
            fallback: None,
        }
    }

    /// Set the router name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a route
    pub fn route(mut self, route: Route) -> Self {
        self.routes.insert(route.label.clone(), route);
        self
    }

    /// Add a step route
    pub fn step_route(mut self, label: impl Into<String>, step: Step) -> Self {
        let route = Route::step(label, step);
        self.routes.insert(route.label.clone(), route);
        self
    }

    /// Add a chain route
    pub fn chain_route(mut self, label: impl Into<String>, chain: Chain) -> Self {
        let route = Route::chain(label, chain);
        self.routes.insert(route.label.clone(), route);
        self
    }

    /// Set the fallback route (step)
    pub fn fallback_step(mut self, step: Step) -> Self {
        self.fallback = Some(Route::step("fallback", step));
        self
    }

    /// Set the fallback route (chain)
    pub fn fallback_chain(mut self, chain: Chain) -> Self {
        self.fallback = Some(Route::chain("fallback", chain));
        self
    }

    /// Build the router
    pub fn build(self) -> Router {
        Router {
            name: self.name,
            routes: self.routes,
            fallback: self.fallback,
        }
    }
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LLMConfig, LLMRequest, LLMResponse, ModelInfo};
    use crate::workflow::classifier::RuleClassifier;
    use async_trait::async_trait;

    struct MockProvider;

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> crate::error::Result<String> {
            Ok("mock response".to_string())
        }

        async fn generate_request(&self, _request: &LLMRequest) -> crate::error::Result<LLMResponse> {
            Ok(LLMResponse {
                content: "mock response".to_string(),
                usage: None,
            })
        }

        fn model_info(&self) -> ModelInfo {
            ModelInfo {
                provider: "mock".to_string(),
                model_name: "test".to_string(),
            }
        }
    }

    #[test]
    fn test_router_builder() {
        let router = Router::builder()
            .name("test-router")
            .step_route("greeting", Step::llm("greet", "{{input}}").build())
            .step_route("question", Step::llm("answer", "{{input}}").build())
            .fallback_step(Step::llm("default", "{{input}}").build())
            .build();

        assert_eq!(router.name(), "test-router");
        assert_eq!(router.route_count(), 2);
    }

    #[tokio::test]
    async fn test_router_execution() {
        let router = Router::builder()
            .name("test")
            .step_route(
                "greeting",
                Step::transform("greet_handler", |_| {
                    Ok(serde_json::json!({"type": "greeting_response"}))
                })
                .build(),
            )
            .step_route(
                "question",
                Step::transform("question_handler", |_| {
                    Ok(serde_json::json!({"type": "question_response"}))
                })
                .build(),
            )
            .fallback_step(
                Step::transform("default_handler", |_| {
                    Ok(serde_json::json!({"type": "default_response"}))
                })
                .build(),
            )
            .build();

        let classifier = RuleClassifier::new("other")
            .add_contains_rule("greeting", "hello")
            .add_contains_rule("question", "?");

        let provider = MockProvider;

        // Test greeting route
        let (output, trace) = router
            .execute(serde_json::json!("Hello there!"), &classifier, &provider)
            .await
            .unwrap();

        assert_eq!(output.data["type"], "greeting_response");
        assert_eq!(trace.selected_route, "greeting");

        // Test question route
        let (output, trace) = router
            .execute(serde_json::json!("How are you?"), &classifier, &provider)
            .await
            .unwrap();

        assert_eq!(output.data["type"], "question_response");
        assert_eq!(trace.selected_route, "question");

        // Test fallback route
        let (output, trace) = router
            .execute(serde_json::json!("Just a statement"), &classifier, &provider)
            .await
            .unwrap();

        assert_eq!(output.data["type"], "default_response");
        assert_eq!(trace.selected_route, "fallback");
    }

    #[tokio::test]
    async fn test_router_with_confidence_threshold() {
        let router = Router::builder()
            .route(
                Route::step(
                    "high_confidence",
                    Step::transform("high", |_| Ok(serde_json::json!("high"))).build(),
                )
                .with_min_confidence(0.8),
            )
            .fallback_step(
                Step::transform("fallback", |_| Ok(serde_json::json!("fallback"))).build(),
            )
            .build();

        // Create a classifier that returns low confidence
        struct LowConfidenceClassifier;

        #[async_trait]
        impl Classifier for LowConfidenceClassifier {
            async fn classify(&self, _input: &serde_json::Value) -> WorkflowResult<Classification> {
                Ok(Classification::new("high_confidence", 0.5))
            }

            fn labels(&self) -> &[String] {
                &[]
            }
        }

        let provider = MockProvider;
        let (output, trace) = router
            .execute(serde_json::json!("test"), &LowConfidenceClassifier, &provider)
            .await
            .unwrap();

        // Should fall back due to low confidence
        assert_eq!(output.data, serde_json::json!("fallback"));
        assert_eq!(trace.selected_route, "fallback");
    }

    #[tokio::test]
    async fn test_execute_with_label() {
        let router = Router::builder()
            .step_route(
                "route_a",
                Step::transform("a", |_| Ok(serde_json::json!("a"))).build(),
            )
            .step_route(
                "route_b",
                Step::transform("b", |_| Ok(serde_json::json!("b"))).build(),
            )
            .build();

        let provider = MockProvider;

        let (output, _) = router
            .execute_with_label(serde_json::json!("input"), "route_a", &provider)
            .await
            .unwrap();

        assert_eq!(output.data, serde_json::json!("a"));

        let (output, _) = router
            .execute_with_label(serde_json::json!("input"), "route_b", &provider)
            .await
            .unwrap();

        assert_eq!(output.data, serde_json::json!("b"));
    }
}
