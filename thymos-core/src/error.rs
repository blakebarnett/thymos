//! Error types for Thymos operations

/// Result type for Thymos operations
pub type Result<T> = std::result::Result<T, ThymosError>;

/// Error types for Thymos framework
#[derive(Debug, thiserror::Error)]
pub enum ThymosError {
    /// Agent-related errors
    #[error("Agent error: {0}")]
    Agent(String),

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Memory operation failed
    #[error("Memory error: {0}")]
    Memory(String),

    /// Memory system initialization failed
    #[error("Memory initialization error: {0}")]
    MemoryInit(String),

    /// Lifecycle management error
    #[error("Lifecycle error: {0}")]
    Lifecycle(String),

    /// Event system error
    #[error("Event error: {0}")]
    Event(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Invalid relevance context
    #[error("Invalid relevance context: {0}")]
    InvalidContext(String),

    /// Agent startup timeout
    #[error("Agent startup timeout")]
    StartupTimeout,

    /// Agent shutdown timeout
    #[error("Agent shutdown timeout")]
    ShutdownTimeout,

    /// Storage error (from Locai)
    #[error("Storage error: {0}")]
    Storage(#[from] locai::LocaiError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for ThymosError {
    fn from(s: String) -> Self {
        ThymosError::Other(s)
    }
}

impl From<&str> for ThymosError {
    fn from(s: &str) -> Self {
        ThymosError::Other(s.to_string())
    }
}

impl From<anyhow::Error> for ThymosError {
    fn from(err: anyhow::Error) -> Self {
        ThymosError::Other(err.to_string())
    }
}
