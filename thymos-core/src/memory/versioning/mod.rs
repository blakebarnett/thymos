//! Memory versioning system providing Git-style operations
//!
//! This module provides Git-like operations for agent memory:
//! - Branches: Create and manage memory branches
//! - Commits: Stage and commit memory changes
//! - Checkout: Switch between branches/commits
//! - Merge: Merge branches with conflict resolution
//! - Worktrees: Concurrent agent instances with isolated memory

pub mod repository;
pub mod commit;
pub mod checkout;
pub mod merge;
pub mod worktree;

pub use repository::{MemoryRepository, MemoryBranch};
pub use commit::{MemoryCommit, CommitIndex, MemoryOperation};
pub use checkout::CheckoutResult;
pub use merge::{MergeStrategy, MergeResult, MemoryConflict};
pub use worktree::{MemoryWorktree, MemoryWorktreeManager};



