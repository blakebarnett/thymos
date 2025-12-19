//! Classifiers for Router Workflow Pattern
//!
//! Classifiers analyze input and produce a classification label with confidence.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};

use super::execution::{WorkflowError, WorkflowResult};

/// Classification result with label and confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    /// The classification label
    pub label: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

impl Classification {
    /// Create a new classification
    pub fn new(label: impl Into<String>, confidence: f32) -> Self {
        Self {
            label: label.into(),
            confidence: confidence.clamp(0.0, 1.0),
            metadata: None,
        }
    }

    /// Create with metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Trait for classifying input
#[async_trait]
pub trait Classifier: Send + Sync {
    /// Classify the input and return a classification
    async fn classify(&self, input: &serde_json::Value) -> WorkflowResult<Classification>;

    /// Get the possible labels this classifier can produce
    fn labels(&self) -> &[String];
}

/// Rule-based classifier using exact matching
pub struct RuleClassifier {
    rules: Vec<(String, Box<dyn Fn(&serde_json::Value) -> bool + Send + Sync>)>,
    default_label: String,
    labels: Vec<String>,
}

impl std::fmt::Debug for RuleClassifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleClassifier")
            .field("labels", &self.labels)
            .field("default_label", &self.default_label)
            .finish()
    }
}

impl RuleClassifier {
    /// Create a new rule classifier
    pub fn new(default_label: impl Into<String>) -> Self {
        let default = default_label.into();
        Self {
            rules: Vec::new(),
            labels: vec![default.clone()],
            default_label: default,
        }
    }

    /// Add a rule
    pub fn add_rule<F>(mut self, label: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&serde_json::Value) -> bool + Send + Sync + 'static,
    {
        let label = label.into();
        if !self.labels.contains(&label) {
            self.labels.push(label.clone());
        }
        self.rules.push((label, Box::new(predicate)));
        self
    }

    /// Add a rule that checks if input contains a substring
    pub fn add_contains_rule(self, label: impl Into<String>, substring: impl Into<String>) -> Self {
        let substring = substring.into();
        self.add_rule(label, move |input| {
            let text = match input {
                serde_json::Value::String(s) => s.clone(),
                other => serde_json::to_string(other).unwrap_or_default(),
            };
            text.to_lowercase().contains(&substring.to_lowercase())
        })
    }

    /// Add a rule that checks if a field equals a value
    pub fn add_field_equals_rule(
        self,
        label: impl Into<String>,
        field: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        let field = field.into();
        self.add_rule(label, move |input| {
            input.get(&field).map(|v| v == &value).unwrap_or(false)
        })
    }
}

#[async_trait]
impl Classifier for RuleClassifier {
    async fn classify(&self, input: &serde_json::Value) -> WorkflowResult<Classification> {
        for (label, predicate) in &self.rules {
            if predicate(input) {
                return Ok(Classification::new(label.clone(), 1.0));
            }
        }

        Ok(Classification::new(self.default_label.clone(), 0.5))
    }

    fn labels(&self) -> &[String] {
        &self.labels
    }
}

/// LLM-based classifier
pub struct LLMClassifier {
    labels: Vec<String>,
    system_prompt: String,
    default_label: String,
}

impl LLMClassifier {
    /// Create a new LLM classifier
    ///
    /// # Arguments
    ///
    /// * `labels` - Possible classification labels
    /// * `default_label` - Default label if classification fails
    pub fn new(labels: Vec<String>, default_label: impl Into<String>) -> Self {
        let default = default_label.into();
        let labels_str = labels.join(", ");

        let system_prompt = format!(
            "You are a classifier. Classify the user's input into exactly one of these categories: {}. \
            Respond with ONLY the category name, nothing else.",
            labels_str
        );

        Self {
            labels,
            system_prompt,
            default_label: default,
        }
    }

    /// Create with a custom system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Classify using an LLM provider
    pub async fn classify_with_provider(
        &self,
        input: &serde_json::Value,
        provider: &dyn LLMProvider,
    ) -> WorkflowResult<Classification> {
        let input_str = match input {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };

        let request = LLMRequest {
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: MessageRole::User,
                    content: input_str,
                },
            ],
            temperature: Some(0.0),
            max_tokens: Some(50),
            stop_sequences: Vec::new(),
        };

        let response = provider
            .generate_request(&request)
            .await
            .map_err(|e| WorkflowError::LLMError(e.to_string()))?;

        let label = response.content.trim().to_string();

        // Check if the label is valid
        if self.labels.contains(&label) {
            Ok(Classification::new(label, 0.9))
        } else {
            // Try fuzzy matching
            for valid_label in &self.labels {
                if label.to_lowercase().contains(&valid_label.to_lowercase())
                    || valid_label.to_lowercase().contains(&label.to_lowercase())
                {
                    return Ok(Classification::new(valid_label.clone(), 0.7));
                }
            }

            Ok(Classification::new(self.default_label.clone(), 0.3))
        }
    }
}

/// Wrapper to use LLMClassifier with a stored provider reference
pub struct LLMClassifierWithProvider<'a> {
    classifier: &'a LLMClassifier,
    provider: &'a dyn LLMProvider,
}

impl<'a> LLMClassifierWithProvider<'a> {
    pub fn new(classifier: &'a LLMClassifier, provider: &'a dyn LLMProvider) -> Self {
        Self { classifier, provider }
    }
}

#[async_trait]
impl<'a> Classifier for LLMClassifierWithProvider<'a> {
    async fn classify(&self, input: &serde_json::Value) -> WorkflowResult<Classification> {
        self.classifier.classify_with_provider(input, self.provider).await
    }

    fn labels(&self) -> &[String] {
        &self.classifier.labels
    }
}

/// Keyword-based classifier with weighted terms
pub struct KeywordClassifier {
    keywords: Vec<(String, Vec<String>)>,
    default_label: String,
    labels: Vec<String>,
}

impl KeywordClassifier {
    /// Create a new keyword classifier
    pub fn new(default_label: impl Into<String>) -> Self {
        let default = default_label.into();
        Self {
            keywords: Vec::new(),
            labels: vec![default.clone()],
            default_label: default,
        }
    }

    /// Add keywords for a label
    pub fn add_keywords(mut self, label: impl Into<String>, keywords: Vec<String>) -> Self {
        let label = label.into();
        if !self.labels.contains(&label) {
            self.labels.push(label.clone());
        }
        self.keywords.push((label, keywords));
        self
    }
}

#[async_trait]
impl Classifier for KeywordClassifier {
    async fn classify(&self, input: &serde_json::Value) -> WorkflowResult<Classification> {
        let text = match input {
            serde_json::Value::String(s) => s.to_lowercase(),
            other => serde_json::to_string(other).unwrap_or_default().to_lowercase(),
        };

        let mut best_match: Option<(String, usize)> = None;

        for (label, keywords) in &self.keywords {
            let count = keywords
                .iter()
                .filter(|k| text.contains(&k.to_lowercase()))
                .count();

            if count > 0 {
                if let Some((_, best_count)) = &best_match {
                    if count > *best_count {
                        best_match = Some((label.clone(), count));
                    }
                } else {
                    best_match = Some((label.clone(), count));
                }
            }
        }

        match best_match {
            Some((label, count)) => {
                let max_keywords = self
                    .keywords
                    .iter()
                    .find(|(l, _)| l == &label)
                    .map(|(_, kw)| kw.len())
                    .unwrap_or(1);

                let confidence = (count as f32 / max_keywords as f32).min(1.0);
                Ok(Classification::new(label, confidence))
            }
            None => Ok(Classification::new(self.default_label.clone(), 0.3)),
        }
    }

    fn labels(&self) -> &[String] {
        &self.labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rule_classifier() {
        let classifier = RuleClassifier::new("other")
            .add_contains_rule("greeting", "hello")
            .add_contains_rule("question", "?");

        let result = classifier
            .classify(&serde_json::json!("Hello there!"))
            .await
            .unwrap();
        assert_eq!(result.label, "greeting");

        let result = classifier
            .classify(&serde_json::json!("How are you?"))
            .await
            .unwrap();
        assert_eq!(result.label, "question");

        let result = classifier
            .classify(&serde_json::json!("Just a statement"))
            .await
            .unwrap();
        assert_eq!(result.label, "other");
    }

    #[tokio::test]
    async fn test_keyword_classifier() {
        let classifier = KeywordClassifier::new("general")
            .add_keywords(
                "tech".to_string(),
                vec!["computer".to_string(), "software".to_string(), "code".to_string()],
            )
            .add_keywords(
                "food".to_string(),
                vec!["recipe".to_string(), "cook".to_string(), "eat".to_string()],
            );

        let result = classifier
            .classify(&serde_json::json!("I need help with my computer"))
            .await
            .unwrap();
        assert_eq!(result.label, "tech");

        let result = classifier
            .classify(&serde_json::json!("What recipe should I cook?"))
            .await
            .unwrap();
        assert_eq!(result.label, "food");
    }

    #[test]
    fn test_classification_creation() {
        let c = Classification::new("test", 0.8);
        assert_eq!(c.label, "test");
        assert_eq!(c.confidence, 0.8);
    }

    #[test]
    fn test_classification_clamping() {
        let c = Classification::new("test", 1.5);
        assert_eq!(c.confidence, 1.0);

        let c = Classification::new("test", -0.5);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn test_rule_classifier_labels() {
        let classifier = RuleClassifier::new("default")
            .add_contains_rule("a", "test")
            .add_contains_rule("b", "example");

        assert!(classifier.labels().contains(&"default".to_string()));
        assert!(classifier.labels().contains(&"a".to_string()));
        assert!(classifier.labels().contains(&"b".to_string()));
    }
}
