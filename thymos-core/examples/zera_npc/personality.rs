//! Personality system for Zera NPCs

use std::collections::HashMap;

/// Personality traits for an NPC
#[derive(Debug, Clone)]
pub struct Personality {
    /// Trait name -> strength (0.0 to 1.0)
    pub traits: HashMap<String, f64>,

    /// Speech patterns and behaviors
    pub speech_patterns: Vec<String>,
}

impl Personality {
    /// Create a new personality
    pub fn new() -> Self {
        Self {
            traits: HashMap::new(),
            speech_patterns: Vec::new(),
        }
    }

    /// Add a trait
    pub fn with_trait(mut self, name: impl Into<String>, strength: f64) -> Self {
        self.traits.insert(name.into(), strength.clamp(0.0, 1.0));
        self
    }

    /// Add a speech pattern
    pub fn with_speech_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.speech_patterns.push(pattern.into());
        self
    }
}

impl Default for Personality {
    fn default() -> Self {
        Self::new()
    }
}
