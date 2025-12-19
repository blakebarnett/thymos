//! Message History Management

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::llm::{Message, MessageRole};

/// A single turn in the conversation (user message + assistant response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Turn index (0-indexed)
    pub index: usize,
    /// User message
    pub user_message: String,
    /// Assistant response (None if not yet responded)
    pub assistant_message: Option<String>,
    /// When the turn started
    pub started_at: SystemTime,
    /// When the turn completed
    pub completed_at: Option<SystemTime>,
}

impl Turn {
    /// Create a new turn
    pub fn new(index: usize, user_message: impl Into<String>) -> Self {
        Self {
            index,
            user_message: user_message.into(),
            assistant_message: None,
            started_at: SystemTime::now(),
            completed_at: None,
        }
    }

    /// Complete the turn with an assistant response
    pub fn complete(&mut self, assistant_message: impl Into<String>) {
        self.assistant_message = Some(assistant_message.into());
        self.completed_at = Some(SystemTime::now());
    }

    /// Check if the turn is complete
    pub fn is_complete(&self) -> bool {
        self.assistant_message.is_some()
    }

    /// Convert to LLM messages
    pub fn to_messages(&self) -> Vec<Message> {
        let mut messages = vec![Message {
            role: MessageRole::User,
            content: self.user_message.clone(),
        }];

        if let Some(ref response) = self.assistant_message {
            messages.push(Message {
                role: MessageRole::Assistant,
                content: response.clone(),
            });
        }

        messages
    }

    /// Estimate token count (rough approximation: 4 chars per token)
    pub fn estimate_tokens(&self) -> usize {
        let user_tokens = self.user_message.len() / 4;
        let assistant_tokens = self
            .assistant_message
            .as_ref()
            .map(|m| m.len() / 4)
            .unwrap_or(0);
        user_tokens + assistant_tokens
    }
}

/// Message history for a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    /// System prompt (if any)
    system_prompt: Option<String>,
    /// Conversation turns
    turns: Vec<Turn>,
}

impl MessageHistory {
    /// Create a new message history
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            turns: Vec::new(),
        }
    }

    /// Create with a system prompt
    pub fn with_system_prompt(prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: Some(prompt.into()),
            turns: Vec::new(),
        }
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = Some(prompt.into());
    }

    /// Get the system prompt
    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// Add a user message (starts a new turn)
    pub fn add_user_message(&mut self, content: impl Into<String>) -> usize {
        let index = self.turns.len();
        self.turns.push(Turn::new(index, content));
        index
    }

    /// Add an assistant message (completes current turn)
    pub fn add_assistant_message(&mut self, content: impl Into<String>) -> bool {
        if let Some(turn) = self.turns.last_mut() {
            if !turn.is_complete() {
                turn.complete(content);
                return true;
            }
        }
        false
    }

    /// Get all turns
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    /// Get turn count
    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }

    /// Get the last N turns
    pub fn last_turns(&self, n: usize) -> &[Turn] {
        let start = self.turns.len().saturating_sub(n);
        &self.turns[start..]
    }

    /// Get turns in a range
    pub fn turns_range(&self, start: usize, end: usize) -> &[Turn] {
        let start = start.min(self.turns.len());
        let end = end.min(self.turns.len());
        &self.turns[start..end]
    }

    /// Convert to LLM messages
    pub fn to_messages(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        if let Some(ref system) = self.system_prompt {
            messages.push(Message {
                role: MessageRole::System,
                content: system.clone(),
            });
        }

        for turn in &self.turns {
            messages.extend(turn.to_messages());
        }

        messages
    }

    /// Convert specific turns to LLM messages
    pub fn turns_to_messages(&self, turns: &[Turn]) -> Vec<Message> {
        let mut messages = Vec::new();

        if let Some(ref system) = self.system_prompt {
            messages.push(Message {
                role: MessageRole::System,
                content: system.clone(),
            });
        }

        for turn in turns {
            messages.extend(turn.to_messages());
        }

        messages
    }

    /// Estimate total token count
    pub fn estimate_tokens(&self) -> usize {
        let system_tokens = self.system_prompt.as_ref().map(|s| s.len() / 4).unwrap_or(0);
        let turn_tokens: usize = self.turns.iter().map(|t| t.estimate_tokens()).sum();
        system_tokens + turn_tokens
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.turns.clear();
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_creation() {
        let turn = Turn::new(0, "Hello");
        assert_eq!(turn.index, 0);
        assert_eq!(turn.user_message, "Hello");
        assert!(!turn.is_complete());
    }

    #[test]
    fn test_turn_completion() {
        let mut turn = Turn::new(0, "Hello");
        turn.complete("Hi there!");
        assert!(turn.is_complete());
        assert_eq!(turn.assistant_message.as_deref(), Some("Hi there!"));
    }

    #[test]
    fn test_turn_to_messages() {
        let mut turn = Turn::new(0, "Hello");
        turn.complete("Hi!");

        let messages = turn.to_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_message_history_basic() {
        let mut history = MessageHistory::new();

        history.add_user_message("Hello");
        history.add_assistant_message("Hi!");

        assert_eq!(history.turn_count(), 1);
        assert!(history.turns()[0].is_complete());
    }

    #[test]
    fn test_message_history_with_system() {
        let mut history = MessageHistory::with_system_prompt("You are helpful");

        history.add_user_message("Hello");
        history.add_assistant_message("Hi!");

        let messages = history.to_messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_last_turns() {
        let mut history = MessageHistory::new();

        for i in 0..5 {
            history.add_user_message(format!("Message {}", i));
            history.add_assistant_message(format!("Response {}", i));
        }

        let last_2 = history.last_turns(2);
        assert_eq!(last_2.len(), 2);
        assert_eq!(last_2[0].index, 3);
        assert_eq!(last_2[1].index, 4);
    }

    #[test]
    fn test_estimate_tokens() {
        let mut history = MessageHistory::new();
        history.add_user_message("Hello world"); // ~3 tokens
        history.add_assistant_message("Hi there!"); // ~2 tokens

        let estimate = history.estimate_tokens();
        assert!(estimate > 0);
    }
}
