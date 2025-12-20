//! Context Manager Implementation
//!
//! The core ContextManager that integrates session management, memory grounding,
//! automatic summarization, and checkpoint/rollback functionality.

use crate::conversation::{ConversationSession, SummaryStrategy};
use crate::error::{Result, ThymosError};
use crate::llm::{LLMProvider, LLMRequest, Message, MessageRole};
use crate::memory::versioning::MemoryRepository;
use crate::memory::MemorySystem;
use locai::prelude::Memory;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::config::ContextConfig;
use super::quality::{ContextState, QualityScorer, QualityScorerConfig};

/// Result from processing a conversation turn
#[derive(Debug)]
pub struct ContextTurnResult {
    /// Memory IDs used for grounding this turn
    pub grounding_used: Vec<String>,

    /// Whether summarization was triggered
    pub summarization_triggered: bool,

    /// Commit hash if a checkpoint was created
    pub checkpoint_created: Option<String>,

    /// Current quality score
    pub quality_score: f64,

    /// Current token count estimate
    pub token_count: usize,

    /// Whether rollback was triggered due to quality degradation
    pub rollback_triggered: bool,
}

/// Grounded context for LLM responses
#[derive(Debug)]
pub struct GroundedContext {
    /// Retrieved memories relevant to the query
    pub memories: Vec<Memory>,

    /// Summary of retrieved memories
    pub summary: String,

    /// Relevance scores for each memory
    pub relevance_scores: Vec<f64>,
}

/// Result from summarization operation
#[derive(Debug)]
pub struct SummarizationResult {
    /// Token count before summarization
    pub tokens_before: usize,

    /// Token count after summarization
    pub tokens_after: usize,

    /// Number of turns that were summarized
    pub turns_summarized: usize,

    /// The generated summary
    pub summary: String,
}

impl SummarizationResult {
    /// Create a no-op result when summarization wasn't needed
    pub fn no_op() -> Self {
        Self {
            tokens_before: 0,
            tokens_after: 0,
            turns_summarized: 0,
            summary: String::new(),
        }
    }
}

/// Result from rollback operation
#[derive(Debug)]
pub struct RollbackResult {
    /// Commit hash that was restored
    pub commit_restored: String,

    /// Number of turns lost in the rollback
    pub turns_lost: usize,
}

/// Session snapshot for checkpoint restoration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionSnapshot {
    /// Serialized session state
    session_json: String,

    /// Turn count at snapshot time
    turn_count: usize,

    /// Quality score at snapshot time
    quality_score: f64,
}

/// Checkpoint containing both memory commit and session state
#[derive(Debug, Clone)]
struct Checkpoint {
    /// Memory repository commit hash
    commit_hash: String,

    /// Session snapshot at this checkpoint
    session_snapshot: SessionSnapshot,
}

/// High-level context manager integrating session, memory, and versioning
pub struct ContextManager {
    /// Conversation session
    session: ConversationSession,

    /// Memory system for grounding
    memory: Arc<MemorySystem>,

    /// Memory repository for versioning (optional)
    repo: Option<Arc<MemoryRepository>>,

    /// LLM provider for summarization and coherence evaluation
    llm: Arc<dyn LLMProvider>,

    /// Configuration
    config: ContextConfig,

    /// Quality scorer
    scorer: QualityScorer,

    /// Current context state for quality tracking
    state: ContextState,

    /// Summary strategy for truncation
    summary_strategy: SummaryStrategy,

    /// Turns since last checkpoint
    turns_since_checkpoint: usize,

    /// Checkpoint history (commit hash -> session snapshot)
    checkpoints: Vec<Checkpoint>,
}

impl ContextManager {
    /// Create new context manager without versioning support
    pub fn new(
        session_id: impl Into<String>,
        memory: Arc<MemorySystem>,
        llm: Arc<dyn LLMProvider>,
        config: ContextConfig,
    ) -> Self {
        let state = ContextState::new(config.max_tokens);
        let scorer = QualityScorer::new(QualityScorerConfig::default());
        let summary_strategy = SummaryStrategy::new(config.recent_turns_to_keep);

        Self {
            session: ConversationSession::new(session_id),
            memory,
            repo: None,
            llm,
            config,
            scorer,
            state,
            summary_strategy,
            turns_since_checkpoint: 0,
            checkpoints: Vec::new(),
        }
    }

    /// Create context manager with versioning support (enables rollback)
    pub fn with_versioning(
        session_id: impl Into<String>,
        memory: Arc<MemorySystem>,
        repo: Arc<MemoryRepository>,
        llm: Arc<dyn LLMProvider>,
        config: ContextConfig,
    ) -> Self {
        let mut manager = Self::new(session_id, memory, llm, config);
        manager.repo = Some(repo);
        manager
    }

    /// Create context manager with a system prompt
    pub fn with_system_prompt(
        session_id: impl Into<String>,
        system_prompt: impl Into<String>,
        memory: Arc<MemorySystem>,
        llm: Arc<dyn LLMProvider>,
        config: ContextConfig,
    ) -> Self {
        let state = ContextState::new(config.max_tokens);
        let scorer = QualityScorer::new(QualityScorerConfig::default());
        let summary_strategy = SummaryStrategy::new(config.recent_turns_to_keep);

        Self {
            session: ConversationSession::with_system_prompt(session_id, system_prompt),
            memory,
            repo: None,
            llm,
            config,
            scorer,
            state,
            summary_strategy,
            turns_since_checkpoint: 0,
            checkpoints: Vec::new(),
        }
    }

    /// Enable LLM-based coherence evaluation
    pub fn with_llm_coherence(mut self) -> Self {
        let mut scorer_config = QualityScorerConfig::default();
        scorer_config.use_llm_coherence = true;
        self.scorer = QualityScorer::with_llm(scorer_config, Arc::clone(&self.llm));
        self
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        self.session.id()
    }

    /// Get the underlying session (immutable)
    pub fn session(&self) -> &ConversationSession {
        &self.session
    }

    /// Get the underlying session (mutable)
    pub fn session_mut(&mut self) -> &mut ConversationSession {
        &mut self.session
    }

    /// Process a user input turn with automatic context management
    ///
    /// This method:
    /// 1. Adds the user message to the session
    /// 2. Retrieves relevant memories for grounding
    /// 3. Checks if summarization is needed
    /// 4. Creates checkpoints at configured intervals
    /// 5. Monitors quality and triggers rollback if needed
    pub async fn process_turn(&mut self, input: &str) -> Result<ContextTurnResult> {
        // Add user message
        self.session.add_user_message(input);
        self.state.increment_turns();
        self.turns_since_checkpoint += 1;

        // Ground with relevant memories
        let grounded = self.ground_query(input).await?;
        self.state.update_memory_relevance(&grounded.relevance_scores);

        // Update token estimate
        let token_count = self.estimated_tokens();
        self.state.update_tokens(token_count);

        // Check if summarization is needed
        let mut summarization_triggered = false;
        if self.config.auto_summarize && token_count > self.config.summarization_threshold() {
            let result = self.summarize().await?;
            if result.turns_summarized > 0 {
                summarization_triggered = true;
            }
        }

        // Create checkpoint if interval reached
        let mut checkpoint_created = None;
        if self.config.auto_checkpoint
            && self.turns_since_checkpoint >= self.config.checkpoint_interval
        {
            checkpoint_created = self
                .checkpoint(&format!("Auto checkpoint at turn {}", self.state.turn_count))
                .await?;
        }

        // Calculate quality score
        let quality_score = self.scorer.calculate(&self.state);

        // Check for rollback condition
        let mut rollback_triggered = false;
        if self.config.auto_rollback
            && quality_score < self.config.quality_threshold
            && !self.checkpoints.is_empty()
        {
            self.rollback().await?;
            rollback_triggered = true;
        }

        Ok(ContextTurnResult {
            grounding_used: grounded.memories.iter().map(|m| m.id.clone()).collect(),
            summarization_triggered,
            checkpoint_created,
            quality_score,
            token_count: self.estimated_tokens(),
            rollback_triggered,
        })
    }

    /// Add an assistant response to the current turn
    pub fn add_response(&mut self, response: &str) {
        self.session.add_assistant_message(response);
    }

    /// Ground a query with relevant memories
    pub async fn ground_query(&self, query: &str) -> Result<GroundedContext> {
        let memories = self
            .memory
            .search(query, Some(self.config.grounding_memories))
            .await?;

        // Calculate relevance scores based on memory strength
        let relevance_scores: Vec<f64> = memories
            .iter()
            .enumerate()
            .map(|(i, m)| {
                // Position-based relevance (first results are most relevant)
                let position_score = 1.0 - (i as f64 / memories.len().max(1) as f64);
                // Combine with memory strength
                let strength = self.memory.calculate_strength(m);
                (position_score + strength) / 2.0
            })
            .collect();

        // Generate summary of memories
        let summary = if memories.is_empty() {
            String::new()
        } else {
            memories
                .iter()
                .take(3)
                .map(|m| m.content.chars().take(100).collect::<String>())
                .collect::<Vec<_>>()
                .join("; ")
        };

        Ok(GroundedContext {
            memories,
            summary,
            relevance_scores,
        })
    }

    /// Get messages formatted for LLM context window
    pub fn get_context_messages(&self) -> Vec<Message> {
        self.session.get_messages()
    }

    /// Get messages with truncation applied
    pub fn get_context_messages_truncated(&self) -> Vec<Message> {
        self.session
            .get_messages_truncated(&self.summary_strategy, self.config.max_tokens)
    }

    /// Get estimated token count for current context
    pub fn estimated_tokens(&self) -> usize {
        self.session.history().estimate_tokens()
    }

    /// Get current quality score
    pub fn quality_score(&self) -> f64 {
        self.scorer.calculate(&self.state)
    }

    /// Get current context state
    pub fn context_state(&self) -> &ContextState {
        &self.state
    }

    /// Force summarization of older turns
    pub async fn summarize(&mut self) -> Result<SummarizationResult> {
        let turns = self.session.history().turns();
        let keep_recent = self.config.recent_turns_to_keep;

        if turns.len() <= keep_recent {
            return Ok(SummarizationResult::no_op());
        }

        let tokens_before = self.estimated_tokens();

        // Get turns to summarize (exclude recent)
        let to_summarize = &turns[..turns.len() - keep_recent];
        let turns_summarized = to_summarize.len();

        // Build content to summarize
        let content_to_summarize: String = to_summarize
            .iter()
            .map(|t| {
                let mut s = format!("User: {}", t.user_message);
                if let Some(ref resp) = t.assistant_message {
                    s.push_str(&format!("\nAssistant: {}", resp));
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Generate summary via LLM
        let summary = self.generate_summary(&content_to_summarize).await?;

        // Store summary as memory for future grounding
        self.memory
            .remember(format!("[context-summary] {}", summary))
            .await?;

        // Update summary strategy with new summary
        self.summary_strategy.set_summary(&summary);

        self.state.increment_summarizations();

        let tokens_after = self.estimated_tokens();

        Ok(SummarizationResult {
            tokens_before,
            tokens_after,
            turns_summarized,
            summary,
        })
    }

    /// Generate summary using LLM
    async fn generate_summary(&self, content: &str) -> Result<String> {
        let prompt = format!(
            r#"Summarize the following conversation concisely, preserving key information, decisions, and context that would be important for continuing the conversation:

{}

Provide a brief summary (2-4 sentences):"#,
            content
        );

        let request = LLMRequest {
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt,
            }],
            temperature: Some(0.3),
            max_tokens: Some(200),
            stop_sequences: vec![],
        };

        let response = self.llm.generate_request(&request).await?;
        Ok(response.content.trim().to_string())
    }

    /// Create a checkpoint (if versioning enabled)
    pub async fn checkpoint(&mut self, message: &str) -> Result<Option<String>> {
        let Some(repo) = &self.repo else {
            return Ok(None);
        };

        // Create memory commit
        let commit_hash = repo.commit(message, "context-manager", None).await?;

        // Create session snapshot
        let session_json = self
            .session
            .to_json()
            .map_err(|e| ThymosError::Serialization(e))?;

        let session_snapshot = SessionSnapshot {
            session_json,
            turn_count: self.state.turn_count,
            quality_score: self.quality_score(),
        };

        // Store checkpoint
        self.checkpoints.push(Checkpoint {
            commit_hash: commit_hash.clone(),
            session_snapshot,
        });

        self.turns_since_checkpoint = 0;

        Ok(Some(commit_hash))
    }

    /// Rollback to last checkpoint (if versioning enabled)
    pub async fn rollback(&mut self) -> Result<RollbackResult> {
        let Some(repo) = &self.repo else {
            return Err(ThymosError::Configuration(
                "Versioning not enabled".to_string(),
            ));
        };

        let checkpoint = self
            .checkpoints
            .pop()
            .ok_or_else(|| ThymosError::Memory("No checkpoint to rollback to".into()))?;

        // Restore memory state via checkout
        repo.checkout(&checkpoint.commit_hash).await?;

        // Calculate turns lost
        let turns_lost = self.state.turn_count - checkpoint.session_snapshot.turn_count;

        // Restore session state
        let restored_session =
            ConversationSession::from_json(&checkpoint.session_snapshot.session_json)
                .map_err(|e| ThymosError::Serialization(e))?;

        self.session = restored_session;
        self.state.turn_count = checkpoint.session_snapshot.turn_count;
        self.turns_since_checkpoint = 0;

        Ok(RollbackResult {
            commit_restored: checkpoint.commit_hash,
            turns_lost,
        })
    }

    /// Get number of available checkpoints
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Check if versioning is enabled
    pub fn has_versioning(&self) -> bool {
        self.repo.is_some()
    }

    /// Get grounded messages with memory context prepended
    pub async fn get_grounded_messages(&self, query: &str) -> Result<Vec<Message>> {
        let grounded = self.ground_query(query).await?;
        let mut messages = Vec::new();

        // Add system prompt if present
        if let Some(system) = self.session.history().system_prompt() {
            messages.push(Message {
                role: MessageRole::System,
                content: system.to_string(),
            });
        }

        // Add grounding context if we have relevant memories
        if !grounded.memories.is_empty() {
            let memory_context: String = grounded
                .memories
                .iter()
                .map(|m| format!("- {}", m.content))
                .collect::<Vec<_>>()
                .join("\n");

            messages.push(Message {
                role: MessageRole::System,
                content: format!("Relevant context from memory:\n{}", memory_context),
            });
        }

        // Add summary if available
        if let Some(summary) = self.summary_strategy.summary() {
            messages.push(Message {
                role: MessageRole::System,
                content: format!("Summary of earlier conversation:\n{}", summary),
            });
        }

        // Add recent conversation turns
        let recent_turns = self.session.history().last_turns(self.config.recent_turns_to_keep);
        for turn in recent_turns {
            messages.extend(turn.to_messages());
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::llm::{LLMConfig, LLMResponse, StubLLMProvider};
    use async_trait::async_trait;
    use tempfile::TempDir;

    struct MockLLMProvider {
        response: String,
    }

    impl MockLLMProvider {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl LLMProvider for MockLLMProvider {
        async fn generate(&self, _prompt: &str, _config: &LLMConfig) -> Result<String> {
            Ok(self.response.clone())
        }

        async fn generate_request(&self, _request: &LLMRequest) -> Result<LLMResponse> {
            Ok(LLMResponse {
                content: self.response.clone(),
                usage: None,
            })
        }
    }

    async fn create_test_memory() -> (TempDir, Arc<MemorySystem>) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MemoryConfig {
            mode: crate::config::MemoryMode::Embedded {
                data_dir: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };
        let memory = MemorySystem::new(config)
            .await
            .expect("Failed to create memory");
        (temp_dir, Arc::new(memory))
    }

    #[tokio::test]
    async fn test_context_manager_creation() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test response"));

        let manager = ContextManager::new("test-session", memory, llm, ContextConfig::default());

        assert_eq!(manager.session_id(), "test-session");
        assert!(!manager.has_versioning());
    }

    #[tokio::test]
    async fn test_context_manager_with_system_prompt() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test"));

        let manager = ContextManager::with_system_prompt(
            "test",
            "You are a helpful assistant",
            memory,
            llm,
            ContextConfig::default(),
        );

        assert_eq!(
            manager.session().history().system_prompt(),
            Some("You are a helpful assistant")
        );
    }

    #[tokio::test]
    async fn test_process_turn() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("Summary of conversation"));

        let config = ContextConfig::new()
            .with_auto_checkpoint(false)
            .with_auto_summarize(false);

        let mut manager = ContextManager::new("test", memory, llm, config);

        let result = manager.process_turn("Hello, how are you?").await.unwrap();

        assert!(!result.summarization_triggered);
        assert!(result.checkpoint_created.is_none());
        assert!(result.quality_score > 0.0);
    }

    #[tokio::test]
    async fn test_add_response() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test"));

        let mut manager =
            ContextManager::new("test", memory, llm, ContextConfig::default());

        manager.session_mut().add_user_message("Hello");
        manager.add_response("Hi there!");

        let messages = manager.get_context_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "Hi there!");
    }

    #[tokio::test]
    async fn test_ground_query() {
        let (_temp_dir, memory) = create_test_memory().await;

        // Add some memories
        memory
            .remember("The sky is blue".to_string())
            .await
            .unwrap();
        memory
            .remember("Rust is a programming language".to_string())
            .await
            .unwrap();

        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test"));
        let manager = ContextManager::new("test", memory, llm, ContextConfig::default());

        let grounded = manager.ground_query("programming").await.unwrap();

        // Results depend on search implementation
        assert!(grounded.relevance_scores.len() == grounded.memories.len());
    }

    #[tokio::test]
    async fn test_quality_score() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test"));

        let manager = ContextManager::new("test", memory, llm, ContextConfig::default());

        let score = manager.quality_score();
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[tokio::test]
    async fn test_estimated_tokens() {
        let (_temp_dir, memory) = create_test_memory().await;
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("test"));

        let mut manager =
            ContextManager::new("test", memory, llm, ContextConfig::default());

        assert_eq!(manager.estimated_tokens(), 0);

        manager.session_mut().add_user_message("Hello world");
        assert!(manager.estimated_tokens() > 0);
    }
}
