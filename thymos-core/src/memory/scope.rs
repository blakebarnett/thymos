//! Memory scope types for hybrid memory mode

use serde::{Deserialize, Serialize};

/// Memory scope determines where a memory is stored
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    /// Private memory stored in embedded backend
    Private,
    /// Shared memory stored in server backend
    Shared,
}

/// Search scope determines which backends to search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchScope {
    /// Search only private memories
    Private,
    /// Search only shared memories
    Shared,
    /// Search both backends and merge results
    Both,
}
