//! Error types for supervisor operations

use thiserror::Error;

/// Result type for supervisor operations
pub type Result<T> = std::result::Result<T, SupervisorError>;

/// Error types for supervisor
#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("Supervisor error: {0}")]
    Supervisor(String),
    
    #[error("Agent binary not found: {0}")]
    BinaryNotFound(String),
    
    #[error("Agent startup timeout")]
    StartupTimeout,
    
    #[error("Agent shutdown timeout")]
    ShutdownTimeout,
    
    #[error("Process error: {0}")]
    Process(String),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<thymos_core::error::ThymosError> for SupervisorError {
    fn from(err: thymos_core::error::ThymosError) -> Self {
        SupervisorError::Other(anyhow::anyhow!("{}", err))
    }
}

