//! Capability-based permission system for tools
//!
//! Implements deny-by-default capability enforcement. Tools must declare
//! required capabilities, and the runtime enforces policy before execution.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Capabilities that tools may require
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read from filesystem
    FilesystemRead,

    /// Write to filesystem
    FilesystemWrite,

    /// Make network requests
    Network,

    /// Execute subprocesses
    Subprocess,

    /// Access secrets/credentials
    Secrets,

    /// Access memory system (read)
    MemoryRead,

    /// Access memory system (write)
    MemoryWrite,

    /// Spawn subagents
    SubagentSpawn,

    /// Access LLM providers
    LlmAccess,
}

impl Capability {
    /// Get all defined capabilities
    pub fn all() -> &'static [Capability] {
        &[
            Capability::FilesystemRead,
            Capability::FilesystemWrite,
            Capability::Network,
            Capability::Subprocess,
            Capability::Secrets,
            Capability::MemoryRead,
            Capability::MemoryWrite,
            Capability::SubagentSpawn,
            Capability::LlmAccess,
        ]
    }

    /// Get capabilities considered "privileged" (dangerous if misused)
    pub fn privileged() -> &'static [Capability] {
        &[
            Capability::FilesystemWrite,
            Capability::Network,
            Capability::Subprocess,
            Capability::Secrets,
            Capability::SubagentSpawn,
        ]
    }

    /// Check if this capability is privileged
    pub fn is_privileged(&self) -> bool {
        Self::privileged().contains(self)
    }

    /// Get the string name of this capability
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::FilesystemRead => "filesystem_read",
            Capability::FilesystemWrite => "filesystem_write",
            Capability::Network => "network",
            Capability::Subprocess => "subprocess",
            Capability::Secrets => "secrets",
            Capability::MemoryRead => "memory_read",
            Capability::MemoryWrite => "memory_write",
            Capability::SubagentSpawn => "subagent_spawn",
            Capability::LlmAccess => "llm_access",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A set of capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create an empty capability set
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a capability set with all capabilities
    pub fn all() -> Self {
        Self {
            capabilities: Capability::all().iter().copied().collect(),
        }
    }

    /// Create a capability set from an iterator of capabilities
    pub fn from_capabilities(iter: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            capabilities: iter.into_iter().collect(),
        }
    }

    /// Add a capability
    pub fn add(&mut self, cap: Capability) -> &mut Self {
        self.capabilities.insert(cap);
        self
    }

    /// Remove a capability
    pub fn remove(&mut self, cap: Capability) -> &mut Self {
        self.capabilities.remove(&cap);
        self
    }

    /// Check if capability is present
    pub fn contains(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if all capabilities in `required` are present
    pub fn contains_all(&self, required: &CapabilitySet) -> bool {
        required.capabilities.is_subset(&self.capabilities)
    }

    /// Get missing capabilities compared to required set
    pub fn missing(&self, required: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            capabilities: required
                .capabilities
                .difference(&self.capabilities)
                .copied()
                .collect(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Get iterator over capabilities
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self {
            capabilities: iter.into_iter().collect(),
        }
    }
}

/// Policy that controls which capabilities are allowed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPolicy {
    /// Explicitly allowed capabilities
    allowed: CapabilitySet,

    /// Explicitly denied capabilities (takes precedence over allowed)
    denied: CapabilitySet,

    /// If true, allow capabilities not explicitly mentioned (default: false = deny-by-default)
    default_allow: bool,
}

impl Default for CapabilityPolicy {
    fn default() -> Self {
        Self::deny_all()
    }
}

impl CapabilityPolicy {
    /// Create a policy that denies all capabilities by default
    pub fn deny_all() -> Self {
        Self {
            allowed: CapabilitySet::new(),
            denied: CapabilitySet::new(),
            default_allow: false,
        }
    }

    /// Create a policy that allows all capabilities (use with caution!)
    pub fn allow_all() -> Self {
        Self {
            allowed: CapabilitySet::all(),
            denied: CapabilitySet::new(),
            default_allow: true,
        }
    }

    /// Create a policy allowing only safe (non-privileged) capabilities
    pub fn safe_only() -> Self {
        let mut policy = Self::deny_all();
        for cap in Capability::all() {
            if !cap.is_privileged() {
                policy.allowed.add(*cap);
            }
        }
        policy
    }

    /// Create a policy allowing memory operations only
    pub fn memory_only() -> Self {
        let mut policy = Self::deny_all();
        policy.allowed.add(Capability::MemoryRead);
        policy.allowed.add(Capability::MemoryWrite);
        policy
    }

    /// Allow a specific capability
    pub fn allow(mut self, cap: Capability) -> Self {
        self.allowed.add(cap);
        self.denied.remove(cap);
        self
    }

    /// Deny a specific capability
    pub fn deny(mut self, cap: Capability) -> Self {
        self.denied.add(cap);
        self.allowed.remove(cap);
        self
    }

    /// Allow multiple capabilities
    pub fn allow_many(mut self, caps: impl IntoIterator<Item = Capability>) -> Self {
        for cap in caps {
            self.allowed.add(cap);
            self.denied.remove(cap);
        }
        self
    }

    /// Deny multiple capabilities
    pub fn deny_many(mut self, caps: impl IntoIterator<Item = Capability>) -> Self {
        for cap in caps {
            self.denied.add(cap);
            self.allowed.remove(cap);
        }
        self
    }

    /// Check if a capability is allowed by this policy
    pub fn is_allowed(&self, cap: Capability) -> bool {
        // Explicit deny takes precedence
        if self.denied.contains(cap) {
            return false;
        }

        // Then check explicit allow
        if self.allowed.contains(cap) {
            return true;
        }

        // Fall back to default
        self.default_allow
    }

    /// Check if all required capabilities are allowed
    pub fn check_all(&self, required: &CapabilitySet) -> Result<(), CapabilitySet> {
        let mut denied = CapabilitySet::new();

        for cap in required.iter() {
            if !self.is_allowed(*cap) {
                denied.add(*cap);
            }
        }

        if denied.is_empty() {
            Ok(())
        } else {
            Err(denied)
        }
    }

    /// Get allowed capabilities
    pub fn allowed(&self) -> &CapabilitySet {
        &self.allowed
    }

    /// Get denied capabilities
    pub fn denied(&self) -> &CapabilitySet {
        &self.denied
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn test_deny_all_default() {
        let policy = CapabilityPolicy::deny_all();

        for cap in Capability::all() {
            assert!(
                !policy.is_allowed(*cap),
                "Capability {:?} should be denied by default",
                cap
            );
        }
    }

    #[test]
    fn test_allow_all() {
        let policy = CapabilityPolicy::allow_all();

        for cap in Capability::all() {
            assert!(
                policy.is_allowed(*cap),
                "Capability {:?} should be allowed",
                cap
            );
        }
    }

    #[test]
    fn test_safe_only() {
        let policy = CapabilityPolicy::safe_only();

        // Safe capabilities should be allowed
        assert!(policy.is_allowed(Capability::MemoryRead));
        assert!(policy.is_allowed(Capability::MemoryWrite));
        assert!(policy.is_allowed(Capability::LlmAccess));
        assert!(policy.is_allowed(Capability::FilesystemRead));

        // Privileged capabilities should be denied
        assert!(!policy.is_allowed(Capability::FilesystemWrite));
        assert!(!policy.is_allowed(Capability::Network));
        assert!(!policy.is_allowed(Capability::Subprocess));
        assert!(!policy.is_allowed(Capability::Secrets));
        assert!(!policy.is_allowed(Capability::SubagentSpawn));
    }

    #[test]
    fn test_explicit_allow_deny() {
        let policy = CapabilityPolicy::deny_all()
            .allow(Capability::Network)
            .allow(Capability::MemoryRead);

        assert!(policy.is_allowed(Capability::Network));
        assert!(policy.is_allowed(Capability::MemoryRead));
        assert!(!policy.is_allowed(Capability::FilesystemWrite));
    }

    #[test]
    fn test_deny_takes_precedence() {
        let policy = CapabilityPolicy::allow_all().deny(Capability::Subprocess);

        assert!(!policy.is_allowed(Capability::Subprocess));
        assert!(policy.is_allowed(Capability::Network));
    }

    #[test]
    fn test_check_all() {
        let policy = CapabilityPolicy::deny_all()
            .allow(Capability::MemoryRead)
            .allow(Capability::MemoryWrite);

        let required =
            CapabilitySet::from_capabilities([Capability::MemoryRead, Capability::MemoryWrite]);
        assert!(policy.check_all(&required).is_ok());

        let required_with_network = CapabilitySet::from_capabilities([
            Capability::MemoryRead,
            Capability::Network,
        ]);
        let result = policy.check_all(&required_with_network);
        assert!(result.is_err());
        let denied = result.unwrap_err();
        assert!(denied.contains(Capability::Network));
    }

    #[test]
    fn test_capability_set_operations() {
        let mut set = CapabilitySet::new();
        assert!(set.is_empty());

        set.add(Capability::Network);
        assert!(set.contains(Capability::Network));
        assert!(!set.contains(Capability::Subprocess));

        set.remove(Capability::Network);
        assert!(!set.contains(Capability::Network));
    }

    #[test]
    fn test_missing_capabilities() {
        let have = CapabilitySet::from_capabilities([Capability::MemoryRead]);
        let need = CapabilitySet::from_capabilities([
            Capability::MemoryRead,
            Capability::Network,
            Capability::Subprocess,
        ]);

        let missing = have.missing(&need);
        assert!(missing.contains(Capability::Network));
        assert!(missing.contains(Capability::Subprocess));
        assert!(!missing.contains(Capability::MemoryRead));
    }
}



