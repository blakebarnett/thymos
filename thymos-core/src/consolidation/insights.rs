use serde::{Deserialize, Serialize};

/// Types of insights that can be generated from memory consolidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InsightType {
    /// Recurring theme across multiple memories
    Theme,

    /// Identified pattern in behavior or events
    Pattern,

    /// Connection between entities or concepts
    Relationship,

    /// Significant concept or entity
    ImportantConcept,

    /// Emotionally significant event
    EmotionalEvent,

    /// Conflicting information or contradiction
    Contradiction,
}

impl InsightType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Theme => "theme",
            Self::Pattern => "pattern",
            Self::Relationship => "relationship",
            Self::ImportantConcept => "important_concept",
            Self::EmotionalEvent => "emotional_event",
            Self::Contradiction => "contradiction",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Theme => "A recurring theme or topic across memories",
            Self::Pattern => "An identified pattern in behavior or events",
            Self::Relationship => "A connection between entities or concepts",
            Self::ImportantConcept => "A significant concept or entity",
            Self::EmotionalEvent => "An event with significant emotional weight",
            Self::Contradiction => "Conflicting or contradictory information",
        }
    }
}

/// An insight generated from consolidated memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    /// Type of insight
    pub insight_type: InsightType,

    /// Human-readable summary of the insight
    pub summary: String,

    /// IDs of source memories that contributed to this insight
    pub source_memory_ids: Vec<String>,

    /// Confidence score (0.0-1.0)
    pub confidence: f64,

    /// Timestamp when this insight was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl Insight {
    /// Create a new insight.
    pub fn new(
        insight_type: InsightType,
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self {
            insight_type,
            summary: summary.into(),
            source_memory_ids,
            confidence: confidence.clamp(0.0, 1.0),
            generated_at: chrono::Utc::now(),
        }
    }

    /// Create an insight about a theme.
    pub fn theme(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(InsightType::Theme, summary, source_memory_ids, confidence)
    }

    /// Create an insight about a pattern.
    pub fn pattern(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(InsightType::Pattern, summary, source_memory_ids, confidence)
    }

    /// Create an insight about a relationship.
    pub fn relationship(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(
            InsightType::Relationship,
            summary,
            source_memory_ids,
            confidence,
        )
    }

    /// Create an insight about an important concept.
    pub fn important_concept(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(
            InsightType::ImportantConcept,
            summary,
            source_memory_ids,
            confidence,
        )
    }

    /// Create an insight about an emotional event.
    pub fn emotional_event(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(
            InsightType::EmotionalEvent,
            summary,
            source_memory_ids,
            confidence,
        )
    }

    /// Create an insight about a contradiction.
    pub fn contradiction(
        summary: impl Into<String>,
        source_memory_ids: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self::new(
            InsightType::Contradiction,
            summary,
            source_memory_ids,
            confidence,
        )
    }

    /// Add a source memory ID.
    pub fn with_source(mut self, memory_id: impl Into<String>) -> Self {
        self.source_memory_ids.push(memory_id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insight_creation() {
        let insight = Insight::new(
            InsightType::Theme,
            "A recurring theme",
            vec!["mem1".to_string()],
            0.8,
        );

        assert_eq!(insight.insight_type, InsightType::Theme);
        assert_eq!(insight.confidence, 0.8);
        assert_eq!(insight.source_memory_ids.len(), 1);
    }

    #[test]
    fn test_insight_builders() {
        let theme = Insight::theme("Theme summary", vec!["mem1".to_string()], 0.8);
        assert_eq!(theme.insight_type, InsightType::Theme);

        let pattern = Insight::pattern("Pattern summary", vec!["mem1".to_string()], 0.75);
        assert_eq!(pattern.insight_type, InsightType::Pattern);

        let emotional = Insight::emotional_event(
            "Sad event",
            vec!["mem1".to_string(), "mem2".to_string()],
            0.9,
        );
        assert_eq!(emotional.insight_type, InsightType::EmotionalEvent);
    }

    #[test]
    fn test_confidence_clamping() {
        let insight = Insight::new(InsightType::Theme, "test", vec![], 1.5);
        assert_eq!(insight.confidence, 1.0);

        let insight = Insight::new(InsightType::Theme, "test", vec![], -0.5);
        assert_eq!(insight.confidence, 0.0);
    }

    #[test]
    fn test_insight_type_descriptions() {
        assert!(!InsightType::Theme.description().is_empty());
        assert_eq!(InsightType::Pattern.as_str(), "pattern");
    }
}
