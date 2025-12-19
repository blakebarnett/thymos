use serde::{Deserialize, Serialize};

/// Type of alias for a concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AliasType {
    /// Epithet: descriptive phrase like "the old badger"
    Epithet,
    /// Explicit alias: "also known as", "aka"
    Alias,
    /// Title: "Dr.", "Captain", "King"
    Title,
    /// Descriptor: "the tall one", "the wise"
    Descriptor,
}

impl AliasType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Epithet => "epithet",
            Self::Alias => "alias",
            Self::Title => "title",
            Self::Descriptor => "descriptor",
        }
    }
}

/// Source/provenance of an alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AliasProvenance {
    /// The entity referred to itself with this name
    SelfReference,
    /// Someone else used this name to refer to the entity
    OtherReference,
    /// Third-person narrative description
    Narrator,
}

impl AliasProvenance {
    pub fn as_str(&self) -> &str {
        match self {
            Self::SelfReference => "self",
            Self::OtherReference => "other",
            Self::Narrator => "narrator",
        }
    }
}

/// An alternative name or reference for a concept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Alias {
    /// The alternate name or reference text
    pub text: String,

    /// Confidence score (0.0-1.0) that this is a valid alias
    pub confidence: f64,

    /// Type of alias
    pub alias_type: AliasType,

    /// Source/provenance of this alias
    pub provenance: AliasProvenance,
}

impl Alias {
    /// Create a new alias.
    pub fn new(
        text: impl Into<String>,
        alias_type: AliasType,
        provenance: AliasProvenance,
        confidence: f64,
    ) -> Self {
        Self {
            text: text.into(),
            confidence: confidence.clamp(0.0, 1.0),
            alias_type,
            provenance,
        }
    }

    /// Create an epithet (e.g., "the old badger")
    pub fn epithet(text: impl Into<String>, confidence: f64) -> Self {
        Self::new(
            text,
            AliasType::Epithet,
            AliasProvenance::Narrator,
            confidence,
        )
    }

    /// Create an explicit alias (e.g., "aka John Smith")
    pub fn new_alias(text: impl Into<String>, confidence: f64) -> Self {
        Self::new(
            text,
            AliasType::Alias,
            AliasProvenance::OtherReference,
            confidence,
        )
    }

    /// Create a title (e.g., "Dr.", "Captain")
    pub fn title(text: impl Into<String>, confidence: f64) -> Self {
        Self::new(
            text,
            AliasType::Title,
            AliasProvenance::Narrator,
            confidence,
        )
    }

    /// Create a descriptor (e.g., "the tall one")
    pub fn descriptor(text: impl Into<String>, confidence: f64) -> Self {
        Self::new(
            text,
            AliasType::Descriptor,
            AliasProvenance::Narrator,
            confidence,
        )
    }

    /// Set the provenance of this alias
    pub fn with_provenance(mut self, provenance: AliasProvenance) -> Self {
        self.provenance = provenance;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_creation() {
        let alias = Alias::epithet("the old badger", 0.85);
        assert_eq!(alias.text, "the old badger");
        assert_eq!(alias.alias_type, AliasType::Epithet);
        assert_eq!(alias.confidence, 0.85);
    }

    #[test]
    fn test_alias_types() {
        let epithet = Alias::epithet("the wise", 0.9);
        assert_eq!(epithet.alias_type, AliasType::Epithet);

        let alias = Alias::new_alias("aka John", 0.95);
        assert_eq!(alias.alias_type, AliasType::Alias);

        let title = Alias::title("Dr.", 0.99);
        assert_eq!(title.alias_type, AliasType::Title);

        let descriptor = Alias::descriptor("the tall one", 0.7);
        assert_eq!(descriptor.alias_type, AliasType::Descriptor);
    }

    #[test]
    fn test_confidence_clamping() {
        let high = Alias::epithet("x", 1.5);
        assert_eq!(high.confidence, 1.0);

        let low = Alias::epithet("x", -0.5);
        assert_eq!(low.confidence, 0.0);
    }

    #[test]
    fn test_provenance() {
        let alias =
            Alias::epithet("the badger", 0.8).with_provenance(AliasProvenance::SelfReference);
        assert_eq!(alias.provenance, AliasProvenance::SelfReference);
    }
}
