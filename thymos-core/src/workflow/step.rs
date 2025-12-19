//! Workflow Step definition

use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::execution::{StepTrace, WorkflowError, WorkflowResult};

/// Type of step execution
#[derive(Clone)]
pub enum StepType {
    /// LLM call with a prompt template
    LLM {
        /// System prompt for this step
        system_prompt: Option<String>,
        /// User prompt template (can use {{input}} placeholder)
        prompt_template: String,
    },
    /// Transform step (applies a function to the input)
    Transform {
        /// Transform function name (for tracing)
        name: String,
        /// Transform function
        transform: Arc<dyn Fn(serde_json::Value) -> WorkflowResult<serde_json::Value> + Send + Sync>,
    },
    /// Extract specific fields from the input
    Extract {
        /// JSON path or field names to extract
        fields: Vec<String>,
    },
}

impl std::fmt::Debug for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepType::LLM { system_prompt, prompt_template } => {
                f.debug_struct("LLM")
                    .field("system_prompt", system_prompt)
                    .field("prompt_template", prompt_template)
                    .finish()
            }
            StepType::Transform { name, .. } => {
                f.debug_struct("Transform")
                    .field("name", name)
                    .finish()
            }
            StepType::Extract { fields } => {
                f.debug_struct("Extract")
                    .field("fields", fields)
                    .finish()
            }
        }
    }
}

/// Output from a step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    /// The output data
    pub data: serde_json::Value,
    /// Raw text output (for LLM steps)
    pub raw_text: Option<String>,
    /// Token usage if available
    pub prompt_tokens: Option<usize>,
    /// Completion tokens if available
    pub completion_tokens: Option<usize>,
}

impl StepOutput {
    /// Create a new step output
    pub fn new(data: serde_json::Value) -> Self {
        Self {
            data,
            raw_text: None,
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    /// Create with raw text
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            data: serde_json::Value::String(text.clone()),
            raw_text: Some(text),
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    /// Add token usage
    pub fn with_usage(mut self, prompt_tokens: usize, completion_tokens: usize) -> Self {
        self.prompt_tokens = Some(prompt_tokens);
        self.completion_tokens = Some(completion_tokens);
        self
    }
}

/// A single step in a workflow
pub struct Step {
    /// Step name for identification
    pub name: String,
    /// Step type (LLM, Transform, Extract)
    pub step_type: StepType,
    /// Optional description
    pub description: Option<String>,
    /// Whether to parse output as JSON
    pub parse_json: bool,
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Step")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("parse_json", &self.parse_json)
            .finish()
    }
}

impl Step {
    /// Create an LLM step with a prompt template
    ///
    /// The template can use `{{input}}` to include the input from the previous step.
    pub fn llm(name: impl Into<String>, prompt_template: impl Into<String>) -> StepBuilder {
        StepBuilder {
            name: name.into(),
            step_type: StepType::LLM {
                system_prompt: None,
                prompt_template: prompt_template.into(),
            },
            description: None,
            parse_json: false,
        }
    }

    /// Create a transform step
    pub fn transform<F>(name: impl Into<String>, transform: F) -> StepBuilder
    where
        F: Fn(serde_json::Value) -> WorkflowResult<serde_json::Value> + Send + Sync + 'static,
    {
        let name_str = name.into();
        StepBuilder {
            name: name_str.clone(),
            step_type: StepType::Transform {
                name: name_str,
                transform: Arc::new(transform),
            },
            description: None,
            parse_json: false,
        }
    }

    /// Create an extract step
    pub fn extract(name: impl Into<String>, fields: Vec<String>) -> StepBuilder {
        StepBuilder {
            name: name.into(),
            step_type: StepType::Extract { fields },
            description: None,
            parse_json: false,
        }
    }

    /// Execute this step
    pub async fn execute(
        &self,
        input: serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<(StepOutput, StepTrace)> {
        let start = std::time::Instant::now();

        let result = match &self.step_type {
            StepType::LLM {
                system_prompt,
                prompt_template,
            } => {
                self.execute_llm(input.clone(), system_prompt.as_deref(), prompt_template, provider)
                    .await
            }
            StepType::Transform { transform, .. } => self.execute_transform(input.clone(), transform),
            StepType::Extract { fields } => self.execute_extract(input.clone(), fields),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let mut trace = StepTrace::success(
                    &self.name,
                    input,
                    output.data.clone(),
                    duration_ms,
                );

                if let (Some(p), Some(c)) = (output.prompt_tokens, output.completion_tokens) {
                    trace = trace.with_token_usage(p, c, p + c);
                }

                Ok((output, trace))
            }
            Err(e) => {
                let trace = StepTrace::failure(&self.name, input, e.to_string(), duration_ms);
                Err(e)
            }
        }
    }

    async fn execute_llm(
        &self,
        input: serde_json::Value,
        system_prompt: Option<&str>,
        prompt_template: &str,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<StepOutput> {
        // Render the prompt template
        let input_str = match &input {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };

        let rendered_prompt = prompt_template.replace("{{input}}", &input_str);

        // Build messages
        let mut messages = Vec::new();

        if let Some(system) = system_prompt {
            messages.push(Message {
                role: MessageRole::System,
                content: system.to_string(),
            });
        }

        messages.push(Message {
            role: MessageRole::User,
            content: rendered_prompt,
        });

        let request = LLMRequest {
            messages,
            temperature: Some(0.7),
            max_tokens: Some(1000),
            stop_sequences: Vec::new(),
        };

        let response = provider
            .generate_request(&request)
            .await
            .map_err(|e| WorkflowError::LLMError(e.to_string()))?;

        let mut output = if self.parse_json {
            // Try to parse as JSON
            let data = serde_json::from_str(&response.content)
                .map_err(|e| WorkflowError::ParseError(format!("Invalid JSON: {}", e)))?;
            StepOutput::new(data)
        } else {
            StepOutput::with_text(&response.content)
        };

        output.raw_text = Some(response.content);

        if let Some(usage) = response.usage {
            output = output.with_usage(usage.prompt_tokens, usage.completion_tokens);
        }

        Ok(output)
    }

    fn execute_transform(
        &self,
        input: serde_json::Value,
        transform: &Arc<dyn Fn(serde_json::Value) -> WorkflowResult<serde_json::Value> + Send + Sync>,
    ) -> WorkflowResult<StepOutput> {
        let data = transform(input)?;
        Ok(StepOutput::new(data))
    }

    fn execute_extract(
        &self,
        input: serde_json::Value,
        fields: &[String],
    ) -> WorkflowResult<StepOutput> {
        let obj = input.as_object().ok_or_else(|| {
            WorkflowError::ParseError("Extract step requires object input".to_string())
        })?;

        let mut extracted = serde_json::Map::new();
        for field in fields {
            if let Some(value) = obj.get(field) {
                extracted.insert(field.clone(), value.clone());
            }
        }

        Ok(StepOutput::new(serde_json::Value::Object(extracted)))
    }
}

/// Builder for creating Steps
pub struct StepBuilder {
    name: String,
    step_type: StepType,
    description: Option<String>,
    parse_json: bool,
}

impl StepBuilder {
    /// Add a description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a system prompt (for LLM steps)
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        if let StepType::LLM {
            ref mut system_prompt,
            ..
        } = self.step_type
        {
            *system_prompt = Some(prompt.into());
        }
        self
    }

    /// Parse the output as JSON
    pub fn parse_json(mut self) -> Self {
        self.parse_json = true;
        self
    }

    /// Build the step
    pub fn build(self) -> Step {
        Step {
            name: self.name,
            step_type: self.step_type,
            description: self.description,
            parse_json: self.parse_json,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_output() {
        let output = StepOutput::new(serde_json::json!({"key": "value"}));
        assert!(output.data.is_object());
    }

    #[test]
    fn test_step_output_with_text() {
        let output = StepOutput::with_text("Hello");
        assert_eq!(output.raw_text, Some("Hello".to_string()));
    }

    #[test]
    fn test_step_builder_llm() {
        let step = Step::llm("summarize", "Summarize: {{input}}")
            .description("Summarizes the input")
            .system_prompt("You are a helpful assistant")
            .parse_json()
            .build();

        assert_eq!(step.name, "summarize");
        assert!(step.parse_json);
    }

    #[test]
    fn test_step_builder_transform() {
        let step = Step::transform("uppercase", |input| {
            let s = input.as_str().unwrap_or("").to_uppercase();
            Ok(serde_json::Value::String(s))
        })
        .build();

        assert_eq!(step.name, "uppercase");
    }

    #[test]
    fn test_step_builder_extract() {
        let step = Step::extract("extract_name", vec!["name".to_string(), "age".to_string()])
            .description("Extract name and age fields")
            .build();

        assert_eq!(step.name, "extract_name");
    }

    #[test]
    fn test_extract_step_execution() {
        let step = Step::extract("extract", vec!["name".to_string()]).build();

        let input = serde_json::json!({"name": "Alice", "age": 30});
        let result = step.execute_extract(input, &["name".to_string()]);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.data, serde_json::json!({"name": "Alice"}));
    }

    #[test]
    fn test_transform_step_execution() {
        let transform: Arc<dyn Fn(serde_json::Value) -> WorkflowResult<serde_json::Value> + Send + Sync> =
            Arc::new(|input| {
                let n = input.as_i64().unwrap_or(0);
                Ok(serde_json::json!(n * 2))
            });

        let step = Step {
            name: "double".to_string(),
            step_type: StepType::Transform {
                name: "double".to_string(),
                transform: transform.clone(),
            },
            description: None,
            parse_json: false,
        };

        let result = step.execute_transform(serde_json::json!(5), &transform);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data, serde_json::json!(10));
    }
}
