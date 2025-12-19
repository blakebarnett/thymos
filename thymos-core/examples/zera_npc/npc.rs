//! Zera NPC agent implementation

use super::game_context::SharedGameContext;
use super::personality::Personality;
use super::relevance::ZeraRelevanceEvaluator;
use std::sync::Arc;
use thymos_core::config::MemoryMode;
use thymos_core::memory::SearchScope;
use thymos_core::prelude::*;

/// Zera NPC agent with hybrid memory and game context
pub struct ZeraNPC {
    /// Underlying agent
    pub agent: Agent,

    /// Personality traits
    #[allow(dead_code)]
    pub personality: Personality,

    /// Shared game context
    pub game_context: SharedGameContext,

    /// Relevance evaluator
    pub relevance_evaluator: Arc<ZeraRelevanceEvaluator>,
}

impl ZeraNPC {
    /// Create a new Zera NPC
    pub async fn new(
        id: impl Into<String>,
        personality: Personality,
        game_context: SharedGameContext,
    ) -> Result<Self> {
        let id = id.into();

        // Configure hybrid memory
        let memory_config = MemoryConfig {
            mode: MemoryMode::Hybrid {
                private_data_dir: std::path::PathBuf::from(format!("./data/npcs/{}", id)),
                shared_url: "http://localhost:3000".to_string(),
                shared_api_key: None,
            },
            ..Default::default()
        };

        let agent = Agent::builder()
            .id(&id)
            .with_memory_config(memory_config)
            .build()
            .await?;

        let relevance_evaluator = Arc::new(ZeraRelevanceEvaluator);

        Ok(Self {
            agent,
            personality,
            game_context,
            relevance_evaluator,
        })
    }

    /// Observe something in the world (stored as shared memory)
    pub async fn observe(&self, observation: &str) -> Result<()> {
        self.agent.remember_shared(observation).await?;
        Ok(())
    }

    /// Think something internally (stored as private memory)
    pub async fn think(&self, thought: &str) -> Result<()> {
        self.agent.remember_private(thought).await?;
        Ok(())
    }

    /// Recall memories with a specific scope
    pub async fn recall(
        &self,
        query: &str,
        scope: SearchScope,
    ) -> Result<Vec<locai::models::Memory>> {
        self.agent.search_memories_with_scope(query, scope).await
    }

    /// Evaluate relevance based on current game context
    pub async fn evaluate_relevance(&self) -> Result<thymos_core::lifecycle::RelevanceScore> {
        let context = self.build_relevance_context().await;
        self.relevance_evaluator
            .evaluate(self.agent.id(), &context)
            .await
    }

    /// Build relevance context from game state
    async fn build_relevance_context(&self) -> thymos_core::lifecycle::RelevanceContext {
        let game_ctx = self.game_context.read().unwrap();
        let mut context = thymos_core::lifecycle::RelevanceContext::new();

        let npc_id = self.agent.id();
        let npc_zone = "Oakshire"; // Simplified - would come from NPC state

        context.set("in_party", game_ctx.is_in_party(npc_id));
        context.set("zones_away", game_ctx.zones_away(npc_zone));
        context.set("in_active_quest", game_ctx.is_in_active_quest(npc_id));
        context.set("mentioned_recently", false); // Simplified

        context
    }
}
