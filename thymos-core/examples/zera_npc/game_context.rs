//! Game context for Zera NPC example

use std::sync::{Arc, RwLock};

/// Game context shared between NPCs
#[derive(Debug, Clone, Default)]
pub struct GameContext {
    /// Current game state
    pub current_zone: String,

    /// Active quests
    pub active_quests: Vec<String>,

    /// Party members
    pub party_members: Vec<String>,
}

impl GameContext {
    /// Create a new game context
    pub fn new() -> Self {
        Self {
            current_zone: "Oakshire".to_string(),
            active_quests: Vec::new(),
            party_members: Vec::new(),
        }
    }

    /// Check if an NPC is in the party
    pub fn is_in_party(&self, npc_id: &str) -> bool {
        self.party_members.contains(&npc_id.to_string())
    }

    /// Get zones away (simplified - always returns 0 for same zone)
    pub fn zones_away(&self, npc_zone: &str) -> i32 {
        if npc_zone == self.current_zone {
            0
        } else {
            1 // Simplified
        }
    }

    /// Check if NPC is in an active quest
    pub fn is_in_active_quest(&self, npc_id: &str) -> bool {
        self.active_quests.iter().any(|q| q.contains(npc_id))
    }
}

/// Type alias for shared game context
pub type SharedGameContext = Arc<RwLock<GameContext>>;
