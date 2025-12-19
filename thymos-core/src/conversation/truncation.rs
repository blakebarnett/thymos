//! Context Window Truncation Strategies

use crate::llm::Message;

use super::history::MessageHistory;

/// Trait for truncation strategies
pub trait TruncationStrategy: Send + Sync {
    /// Truncate history to fit within token limit
    fn truncate(&self, history: &MessageHistory, max_tokens: usize) -> Vec<Message>;

    /// Get the strategy name
    fn name(&self) -> &'static str;
}

/// Sliding window strategy - keeps the most recent messages
#[derive(Debug, Clone)]
pub struct SlidingWindowStrategy {
    /// Minimum number of turns to keep
    min_turns: usize,
}

impl SlidingWindowStrategy {
    /// Create a new sliding window strategy
    pub fn new() -> Self {
        Self { min_turns: 1 }
    }

    /// Set minimum turns to keep
    pub fn with_min_turns(mut self, min: usize) -> Self {
        self.min_turns = min.max(1);
        self
    }
}

impl Default for SlidingWindowStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl TruncationStrategy for SlidingWindowStrategy {
    fn truncate(&self, history: &MessageHistory, max_tokens: usize) -> Vec<Message> {
        let turns = history.turns();

        if turns.is_empty() {
            return history.turns_to_messages(&[]);
        }

        // Start with all turns and remove from the beginning until we fit
        let mut start_index = 0;
        let max_start = turns.len().saturating_sub(self.min_turns);

        // Estimate system prompt tokens
        let system_tokens = history
            .system_prompt()
            .map(|s| s.len() / 4)
            .unwrap_or(0);

        while start_index < max_start {
            let remaining_turns = &turns[start_index..];
            let turn_tokens: usize = remaining_turns.iter().map(|t| t.estimate_tokens()).sum();

            if system_tokens + turn_tokens <= max_tokens {
                break;
            }

            start_index += 1;
        }

        history.turns_to_messages(&turns[start_index..])
    }

    fn name(&self) -> &'static str {
        "sliding_window"
    }
}

/// First N + Last M strategy - keeps first N and last M turns
#[derive(Debug, Clone)]
pub struct FirstLastStrategy {
    /// Number of turns to keep from the start
    first_n: usize,
    /// Number of turns to keep from the end
    last_m: usize,
}

impl FirstLastStrategy {
    /// Create a new first+last strategy
    pub fn new(first_n: usize, last_m: usize) -> Self {
        Self {
            first_n: first_n.max(1),
            last_m: last_m.max(1),
        }
    }
}

impl TruncationStrategy for FirstLastStrategy {
    fn truncate(&self, history: &MessageHistory, max_tokens: usize) -> Vec<Message> {
        let turns = history.turns();

        if turns.is_empty() {
            return history.turns_to_messages(&[]);
        }

        let total_turns = turns.len();

        if total_turns <= self.first_n + self.last_m {
            // No truncation needed
            return history.to_messages();
        }

        // Collect first N and last M turns
        let first_turns: Vec<_> = turns.iter().take(self.first_n).cloned().collect();
        let last_turns: Vec<_> = turns.iter().skip(total_turns - self.last_m).cloned().collect();

        let mut combined: Vec<_> = first_turns;
        combined.extend(last_turns);

        // Check if we fit, otherwise reduce
        let system_tokens = history
            .system_prompt()
            .map(|s| s.len() / 4)
            .unwrap_or(0);

        let turn_tokens: usize = combined.iter().map(|t| t.estimate_tokens()).sum();

        if system_tokens + turn_tokens <= max_tokens {
            return history.turns_to_messages(&combined);
        }

        // Fall back to just the last M if we don't fit
        history.turns_to_messages(&turns[total_turns - self.last_m..])
    }

    fn name(&self) -> &'static str {
        "first_last"
    }
}

/// Summary strategy - summarizes older turns
#[derive(Debug, Clone)]
pub struct SummaryStrategy {
    /// Number of recent turns to keep verbatim
    recent_turns: usize,
    /// Summary of older turns
    summary: Option<String>,
}

impl SummaryStrategy {
    /// Create a new summary strategy
    pub fn new(recent_turns: usize) -> Self {
        Self {
            recent_turns: recent_turns.max(1),
            summary: None,
        }
    }

    /// Set the summary of older turns
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Update the summary
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = Some(summary.into());
    }

    /// Clear the summary
    pub fn clear_summary(&mut self) {
        self.summary = None;
    }

    /// Get the current summary
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

impl TruncationStrategy for SummaryStrategy {
    fn truncate(&self, history: &MessageHistory, max_tokens: usize) -> Vec<Message> {
        let turns = history.turns();
        let mut messages = Vec::new();

        // Add system prompt
        if let Some(system) = history.system_prompt() {
            messages.push(Message {
                role: crate::llm::MessageRole::System,
                content: system.to_string(),
            });
        }

        // Add summary as a system message if we have one and there are older turns
        if let Some(ref summary) = self.summary {
            if turns.len() > self.recent_turns {
                messages.push(Message {
                    role: crate::llm::MessageRole::System,
                    content: format!("Summary of earlier conversation:\n{}", summary),
                });
            }
        }

        // Add recent turns
        let start = turns.len().saturating_sub(self.recent_turns);
        for turn in &turns[start..] {
            messages.extend(turn.to_messages());
        }

        // Check if we fit within max_tokens
        let total_tokens: usize = messages.iter().map(|m| m.content.len() / 4).sum();
        if total_tokens > max_tokens && messages.len() > 2 {
            // Remove some messages from the middle if too long
            let to_remove = (total_tokens - max_tokens) / 50 + 1;
            for _ in 0..to_remove.min(messages.len() - 2) {
                if messages.len() > 2 {
                    messages.remove(1);
                }
            }
        }

        messages
    }

    fn name(&self) -> &'static str {
        "summary"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_history(turn_count: usize) -> MessageHistory {
        let mut history = MessageHistory::with_system_prompt("You are helpful");
        for i in 0..turn_count {
            history.add_user_message(format!("Message {}", i));
            history.add_assistant_message(format!("Response {}", i));
        }
        history
    }

    #[test]
    fn test_sliding_window_no_truncation() {
        let history = create_history(3);
        let strategy = SlidingWindowStrategy::new();

        let messages = strategy.truncate(&history, 10000);
        // System + 3 turns * 2 messages = 7 messages
        assert_eq!(messages.len(), 7);
    }

    #[test]
    fn test_sliding_window_with_truncation() {
        let history = create_history(10);
        let strategy = SlidingWindowStrategy::new().with_min_turns(2);

        // Very small token limit should still keep min_turns
        let messages = strategy.truncate(&history, 50);
        // Should have at least system + 2 turns * 2 = 5 messages
        assert!(messages.len() >= 5);
    }

    #[test]
    fn test_first_last_strategy() {
        let history = create_history(10);
        let strategy = FirstLastStrategy::new(2, 2);

        let messages = strategy.truncate(&history, 10000);
        // System + 4 turns * 2 = 9 messages
        assert_eq!(messages.len(), 9);
    }

    #[test]
    fn test_first_last_no_truncation_needed() {
        let history = create_history(3);
        let strategy = FirstLastStrategy::new(2, 2);

        let messages = strategy.truncate(&history, 10000);
        // System + 3 turns * 2 = 7 messages (all kept)
        assert_eq!(messages.len(), 7);
    }

    #[test]
    fn test_summary_strategy() {
        let history = create_history(5);
        let strategy = SummaryStrategy::new(2)
            .with_summary("Earlier we discussed greetings");

        let messages = strategy.truncate(&history, 10000);

        // Should have: system, summary, + 2 turns * 2 = 6 messages
        assert!(messages.len() >= 5);

        // Check that summary is included
        let has_summary = messages
            .iter()
            .any(|m| m.content.contains("Summary of earlier"));
        assert!(has_summary);
    }

    #[test]
    fn test_summary_strategy_no_summary_when_few_turns() {
        let history = create_history(2);
        let strategy = SummaryStrategy::new(2).with_summary("Summary");

        let messages = strategy.truncate(&history, 10000);

        // No summary should be included when turns <= recent_turns
        let has_summary = messages
            .iter()
            .any(|m| m.content.contains("Summary of earlier"));
        assert!(!has_summary);
    }

    #[test]
    fn test_strategy_names() {
        assert_eq!(SlidingWindowStrategy::new().name(), "sliding_window");
        assert_eq!(FirstLastStrategy::new(1, 1).name(), "first_last");
        assert_eq!(SummaryStrategy::new(1).name(), "summary");
    }
}
