//! Memory consolidation engine for analyzing and extracting insights.
//!
//! This module implements the consolidation engine that periodically processes
//! memories to generate insights, identify patterns, and update importance scores.

pub mod config;
pub mod engine;
pub mod insights;

pub use config::ConsolidationConfig;
pub use engine::ConsolidationEngine;
pub use insights::{Insight, InsightType};

// Re-export LLM types from top-level llm module
pub use crate::llm::{
    LLMConfig, LLMProvider, LLMProviderFactory, LLMRequest, LLMResponse, Message, MessageRole,
    ModelInfo, TokenUsage,
};

pub mod prelude {
    pub use crate::consolidation::{
        ConsolidationConfig, ConsolidationEngine, Insight, InsightType, LLMConfig, LLMProvider,
    };
}
