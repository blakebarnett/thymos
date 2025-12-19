//! Bisect Regression Pattern
//!
//! Binary search through memory history to find when something broke.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{Result, ThymosError};
use crate::memory::versioning::{MemoryCommit, MemoryRepository};

/// Result of testing a commit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BisectResult {
    /// Commit is good (before the regression)
    Good,
    /// Commit is bad (has the regression)
    Bad,
    /// Cannot test this commit, skip it
    Skip,
}

/// Trait for testing commits during bisect
#[async_trait]
pub trait CommitTester: Send + Sync {
    /// Test a commit and return whether it's good, bad, or should be skipped
    async fn test_commit(&self, commit: &MemoryCommit) -> Result<BisectResult>;

    /// Get tester name
    fn name(&self) -> &str;
}

/// Function-based commit tester
pub struct FunctionTester<F>
where
    F: Fn(&MemoryCommit) -> BisectResult + Send + Sync,
{
    name: String,
    func: F,
}

impl<F> FunctionTester<F>
where
    F: Fn(&MemoryCommit) -> BisectResult + Send + Sync,
{
    /// Create a new function tester
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
        }
    }
}

#[async_trait]
impl<F> CommitTester for FunctionTester<F>
where
    F: Fn(&MemoryCommit) -> BisectResult + Send + Sync,
{
    async fn test_commit(&self, commit: &MemoryCommit) -> Result<BisectResult> {
        Ok((self.func)(commit))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Trace of bisect operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BisectTrace {
    /// Starting good commit
    pub good_commit: String,
    /// Starting bad commit
    pub bad_commit: String,
    /// All tested commits
    pub tested: Vec<TestedCommit>,
    /// Found culprit commit (if any)
    pub culprit: Option<String>,
    /// Total commits examined
    pub commits_examined: usize,
    /// Total commits in range
    pub total_commits: usize,
}

/// A commit that was tested during bisect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestedCommit {
    /// Commit hash
    pub hash: String,
    /// Test result
    pub result: BisectResult,
    /// Commit message
    pub message: String,
}

/// Bisect regression finder
pub struct BisectRegression {
    /// Name for tracing
    name: String,
    /// Memory repository
    repository: Arc<MemoryRepository>,
    /// Commit tester
    tester: Arc<dyn CommitTester>,
}

impl BisectRegression {
    /// Create a new bisect regression finder
    pub fn new(
        name: impl Into<String>,
        repository: Arc<MemoryRepository>,
        tester: Arc<dyn CommitTester>,
    ) -> Self {
        Self {
            name: name.into(),
            repository,
            tester,
        }
    }

    /// Create with a function tester
    pub fn with_function<F>(
        name: impl Into<String>,
        repository: Arc<MemoryRepository>,
        tester_name: impl Into<String>,
        func: F,
    ) -> Self
    where
        F: Fn(&MemoryCommit) -> BisectResult + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            repository,
            tester: Arc::new(FunctionTester::new(tester_name, func)),
        }
    }

    /// Run bisect to find the first bad commit
    ///
    /// # Arguments
    /// * `good_commit` - A known good commit (before regression)
    /// * `bad_commit` - A known bad commit (has regression)
    /// * `branch` - Branch to search in
    pub async fn bisect(
        &self,
        good_commit: &str,
        bad_commit: &str,
        branch: &str,
    ) -> Result<BisectTrace> {
        // Get commit history
        let history = self.repository.get_commit_history(branch, None).await?;

        if history.is_empty() {
            return Err(ThymosError::Memory("No commits in history".to_string()));
        }

        // Find positions of good and bad commits
        let good_idx = history
            .iter()
            .position(|c| c.hash == good_commit)
            .ok_or_else(|| ThymosError::Memory(format!("Good commit {} not found", good_commit)))?;

        let bad_idx = history
            .iter()
            .position(|c| c.hash == bad_commit)
            .ok_or_else(|| ThymosError::Memory(format!("Bad commit {} not found", bad_commit)))?;

        // Good should be older (higher index in history which is newest-first)
        if good_idx <= bad_idx {
            return Err(ThymosError::Configuration(
                "Good commit must be older than bad commit".to_string(),
            ));
        }

        let mut trace = BisectTrace {
            good_commit: good_commit.to_string(),
            bad_commit: bad_commit.to_string(),
            tested: Vec::new(),
            culprit: None,
            commits_examined: 0,
            total_commits: good_idx - bad_idx + 1,
        };

        // Binary search
        let mut left = bad_idx; // Newest (known bad)
        let mut right = good_idx; // Oldest (known good)

        while left < right {
            let mid = left + (right - left) / 2;
            let commit = &history[mid];

            trace.commits_examined += 1;

            let result = self.tester.test_commit(commit).await?;

            trace.tested.push(TestedCommit {
                hash: commit.hash.clone(),
                result,
                message: commit.message.clone(),
            });

            match result {
                BisectResult::Good => {
                    // This commit is good, first bad is between left and mid
                    right = mid;
                }
                BisectResult::Bad => {
                    // This commit is bad, first bad is between mid+1 and right
                    // But if mid == left, we found it
                    if mid == left {
                        trace.culprit = Some(commit.hash.clone());
                        break;
                    }
                    left = mid;
                }
                BisectResult::Skip => {
                    // Can't test this commit, try neighbors
                    // First try moving towards bad (left)
                    if mid > left {
                        right = mid;
                    } else if mid < right {
                        left = mid + 1;
                    } else {
                        // Can't make progress
                        break;
                    }
                }
            }

            // Check if we've converged
            if right - left <= 1 {
                // The culprit is at 'left' (the first bad commit)
                let culprit_commit = &history[left];
                let culprit_result = self.tester.test_commit(culprit_commit).await?;

                if culprit_result == BisectResult::Bad {
                    trace.culprit = Some(culprit_commit.hash.clone());
                }
                break;
            }
        }

        Ok(trace)
    }

    /// Get details about the culprit commit
    pub async fn get_culprit_details(&self, trace: &BisectTrace) -> Result<Option<MemoryCommit>> {
        if let Some(ref hash) = trace.culprit {
            self.repository.get_commit(hash).await
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bisect_result() {
        assert_eq!(BisectResult::Good, BisectResult::Good);
        assert_ne!(BisectResult::Good, BisectResult::Bad);
    }

    #[test]
    fn test_tested_commit() {
        let tested = TestedCommit {
            hash: "abc123".to_string(),
            result: BisectResult::Bad,
            message: "Broke something".to_string(),
        };

        assert_eq!(tested.result, BisectResult::Bad);
    }

    #[test]
    fn test_bisect_trace() {
        let trace = BisectTrace {
            good_commit: "good123".to_string(),
            bad_commit: "bad456".to_string(),
            tested: vec![],
            culprit: Some("culprit789".to_string()),
            commits_examined: 5,
            total_commits: 10,
        };

        assert!(trace.culprit.is_some());
        assert_eq!(trace.commits_examined, 5);
    }

    #[tokio::test]
    async fn test_function_tester() {
        let tester = FunctionTester::new("test", |commit| {
            if commit.message.contains("bad") {
                BisectResult::Bad
            } else {
                BisectResult::Good
            }
        });

        let commit = MemoryCommit {
            hash: "test".to_string(),
            message: "This is bad".to_string(),
            author: "test".to_string(),
            timestamp: chrono::Utc::now(),
            parent_commits: vec![],
            snapshot_id: "snap".to_string(),
        };

        let result = tester.test_commit(&commit).await.unwrap();
        assert_eq!(result, BisectResult::Bad);
    }
}
