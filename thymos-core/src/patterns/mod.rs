//! Versioning-Powered Patterns
//!
//! Advanced agent patterns leveraging git-style memory versioning for
//! isolation, speculation, and consensus.
//!
//! # Patterns
//!
//! - **SpeculativeExecution**: Try multiple approaches in parallel branches, commit best
//! - **ParallelWithIsolation**: Concurrent execution with memory isolation per branch
//! - **ConsensusMerge**: Multi-agent consensus via voting or LLM-assisted merge
//! - **BisectRegression**: Binary search through memory history for debugging
//!
//! # Workflow Integration
//!
//! All patterns can be used as workflow steps and composed with Chain, Router, etc.
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::patterns::{SpeculativeExecution, SpeculativeEvaluator};
//!
//! let spec = SpeculativeExecution::new(repo)
//!     .with_evaluator(MyEvaluator)
//!     .add_approach("conservative", conservative_step)
//!     .add_approach("aggressive", aggressive_step)
//!     .build();
//!
//! let (result, trace) = spec.execute(input).await?;
//! ```

mod evaluator;
mod speculative;
mod parallel_isolated;
mod consensus;
mod bisect;

pub use evaluator::{ApproachEvaluator, ApproachResult, EvaluationScore, LLMApproachEvaluator};
pub use speculative::{SpeculativeExecution, SpeculativeExecutionBuilder, SpeculativeTrace};
pub use parallel_isolated::{ParallelIsolated, ParallelIsolatedBuilder, IsolatedBranchResult};
pub use consensus::{ConsensusMerge, ConsensusConfig, ConsensusResult, ConsensusStrategy};
pub use bisect::{BisectRegression, BisectResult, BisectTrace};
