//! Replay record system for agent execution
//!
//! This module provides infrastructure to record agent runs for:
//! - Debugging: Replay past executions to understand behavior
//! - Evaluation: Compare outputs across runs
//! - Regression testing: Detect changes in behavior
//!
//! # Architecture
//!
//! The replay system uses an append-only event log with typed events.
//! Each event captures a specific action (LLM call, tool invocation,
//! memory operation, etc.) with full provenance.
//!
//! # Example
//!
//! ```rust,no_run
//! use thymos_core::replay::{ReplayRecorder, ReplayEvent};
//!
//! // Create a recorder for a session
//! let mut recorder = ReplayRecorder::new("session_123");
//!
//! // Events are automatically captured via hooks
//! // Or manually emit events:
//! recorder.emit(ReplayEvent::SessionStart { ... });
//!
//! // Save the record
//! let record = recorder.finish();
//! record.save("replay_session_123.jsonl")?;
//! ```

mod capture;
mod record;

pub use capture::{ReplayCapture, ReplayCaptureHooks, RecordingMode};
pub use record::{
    LlmCallEvent, MemoryRetrievalEvent, MemoryVersioningEvent, ReplayEvent, ReplayEventEnvelope,
    ReplayRecord, SessionEvent, ToolCallEvent, ToolCallStatus, VersioningOperation,
};

/// Current schema version for replay records
pub const REPLAY_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests;

