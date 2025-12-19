//! Skill system for bundling tools, prompts, memory scope, and policy
//!
//! Skills are high-level capability bundles that group related tools with:
//! - A common prompt/context template
//! - Memory scope restrictions
//! - Capability policy overrides
//!
//! # Anthropic Skills Compatibility
//!
//! This module supports Anthropic's Agent Skills format (SKILL.md files):
//!
//! ```rust,ignore
//! // Load from Anthropic SKILL.md format
//! let skill = Skill::from_skill_md("skills/my-skill/SKILL.md")?;
//!
//! // Export to SKILL.md format
//! let md = skill.to_skill_md();
//!
//! // Generate system prompt XML
//! let xml = format_skills_for_prompt(&[&skill], Some(Path::new("/skills")));
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::skills::{Skill, SkillBuilder};
//! use thymos_core::tools::CapabilityPolicy;
//!
//! let research_skill = SkillBuilder::new("research", "Perform online research")
//!     .add_tool(search_tool)
//!     .add_tool(browse_tool)
//!     .with_prompt_template("You are a research assistant. {{task}}")
//!     .with_memory_scope("research")
//!     .with_policy(CapabilityPolicy::with_capabilities([Capability::Network]))
//!     .build();
//!
//! agent.register_skill(research_skill)?;
//! ```

mod anthropic;
mod builtin;
mod skill;

pub use anthropic::{
    format_skills_for_prompt, is_valid_hyphen_case, to_hyphen_case, AnthropicSkillFrontmatter,
};
pub use builtin::{create_memory_skill, MemorySearchTool, MemoryStoreTool};
pub use skill::{PromptTemplate, Skill, SkillBuilder, SkillError};

