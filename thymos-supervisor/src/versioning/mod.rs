//! Memory versioning integration for supervisor

pub mod supervisor;
pub mod decision;

#[cfg(test)]
mod tests;

pub use supervisor::{VersioningSupervisor, VersioningSupervisorConfig};
pub use decision::{AutoDecisionEngine, DecisionAction, DecisionCondition, DecisionRule};

