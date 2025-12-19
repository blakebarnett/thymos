//! Core parser trait and error types

use thiserror::Error;

/// Error type for parsing operations
#[derive(Debug, Error, Clone)]
pub enum ParseError {
    /// Invalid format
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Type mismatch
    #[error("Type mismatch for '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    /// Repair failed
    #[error("Failed to repair malformed output: {0}")]
    RepairFailed(String),

    /// Empty input
    #[error("Empty input")]
    EmptyInput,
}

/// Result type for parsing operations
pub type ParseResult<T> = Result<T, ParseError>;

/// Trait for output parsers
pub trait OutputParser: Send + Sync {
    /// The output type produced by this parser
    type Output;

    /// Parse the raw output string
    fn parse(&self, raw: &str) -> ParseResult<Self::Output>;

    /// Check if this parser can handle the input
    fn can_parse(&self, raw: &str) -> bool;

    /// Get the parser name for debugging
    fn name(&self) -> &'static str;
}

/// Configuration for parser behavior
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Attempt to repair malformed input
    pub attempt_repair: bool,
    /// Strip markdown code fences
    pub strip_code_fences: bool,
    /// Trim whitespace
    pub trim_whitespace: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            attempt_repair: true,
            strip_code_fences: true,
            trim_whitespace: true,
        }
    }
}

impl ParserConfig {
    /// Create a strict config (no repair attempts)
    pub fn strict() -> Self {
        Self {
            attempt_repair: false,
            strip_code_fences: true,
            trim_whitespace: true,
        }
    }

    /// Create a lenient config (maximum repair)
    pub fn lenient() -> Self {
        Self {
            attempt_repair: true,
            strip_code_fences: true,
            trim_whitespace: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_config_default() {
        let config = ParserConfig::default();
        assert!(config.attempt_repair);
        assert!(config.strip_code_fences);
    }

    #[test]
    fn test_parser_config_strict() {
        let config = ParserConfig::strict();
        assert!(!config.attempt_repair);
    }
}
