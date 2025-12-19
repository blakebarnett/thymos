//! Conversation Session Management
//!
//! Multi-turn conversation handling with history, truncation, and persistence.
//!
//! # Features
//!
//! - Session lifecycle (active, paused, ended)
//! - Message history with turn tracking
//! - Context window truncation strategies
//! - Optional persistence for session recovery
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::conversation::{ConversationSession, TruncationStrategy};
//!
//! let mut session = ConversationSession::new("session-1");
//! session.add_user_message("Hello!");
//! session.add_assistant_message("Hi there! How can I help?");
//!
//! let messages = session.get_messages_for_context(4096);
//! ```

mod history;
mod session;
mod truncation;

pub use history::{MessageHistory, Turn};
pub use session::{ConversationSession, SessionMetadata, SessionState};
pub use truncation::{FirstLastStrategy, SlidingWindowStrategy, SummaryStrategy, TruncationStrategy};
