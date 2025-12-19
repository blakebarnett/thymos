use serde::{Deserialize, Serialize};

/// Hierarchical concept importance tier.
///
/// Concepts progress through tiers based on significance and mention frequency:
/// - **Mentioned**: Low significance, mentioned once in a single context
/// - **Provisional**: Medium significance or multiple mentions, worth tracking
/// - **Tracked**: High significance, persistent tracking, linked to core memories
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ConceptTier {
    /// Mentioned once, low significance (value: 1)
    Mentioned = 1,

    /// Multiple mentions or medium significance (value: 2)
    Provisional = 2,

    /// High significance, tracked persistently (value: 3)
    Tracked = 3,
}

impl ConceptTier {
    /// Get string representation of the tier.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Mentioned => "mentioned",
            Self::Provisional => "provisional",
            Self::Tracked => "tracked",
        }
    }

    /// Promote to the next tier if possible.
    pub fn promote(self) -> Option<Self> {
        match self {
            Self::Mentioned => Some(Self::Provisional),
            Self::Provisional => Some(Self::Tracked),
            Self::Tracked => None,
        }
    }

    /// Demote to the previous tier if possible.
    pub fn demote(self) -> Option<Self> {
        match self {
            Self::Mentioned => None,
            Self::Provisional => Some(Self::Mentioned),
            Self::Tracked => Some(Self::Provisional),
        }
    }
}

impl std::fmt::Display for ConceptTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_ordering() {
        assert!(ConceptTier::Mentioned < ConceptTier::Provisional);
        assert!(ConceptTier::Provisional < ConceptTier::Tracked);
        assert!(ConceptTier::Tracked > ConceptTier::Mentioned);
    }

    #[test]
    fn test_tier_promotion() {
        let tier = ConceptTier::Mentioned;
        assert_eq!(tier.promote(), Some(ConceptTier::Provisional));

        let tier = ConceptTier::Provisional;
        assert_eq!(tier.promote(), Some(ConceptTier::Tracked));

        let tier = ConceptTier::Tracked;
        assert_eq!(tier.promote(), None);
    }

    #[test]
    fn test_tier_demotion() {
        let tier = ConceptTier::Tracked;
        assert_eq!(tier.demote(), Some(ConceptTier::Provisional));

        let tier = ConceptTier::Provisional;
        assert_eq!(tier.demote(), Some(ConceptTier::Mentioned));

        let tier = ConceptTier::Mentioned;
        assert_eq!(tier.demote(), None);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(ConceptTier::Mentioned.to_string(), "mentioned");
        assert_eq!(ConceptTier::Provisional.to_string(), "provisional");
        assert_eq!(ConceptTier::Tracked.to_string(), "tracked");
    }
}
