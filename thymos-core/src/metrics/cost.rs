//! LLM cost calculation

use crate::llm::{ModelInfo, TokenUsage};
use std::collections::HashMap;

/// Cost per 1M tokens for different providers/models
/// Prices are approximate and may vary by region/time
pub struct LLMCostCalculator {
    /// Cost per 1M input tokens by provider/model
    input_costs: HashMap<String, f64>,
    /// Cost per 1M output tokens by provider/model
    output_costs: HashMap<String, f64>,
}

impl LLMCostCalculator {
    /// Create a new cost calculator with default pricing
    pub fn new() -> Self {
        let mut input_costs = HashMap::new();
        let mut output_costs = HashMap::new();

        // OpenAI pricing (as of 2024, approximate)
        input_costs.insert("openai:gpt-4".to_string(), 30.0); // $30 per 1M tokens
        output_costs.insert("openai:gpt-4".to_string(), 60.0);
        input_costs.insert("openai:gpt-4-turbo".to_string(), 10.0);
        output_costs.insert("openai:gpt-4-turbo".to_string(), 30.0);
        input_costs.insert("openai:gpt-3.5-turbo".to_string(), 0.5);
        output_costs.insert("openai:gpt-3.5-turbo".to_string(), 1.5);

        // Anthropic pricing (as of 2024, approximate)
        input_costs.insert("anthropic:claude-3-opus".to_string(), 15.0);
        output_costs.insert("anthropic:claude-3-opus".to_string(), 75.0);
        input_costs.insert("anthropic:claude-3-sonnet".to_string(), 3.0);
        output_costs.insert("anthropic:claude-3-sonnet".to_string(), 15.0);
        input_costs.insert("anthropic:claude-3-haiku".to_string(), 0.25);
        output_costs.insert("anthropic:claude-3-haiku".to_string(), 1.25);

        // Groq pricing (as of 2024, approximate)
        input_costs.insert("groq:llama-3-70b".to_string(), 0.59);
        output_costs.insert("groq:llama-3-70b".to_string(), 0.79);

        Self {
            input_costs,
            output_costs,
        }
    }

    /// Calculate cost for token usage
    pub fn calculate_cost(
        &self,
        model_info: &ModelInfo,
        usage: &TokenUsage,
    ) -> f64 {
        let key = format!("{}:{}", model_info.provider, model_info.model_name);
        
        let input_cost_per_million = self.input_costs.get(&key)
            .or_else(|| self.input_costs.get(&format!("{}:*", model_info.provider)))
            .copied()
            .unwrap_or(1.0); // Default $1 per 1M tokens if unknown
        
        let output_cost_per_million = self.output_costs.get(&key)
            .or_else(|| self.output_costs.get(&format!("{}:*", model_info.provider)))
            .copied()
            .unwrap_or(2.0); // Default $2 per 1M tokens if unknown

        let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * input_cost_per_million;
        let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * output_cost_per_million;

        input_cost + output_cost
    }

    /// Add or update pricing for a model
    pub fn set_pricing(
        &mut self,
        provider: &str,
        model: &str,
        input_cost_per_million: f64,
        output_cost_per_million: f64,
    ) {
        let key = format!("{}:{}", provider, model);
        self.input_costs.insert(key.clone(), input_cost_per_million);
        self.output_costs.insert(key, output_cost_per_million);
    }
}

impl Default for LLMCostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

