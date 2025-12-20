//! Context Manager - Integrated context window management
//!
//! A high-level API that integrates conversation session management, memory grounding,
//! automatic summarization, quality monitoring, and version-based rollback to prevent
//! "context rot" in long-running agents.
//!
//! # Features
//!
//! - Automatic context summarization when token thresholds are reached
//! - Memory grounding for relevant context retrieval
//! - Quality scoring (heuristic and optional LLM-based)
//! - Checkpoint/rollback for recovery from context degradation
//! - Session snapshot preservation for timeline integrity
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::context::{ContextManager, ContextConfig};
//!
//! let mut ctx = agent.context_manager(ContextConfig {
//!     max_tokens: 4096,
//!     summarize_at_ratio: 0.7,
//!     checkpoint_interval: 5,
//!     ..Default::default()
//! });
//!
//! // Process turns with automatic management
//! let result = ctx.process_turn("What did we discuss earlier?").await?;
//!
//! if result.summarization_triggered {
//!     println!("Context was summarized to fit token limit");
//! }
//!
//! // Get grounded context for LLM
//! let grounded = ctx.ground_query("relevant topic").await?;
//! ```

mod config;
mod manager;
mod quality;

pub use config::ContextConfig;
pub use manager::{
    ContextManager, ContextTurnResult, GroundedContext, RollbackResult, SummarizationResult,
};
pub use quality::{ContextState, QualityScorer, QualityScorerConfig};
