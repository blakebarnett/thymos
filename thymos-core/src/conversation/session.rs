//! Conversation Session

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::llm::Message;

use super::history::MessageHistory;
use super::truncation::TruncationStrategy;

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session is active
    Active,
    /// Session is paused (can be resumed)
    Paused,
    /// Session has ended
    Ended,
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// When the session was created
    pub created_at: SystemTime,
    /// When the session was last updated
    pub updated_at: SystemTime,
    /// Total turn count
    pub turn_count: usize,
    /// Custom metadata
    pub custom: std::collections::HashMap<String, String>,
}

impl SessionMetadata {
    /// Create new metadata
    pub fn new() -> Self {
        let now = SystemTime::now();
        Self {
            created_at: now,
            updated_at: now,
            turn_count: 0,
            custom: std::collections::HashMap::new(),
        }
    }

    /// Update the metadata
    pub fn touch(&mut self) {
        self.updated_at = SystemTime::now();
    }

    /// Set a custom metadata field
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.custom.insert(key.into(), value.into());
        self.touch();
    }

    /// Get a custom metadata field
    pub fn get(&self, key: &str) -> Option<&str> {
        self.custom.get(key).map(|s| s.as_str())
    }
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    /// Unique session ID
    id: String,
    /// Session state
    state: SessionState,
    /// Message history
    history: MessageHistory,
    /// Session metadata
    metadata: SessionMetadata,
    /// Maximum tokens for context window
    max_context_tokens: Option<usize>,
}

impl ConversationSession {
    /// Create a new session
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: SessionState::Active,
            history: MessageHistory::new(),
            metadata: SessionMetadata::new(),
            max_context_tokens: None,
        }
    }

    /// Create with a system prompt
    pub fn with_system_prompt(id: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: SessionState::Active,
            history: MessageHistory::with_system_prompt(system_prompt),
            metadata: SessionMetadata::new(),
            max_context_tokens: None,
        }
    }

    /// Create with a maximum context window
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_context_tokens = Some(max_tokens);
        self
    }

    /// Get the session ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the session state
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get the message history
    pub fn history(&self) -> &MessageHistory {
        &self.history
    }

    /// Get mutable reference to history
    pub fn history_mut(&mut self) -> &mut MessageHistory {
        &mut self.history
    }

    /// Get session metadata
    pub fn metadata(&self) -> &SessionMetadata {
        &self.metadata
    }

    /// Get mutable reference to metadata
    pub fn metadata_mut(&mut self) -> &mut SessionMetadata {
        &mut self.metadata
    }

    /// Get turn count
    pub fn turn_count(&self) -> usize {
        self.history.turn_count()
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.history.set_system_prompt(prompt);
        self.metadata.touch();
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: impl Into<String>) -> usize {
        if self.state != SessionState::Active {
            return self.history.turn_count();
        }

        let turn_index = self.history.add_user_message(content);
        self.metadata.turn_count = self.history.turn_count();
        self.metadata.touch();
        turn_index
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: impl Into<String>) -> bool {
        if self.state != SessionState::Active {
            return false;
        }

        let result = self.history.add_assistant_message(content);
        self.metadata.touch();
        result
    }

    /// Get messages for LLM context, applying truncation if needed
    pub fn get_messages(&self) -> Vec<Message> {
        self.history.to_messages()
    }

    /// Get messages with a specific truncation strategy
    pub fn get_messages_truncated<T: TruncationStrategy>(
        &self,
        strategy: &T,
        max_tokens: usize,
    ) -> Vec<Message> {
        strategy.truncate(&self.history, max_tokens)
    }

    /// Pause the session
    pub fn pause(&mut self) {
        if self.state == SessionState::Active {
            self.state = SessionState::Paused;
            self.metadata.touch();
        }
    }

    /// Resume the session
    pub fn resume(&mut self) {
        if self.state == SessionState::Paused {
            self.state = SessionState::Active;
            self.metadata.touch();
        }
    }

    /// End the session
    pub fn end(&mut self) {
        self.state = SessionState::Ended;
        self.metadata.touch();
    }

    /// Clear the session history (keeps metadata)
    pub fn clear(&mut self) {
        self.history.clear();
        self.metadata.turn_count = 0;
        self.metadata.touch();
    }

    /// Serialize to JSON for persistence
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = ConversationSession::new("test-session");
        assert_eq!(session.id(), "test-session");
        assert!(session.is_active());
        assert_eq!(session.turn_count(), 0);
    }

    #[test]
    fn test_session_with_system_prompt() {
        let session = ConversationSession::with_system_prompt("test", "You are helpful");
        assert_eq!(session.history().system_prompt(), Some("You are helpful"));
    }

    #[test]
    fn test_session_messages() {
        let mut session = ConversationSession::new("test");

        session.add_user_message("Hello");
        session.add_assistant_message("Hi!");
        session.add_user_message("How are you?");
        session.add_assistant_message("I'm doing well!");

        assert_eq!(session.turn_count(), 2);

        let messages = session.get_messages();
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = ConversationSession::new("test");

        assert_eq!(session.state(), SessionState::Active);

        session.pause();
        assert_eq!(session.state(), SessionState::Paused);

        // Can't add messages when paused
        session.add_user_message("Hello");
        assert_eq!(session.turn_count(), 0);

        session.resume();
        assert_eq!(session.state(), SessionState::Active);

        session.add_user_message("Hello");
        assert_eq!(session.turn_count(), 1);

        session.end();
        assert_eq!(session.state(), SessionState::Ended);
    }

    #[test]
    fn test_session_metadata() {
        let mut session = ConversationSession::new("test");

        session.metadata_mut().set("user_id", "123");
        assert_eq!(session.metadata().get("user_id"), Some("123"));
    }

    #[test]
    fn test_session_serialization() {
        let mut session = ConversationSession::new("test");
        session.add_user_message("Hello");
        session.add_assistant_message("Hi!");

        let json = session.to_json().unwrap();
        let restored = ConversationSession::from_json(&json).unwrap();

        assert_eq!(restored.id(), session.id());
        assert_eq!(restored.turn_count(), session.turn_count());
    }

    #[test]
    fn test_session_clear() {
        let mut session = ConversationSession::new("test");
        session.add_user_message("Hello");
        session.add_assistant_message("Hi!");

        assert_eq!(session.turn_count(), 1);

        session.clear();
        assert_eq!(session.turn_count(), 0);
        assert!(session.is_active());
    }
}
