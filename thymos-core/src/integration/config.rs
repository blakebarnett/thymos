//! Configuration for ThymosAgentCore
//!
//! Provides configuration options for Thymos agents when integrating
//! with AutoAgents execution patterns.

use serde::{Deserialize, Serialize};

/// Configuration for ThymosAgentCore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThymosAgentConfig {
    /// Maximum execution turns before stopping
    pub max_turns: usize,

    /// Whether to store tool results in memory
    pub store_tool_results: bool,

    /// Whether to store LLM responses in memory
    pub store_llm_responses: bool,

    /// Whether to enable replay capture
    pub enable_replay_capture: bool,

    /// Whether to log verbose execution information
    pub verbose: bool,
}

impl Default for ThymosAgentConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            store_tool_results: false,
            store_llm_responses: false,
            enable_replay_capture: false,
            verbose: false,
        }
    }
}

impl ThymosAgentConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max turns
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Enable storing tool results in memory
    pub fn with_tool_result_storage(mut self, enabled: bool) -> Self {
        self.store_tool_results = enabled;
        self
    }

    /// Enable storing LLM responses in memory
    pub fn with_llm_response_storage(mut self, enabled: bool) -> Self {
        self.store_llm_responses = enabled;
        self
    }

    /// Enable replay capture
    pub fn with_replay_capture(mut self, enabled: bool) -> Self {
        self.enable_replay_capture = enabled;
        self
    }

    /// Enable verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ThymosAgentConfig::default();
        assert_eq!(config.max_turns, 10);
        assert!(!config.store_tool_results);
        assert!(!config.store_llm_responses);
        assert!(!config.enable_replay_capture);
        assert!(!config.verbose);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ThymosAgentConfig::new()
            .with_max_turns(20)
            .with_tool_result_storage(true)
            .with_replay_capture(true)
            .with_verbose(true);

        assert_eq!(config.max_turns, 20);
        assert!(config.store_tool_results);
        assert!(config.enable_replay_capture);
        assert!(config.verbose);
    }
}


