//! Gate conditions for workflow control flow

use serde_json::Value;

/// Gate condition for controlling workflow execution
#[derive(Clone)]
pub struct Gate {
    /// Gate name for identification
    pub name: String,
    /// The condition to evaluate
    pub condition: GateCondition,
    /// Message to include when gate halts execution
    pub halt_message: Option<String>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("name", &self.name)
            .field("halt_message", &self.halt_message)
            .finish()
    }
}

/// Condition types for gates
#[derive(Clone)]
pub enum GateCondition {
    /// Always pass
    Always,
    /// Never pass (always halt)
    Never,
    /// Check if a field exists
    FieldExists(String),
    /// Check if a field equals a value
    FieldEquals(String, Value),
    /// Check if a field is truthy
    FieldTruthy(String),
    /// Check if output contains a substring
    Contains(String),
    /// Check if output does not contain a substring
    NotContains(String),
    /// Custom predicate function
    Custom(std::sync::Arc<dyn Fn(&Value) -> bool + Send + Sync>),
    /// All conditions must pass
    All(Vec<GateCondition>),
    /// Any condition must pass
    Any(Vec<GateCondition>),
    /// Negate a condition
    Not(Box<GateCondition>),
}

impl std::fmt::Debug for GateCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateCondition::Always => write!(f, "Always"),
            GateCondition::Never => write!(f, "Never"),
            GateCondition::FieldExists(field) => write!(f, "FieldExists({})", field),
            GateCondition::FieldEquals(field, value) => {
                write!(f, "FieldEquals({}, {:?})", field, value)
            }
            GateCondition::FieldTruthy(field) => write!(f, "FieldTruthy({})", field),
            GateCondition::Contains(s) => write!(f, "Contains({})", s),
            GateCondition::NotContains(s) => write!(f, "NotContains({})", s),
            GateCondition::Custom(_) => write!(f, "Custom(...)"),
            GateCondition::All(conditions) => write!(f, "All({:?})", conditions),
            GateCondition::Any(conditions) => write!(f, "Any({:?})", conditions),
            GateCondition::Not(condition) => write!(f, "Not({:?})", condition),
        }
    }
}

impl GateCondition {
    /// Evaluate the condition against a value
    pub fn evaluate(&self, value: &Value) -> bool {
        match self {
            GateCondition::Always => true,
            GateCondition::Never => false,
            GateCondition::FieldExists(field) => {
                value.get(field).is_some()
            }
            GateCondition::FieldEquals(field, expected) => {
                value.get(field).map(|v| v == expected).unwrap_or(false)
            }
            GateCondition::FieldTruthy(field) => {
                value.get(field).map(is_truthy).unwrap_or(false)
            }
            GateCondition::Contains(substring) => {
                value_to_string(value).contains(substring)
            }
            GateCondition::NotContains(substring) => {
                !value_to_string(value).contains(substring)
            }
            GateCondition::Custom(predicate) => predicate(value),
            GateCondition::All(conditions) => {
                conditions.iter().all(|c| c.evaluate(value))
            }
            GateCondition::Any(conditions) => {
                conditions.iter().any(|c| c.evaluate(value))
            }
            GateCondition::Not(condition) => !condition.evaluate(value),
        }
    }
}

impl Gate {
    /// Create a new gate
    pub fn new(name: impl Into<String>, condition: GateCondition) -> Self {
        Self {
            name: name.into(),
            condition,
            halt_message: None,
        }
    }

    /// Set the halt message
    pub fn with_halt_message(mut self, message: impl Into<String>) -> Self {
        self.halt_message = Some(message.into());
        self
    }

    /// Evaluate the gate
    pub fn evaluate(&self, value: &Value) -> bool {
        self.condition.evaluate(value)
    }

    /// Create a gate that checks if a field exists
    pub fn field_exists(name: impl Into<String>, field: impl Into<String>) -> Self {
        Self::new(name, GateCondition::FieldExists(field.into()))
    }

    /// Create a gate that checks if a field equals a value
    pub fn field_equals(name: impl Into<String>, field: impl Into<String>, value: Value) -> Self {
        Self::new(name, GateCondition::FieldEquals(field.into(), value))
    }

    /// Create a gate that checks if output contains a string
    pub fn contains(name: impl Into<String>, substring: impl Into<String>) -> Self {
        Self::new(name, GateCondition::Contains(substring.into()))
    }

    /// Create a gate that checks if output does not contain a string
    pub fn not_contains(name: impl Into<String>, substring: impl Into<String>) -> Self {
        Self::new(name, GateCondition::NotContains(substring.into()))
    }

    /// Create a custom gate
    pub fn custom<F>(name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&Value) -> bool + Send + Sync + 'static,
    {
        Self::new(name, GateCondition::Custom(std::sync::Arc::new(predicate)))
    }
}

/// Check if a JSON value is "truthy"
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    }
}

/// Convert a JSON value to a string for substring matching
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_gate() {
        let gate = Gate::new("always", GateCondition::Always);
        assert!(gate.evaluate(&serde_json::json!({})));
    }

    #[test]
    fn test_never_gate() {
        let gate = Gate::new("never", GateCondition::Never);
        assert!(!gate.evaluate(&serde_json::json!({})));
    }

    #[test]
    fn test_field_exists_gate() {
        let gate = Gate::field_exists("has_name", "name");

        assert!(gate.evaluate(&serde_json::json!({"name": "Alice"})));
        assert!(!gate.evaluate(&serde_json::json!({"age": 30})));
    }

    #[test]
    fn test_field_equals_gate() {
        let gate = Gate::field_equals("status_ok", "status", serde_json::json!("ok"));

        assert!(gate.evaluate(&serde_json::json!({"status": "ok"})));
        assert!(!gate.evaluate(&serde_json::json!({"status": "error"})));
    }

    #[test]
    fn test_contains_gate() {
        let gate = Gate::contains("has_error", "error");

        assert!(gate.evaluate(&serde_json::json!("An error occurred")));
        assert!(!gate.evaluate(&serde_json::json!("All good")));
    }

    #[test]
    fn test_not_contains_gate() {
        let gate = Gate::not_contains("no_error", "error");

        assert!(!gate.evaluate(&serde_json::json!("An error occurred")));
        assert!(gate.evaluate(&serde_json::json!("All good")));
    }

    #[test]
    fn test_custom_gate() {
        let gate = Gate::custom("positive", |v| {
            v.as_i64().map(|n| n > 0).unwrap_or(false)
        });

        assert!(gate.evaluate(&serde_json::json!(5)));
        assert!(!gate.evaluate(&serde_json::json!(-3)));
    }

    #[test]
    fn test_all_condition() {
        let condition = GateCondition::All(vec![
            GateCondition::FieldExists("name".to_string()),
            GateCondition::FieldExists("age".to_string()),
        ]);

        assert!(condition.evaluate(&serde_json::json!({"name": "Alice", "age": 30})));
        assert!(!condition.evaluate(&serde_json::json!({"name": "Alice"})));
    }

    #[test]
    fn test_any_condition() {
        let condition = GateCondition::Any(vec![
            GateCondition::FieldExists("name".to_string()),
            GateCondition::FieldExists("title".to_string()),
        ]);

        assert!(condition.evaluate(&serde_json::json!({"name": "Alice"})));
        assert!(condition.evaluate(&serde_json::json!({"title": "Dr."})));
        assert!(!condition.evaluate(&serde_json::json!({"age": 30})));
    }

    #[test]
    fn test_not_condition() {
        let condition = GateCondition::Not(Box::new(GateCondition::FieldExists("error".to_string())));

        assert!(condition.evaluate(&serde_json::json!({"status": "ok"})));
        assert!(!condition.evaluate(&serde_json::json!({"error": "failed"})));
    }

    #[test]
    fn test_truthy() {
        assert!(!is_truthy(&serde_json::json!(null)));
        assert!(!is_truthy(&serde_json::json!(false)));
        assert!(is_truthy(&serde_json::json!(true)));
        assert!(!is_truthy(&serde_json::json!(0)));
        assert!(is_truthy(&serde_json::json!(1)));
        assert!(!is_truthy(&serde_json::json!("")));
        assert!(is_truthy(&serde_json::json!("hello")));
        assert!(!is_truthy(&serde_json::json!([])));
        assert!(is_truthy(&serde_json::json!([1])));
        assert!(!is_truthy(&serde_json::json!({})));
        assert!(is_truthy(&serde_json::json!({"a": 1})));
    }
}
