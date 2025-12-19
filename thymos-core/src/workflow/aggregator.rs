//! Aggregators for Parallel Workflow Pattern
//!
//! Aggregators combine results from multiple parallel branches.

use async_trait::async_trait;

use super::execution::{WorkflowError, WorkflowResult};
use super::step::StepOutput;

/// Trait for aggregating results from parallel branches
#[async_trait]
pub trait Aggregator: Send + Sync {
    /// Aggregate multiple results into a single result
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput>;
}

/// Returns the first successful result
pub struct FirstSuccess;

#[async_trait]
impl Aggregator for FirstSuccess {
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput> {
        for result in results {
            if let Ok(output) = result {
                return Ok(output);
            }
        }
        Err(WorkflowError::StepFailed {
            step: "parallel".to_string(),
            message: "All branches failed".to_string(),
        })
    }
}

/// Requires all branches to succeed, returns merged results
pub struct AllSuccess;

#[async_trait]
impl Aggregator for AllSuccess {
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput> {
        let mut outputs = Vec::new();

        for result in results {
            match result {
                Ok(output) => outputs.push(output.data),
                Err(e) => return Err(e),
            }
        }

        Ok(StepOutput::new(serde_json::Value::Array(outputs)))
    }
}

/// Voting aggregator - returns the most common result
pub struct Voting {
    /// Minimum number of matching results required
    pub min_votes: usize,
}

impl Voting {
    pub fn new(min_votes: usize) -> Self {
        Self { min_votes }
    }
}

impl Default for Voting {
    fn default() -> Self {
        Self { min_votes: 2 }
    }
}

#[async_trait]
impl Aggregator for Voting {
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput> {
        let mut vote_counts: std::collections::HashMap<String, (usize, StepOutput)> =
            std::collections::HashMap::new();

        for result in results {
            if let Ok(output) = result {
                let key = serde_json::to_string(&output.data).unwrap_or_default();
                let entry = vote_counts.entry(key).or_insert((0, output.clone()));
                entry.0 += 1;
            }
        }

        let winner = vote_counts
            .into_iter()
            .max_by_key(|(_, (count, _))| *count)
            .map(|(_, (count, output))| (count, output));

        match winner {
            Some((count, output)) if count >= self.min_votes => Ok(output),
            Some((count, _)) => Err(WorkflowError::StepFailed {
                step: "voting".to_string(),
                message: format!("Not enough votes: got {}, need {}", count, self.min_votes),
            }),
            None => Err(WorkflowError::StepFailed {
                step: "voting".to_string(),
                message: "No successful results to vote on".to_string(),
            }),
        }
    }
}

/// Merge all results into a single object
pub struct Merge {
    /// Keys to assign to each branch result
    pub keys: Vec<String>,
}

impl Merge {
    pub fn new(keys: Vec<String>) -> Self {
        Self { keys }
    }

    pub fn with_keys(keys: &[&str]) -> Self {
        Self {
            keys: keys.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[async_trait]
impl Aggregator for Merge {
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput> {
        let mut merged = serde_json::Map::new();

        for (i, result) in results.into_iter().enumerate() {
            let key = self.keys.get(i).cloned().unwrap_or_else(|| format!("branch_{}", i));

            match result {
                Ok(output) => {
                    merged.insert(key, output.data);
                }
                Err(e) => {
                    merged.insert(key, serde_json::json!({"error": e.to_string()}));
                }
            }
        }

        Ok(StepOutput::new(serde_json::Value::Object(merged)))
    }
}

/// Best result based on a scoring function
pub struct BestResult {
    scorer: Box<dyn Fn(&serde_json::Value) -> f64 + Send + Sync>,
}

impl BestResult {
    pub fn new<F>(scorer: F) -> Self
    where
        F: Fn(&serde_json::Value) -> f64 + Send + Sync + 'static,
    {
        Self {
            scorer: Box::new(scorer),
        }
    }
}

#[async_trait]
impl Aggregator for BestResult {
    fn aggregate(&self, results: Vec<WorkflowResult<StepOutput>>) -> WorkflowResult<StepOutput> {
        let mut best: Option<(f64, StepOutput)> = None;

        for result in results {
            if let Ok(output) = result {
                let score = (self.scorer)(&output.data);

                if let Some((best_score, _)) = &best {
                    if score > *best_score {
                        best = Some((score, output));
                    }
                } else {
                    best = Some((score, output));
                }
            }
        }

        best.map(|(_, output)| output).ok_or_else(|| WorkflowError::StepFailed {
            step: "best_result".to_string(),
            message: "No successful results".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_output(data: serde_json::Value) -> StepOutput {
        StepOutput::new(data)
    }

    #[test]
    fn test_first_success() {
        let agg = FirstSuccess;

        // First success
        let results = vec![
            Err(WorkflowError::StepFailed {
                step: "a".to_string(),
                message: "fail".to_string(),
            }),
            Ok(make_output(serde_json::json!(2))),
            Ok(make_output(serde_json::json!(3))),
        ];

        let result = agg.aggregate(results).unwrap();
        assert_eq!(result.data, serde_json::json!(2));

        // All fail
        let results = vec![
            Err(WorkflowError::StepFailed {
                step: "a".to_string(),
                message: "fail".to_string(),
            }),
            Err(WorkflowError::StepFailed {
                step: "b".to_string(),
                message: "fail".to_string(),
            }),
        ];

        assert!(agg.aggregate(results).is_err());
    }

    #[test]
    fn test_all_success() {
        let agg = AllSuccess;

        // All succeed
        let results = vec![
            Ok(make_output(serde_json::json!(1))),
            Ok(make_output(serde_json::json!(2))),
            Ok(make_output(serde_json::json!(3))),
        ];

        let result = agg.aggregate(results).unwrap();
        assert_eq!(result.data, serde_json::json!([1, 2, 3]));

        // One fails
        let results = vec![
            Ok(make_output(serde_json::json!(1))),
            Err(WorkflowError::StepFailed {
                step: "b".to_string(),
                message: "fail".to_string(),
            }),
        ];

        assert!(agg.aggregate(results).is_err());
    }

    #[test]
    fn test_voting() {
        let agg = Voting::new(2);

        // Winner with 2 votes
        let results = vec![
            Ok(make_output(serde_json::json!("a"))),
            Ok(make_output(serde_json::json!("b"))),
            Ok(make_output(serde_json::json!("a"))),
        ];

        let result = agg.aggregate(results).unwrap();
        assert_eq!(result.data, serde_json::json!("a"));

        // Not enough votes
        let results = vec![
            Ok(make_output(serde_json::json!("a"))),
            Ok(make_output(serde_json::json!("b"))),
            Ok(make_output(serde_json::json!("c"))),
        ];

        assert!(agg.aggregate(results).is_err());
    }

    #[test]
    fn test_merge() {
        let agg = Merge::with_keys(&["first", "second", "third"]);

        let results = vec![
            Ok(make_output(serde_json::json!(1))),
            Ok(make_output(serde_json::json!(2))),
            Ok(make_output(serde_json::json!(3))),
        ];

        let result = agg.aggregate(results).unwrap();
        assert_eq!(result.data["first"], 1);
        assert_eq!(result.data["second"], 2);
        assert_eq!(result.data["third"], 3);
    }

    #[test]
    fn test_best_result() {
        let agg = BestResult::new(|v| v.as_i64().unwrap_or(0) as f64);

        let results = vec![
            Ok(make_output(serde_json::json!(10))),
            Ok(make_output(serde_json::json!(50))),
            Ok(make_output(serde_json::json!(30))),
        ];

        let result = agg.aggregate(results).unwrap();
        assert_eq!(result.data, serde_json::json!(50));
    }
}
