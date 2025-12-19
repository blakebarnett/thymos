//! Anthropic Skills Spec Compatibility
//!
//! This module provides support for loading and exporting skills in
//! Anthropic's Agent Skills format (SKILL.md files).
//!
//! # Format
//!
//! Anthropic's SKILL.md format uses YAML frontmatter:
//!
//! ```markdown
//! ---
//! name: my-skill-name
//! description: Description of what this skill does
//! license: Apache-2.0
//! allowed-tools:
//!   - tool_a
//!   - tool_b
//! metadata:
//!   version: "1.0"
//! ---
//!
//! # Skill Instructions
//!
//! [Markdown body with instructions]
//! ```
//!
//! # References
//!
//! - [Anthropic Skills Spec](https://github.com/anthropics/skills/tree/main/spec)

use super::{Skill, SkillBuilder, SkillError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Anthropic SKILL.md frontmatter structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AnthropicSkillFrontmatter {
    /// Skill name in hyphen-case
    pub name: String,
    /// Description of what the skill does
    pub description: String,
    /// License (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Pre-approved tools list (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Arbitrary metadata key-value pairs
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl AnthropicSkillFrontmatter {
    /// Validate the frontmatter
    pub fn validate(&self) -> Result<(), SkillError> {
        // Validate name is hyphen-case
        if !is_valid_hyphen_case(&self.name) {
            return Err(SkillError::InvalidConfig(format!(
                "Skill name '{}' must be hyphen-case (lowercase alphanumeric + hyphens)",
                self.name
            )));
        }

        // Description is required
        if self.description.is_empty() {
            return Err(SkillError::InvalidConfig(
                "Skill description is required".to_string(),
            ));
        }

        Ok(())
    }
}

/// Check if a string is valid hyphen-case (lowercase alphanumeric + hyphens)
pub fn is_valid_hyphen_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Must start with a letter
    let first_char = s.chars().next().unwrap();
    if !first_char.is_ascii_lowercase() {
        return false;
    }

    // All characters must be lowercase alphanumeric or hyphen
    s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Convert a string to hyphen-case
pub fn to_hyphen_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_separator = true;

    for c in s.chars() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && !prev_was_separator && !result.is_empty() {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_separator = false;
        } else if !prev_was_separator {
            result.push('-');
            prev_was_separator = true;
        }
    }

    // Remove trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

/// Parse a SKILL.md file content into frontmatter and body
fn parse_skill_md(content: &str) -> Result<(AnthropicSkillFrontmatter, String), SkillError> {
    let content = content.trim();

    // Check for frontmatter delimiter
    if !content.starts_with("---") {
        return Err(SkillError::InvalidConfig(
            "SKILL.md must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find end of frontmatter
    let rest = &content[3..];
    let end_pos = rest.find("---").ok_or_else(|| {
        SkillError::InvalidConfig("SKILL.md frontmatter not properly closed (---)".to_string())
    })?;

    let frontmatter_str = &rest[..end_pos].trim();
    let body = rest[end_pos + 3..].trim().to_string();

    // Parse YAML frontmatter
    let frontmatter: AnthropicSkillFrontmatter = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| SkillError::InvalidConfig(format!("Failed to parse YAML frontmatter: {}", e)))?;

    frontmatter.validate()?;

    Ok((frontmatter, body))
}

impl Skill {
    /// Load a skill from a SKILL.md file
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let skill = Skill::from_skill_md("skills/my-skill/SKILL.md")?;
    /// ```
    pub fn from_skill_md(path: impl AsRef<Path>) -> Result<Self, SkillError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            SkillError::InvalidConfig(format!("Failed to read SKILL.md: {}", e))
        })?;

        Self::from_skill_md_content(&content)
    }

    /// Load a skill from SKILL.md content string
    pub fn from_skill_md_content(content: &str) -> Result<Self, SkillError> {
        let (frontmatter, body) = parse_skill_md(content)?;

        let mut builder = SkillBuilder::new(&frontmatter.name, &frontmatter.description);

        if let Some(license) = frontmatter.license {
            builder = builder.with_license(license);
        }

        if !frontmatter.metadata.is_empty() {
            builder = builder.with_all_metadata(frontmatter.metadata);
        }

        if let Some(allowed_tools) = frontmatter.allowed_tools {
            builder = builder.with_allowed_tools(allowed_tools);
        }

        if !body.is_empty() {
            builder = builder.with_instructions(body);
        }

        Ok(builder.build())
    }

    /// Export the skill to SKILL.md format
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let skill = Skill::builder("my-skill", "Description")
    ///     .with_license("MIT")
    ///     .build();
    ///
    /// let content = skill.to_skill_md();
    /// std::fs::write("SKILL.md", content)?;
    /// ```
    pub fn to_skill_md(&self) -> String {
        let frontmatter = AnthropicSkillFrontmatter {
            name: self.name().to_string(),
            description: self.description().to_string(),
            license: self.license().map(|s| s.to_string()),
            allowed_tools: self.allowed_tools_list().map(|v| v.to_vec()),
            metadata: self.metadata().clone(),
        };

        let yaml = serde_yaml::to_string(&frontmatter).unwrap_or_default();

        let mut result = format!("---\n{}---\n", yaml);

        if let Some(instructions) = self.instructions() {
            result.push('\n');
            result.push_str(instructions);
            result.push('\n');
        }

        result
    }

    /// Generate Anthropic XML element for system prompt
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let xml = skill.to_anthropic_xml(Some("/path/to/skill/SKILL.md"));
    /// // <skill>
    /// // <name>my-skill</name>
    /// // <description>Description</description>
    /// // <location>/path/to/skill/SKILL.md</location>
    /// // </skill>
    /// ```
    pub fn to_anthropic_xml(&self, location: Option<&str>) -> String {
        let mut xml = String::from("<skill>\n");
        xml.push_str(&format!("<name>\n{}\n</name>\n", self.name()));
        xml.push_str(&format!("<description>\n{}\n</description>\n", self.description()));

        if let Some(loc) = location {
            xml.push_str(&format!("<location>\n{}\n</location>\n", loc));
        }

        xml.push_str("</skill>");
        xml
    }

    /// Convert skill name to hyphen-case (Anthropic format)
    pub fn name_as_hyphen_case(&self) -> String {
        to_hyphen_case(self.name())
    }

    /// Check if the skill name is valid hyphen-case
    pub fn is_valid_anthropic_name(&self) -> bool {
        is_valid_hyphen_case(self.name())
    }
}

/// Format multiple skills for system prompt in Anthropic's XML format
///
/// # Example
///
/// ```rust,ignore
/// let skills = vec![&skill1, &skill2];
/// let xml = format_skills_for_prompt(&skills, Some(Path::new("/skills")));
/// // <available_skills>
/// // <skill>...</skill>
/// // <skill>...</skill>
/// // </available_skills>
/// ```
pub fn format_skills_for_prompt(skills: &[&Skill], base_path: Option<&Path>) -> String {
    let mut xml = String::from("<available_skills>\n");

    for skill in skills {
        let location = base_path.map(|p| {
            p.join(skill.name())
                .join("SKILL.md")
                .to_string_lossy()
                .to_string()
        });

        xml.push_str(&skill.to_anthropic_xml(location.as_deref()));
        xml.push('\n');
    }

    xml.push_str("</available_skills>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_hyphen_case() {
        assert!(is_valid_hyphen_case("my-skill"));
        assert!(is_valid_hyphen_case("skill123"));
        assert!(is_valid_hyphen_case("my-skill-name"));
        assert!(is_valid_hyphen_case("a"));

        assert!(!is_valid_hyphen_case("MySkill"));
        assert!(!is_valid_hyphen_case("my_skill"));
        assert!(!is_valid_hyphen_case("-skill"));
        assert!(!is_valid_hyphen_case("skill-"));
        assert!(!is_valid_hyphen_case("my--skill"));
        assert!(!is_valid_hyphen_case(""));
        assert!(!is_valid_hyphen_case("123skill"));
    }

    #[test]
    fn test_to_hyphen_case() {
        assert_eq!(to_hyphen_case("MySkill"), "my-skill");
        assert_eq!(to_hyphen_case("my_skill"), "my-skill");
        assert_eq!(to_hyphen_case("MySkillName"), "my-skill-name");
        assert_eq!(to_hyphen_case("HTTPServer"), "h-t-t-p-server");
        assert_eq!(to_hyphen_case("skill123"), "skill123");
    }

    #[test]
    fn test_parse_skill_md() {
        let content = r#"---
name: my-skill
description: A test skill
license: MIT
allowed-tools:
  - tool_a
  - tool_b
metadata:
  version: "1.0"
---

# Instructions

This is the instruction body.
"#;

        let (frontmatter, body) = parse_skill_md(content).unwrap();

        assert_eq!(frontmatter.name, "my-skill");
        assert_eq!(frontmatter.description, "A test skill");
        assert_eq!(frontmatter.license, Some("MIT".to_string()));
        assert_eq!(
            frontmatter.allowed_tools,
            Some(vec!["tool_a".to_string(), "tool_b".to_string()])
        );
        assert_eq!(frontmatter.metadata.get("version"), Some(&"1.0".to_string()));
        assert!(body.contains("Instructions"));
    }

    #[test]
    fn test_skill_from_skill_md_content() {
        let content = r#"---
name: test-skill
description: Test description
license: Apache-2.0
---

# Test Instructions
"#;

        let skill = Skill::from_skill_md_content(content).unwrap();

        assert_eq!(skill.name(), "test-skill");
        assert_eq!(skill.description(), "Test description");
        assert_eq!(skill.license(), Some("Apache-2.0"));
        assert!(skill.instructions().unwrap().contains("Test Instructions"));
    }

    #[test]
    fn test_skill_to_skill_md() {
        let skill = Skill::builder("my-skill", "My description")
            .with_license("MIT")
            .with_metadata("version", "1.0")
            .with_instructions("# Instructions\n\nDo this.")
            .build();

        let md = skill.to_skill_md();

        assert!(md.contains("name: my-skill"));
        assert!(md.contains("description: My description"));
        assert!(md.contains("license: MIT"));
        assert!(md.contains("# Instructions"));
    }

    #[test]
    fn test_skill_to_anthropic_xml() {
        let skill = Skill::builder("my-skill", "My description").build();

        let xml = skill.to_anthropic_xml(Some("/path/to/SKILL.md"));

        assert!(xml.contains("<skill>"));
        assert!(xml.contains("<name>\nmy-skill\n</name>"));
        assert!(xml.contains("<description>\nMy description\n</description>"));
        assert!(xml.contains("<location>\n/path/to/SKILL.md\n</location>"));
        assert!(xml.contains("</skill>"));
    }

    #[test]
    fn test_format_skills_for_prompt() {
        let skill1 = Skill::builder("skill-one", "First skill").build();
        let skill2 = Skill::builder("skill-two", "Second skill").build();

        let xml = format_skills_for_prompt(&[&skill1, &skill2], Some(Path::new("/skills")));

        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("</available_skills>"));
        assert!(xml.contains("<name>\nskill-one\n</name>"));
        assert!(xml.contains("<name>\nskill-two\n</name>"));
        assert!(xml.contains("/skills/skill-one/SKILL.md"));
    }

    #[test]
    fn test_frontmatter_validation_invalid_name() {
        let frontmatter = AnthropicSkillFrontmatter {
            name: "MySkill".to_string(), // Not hyphen-case
            description: "Test".to_string(),
            license: None,
            allowed_tools: None,
            metadata: HashMap::new(),
        };

        assert!(frontmatter.validate().is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = Skill::builder("roundtrip-skill", "A skill for roundtrip testing")
            .with_license("MIT")
            .with_metadata("author", "test")
            .with_metadata("version", "1.0")
            .with_allowed_tools(vec!["tool_a".to_string(), "tool_b".to_string()])
            .with_instructions("# Usage\n\nFollow these steps.")
            .build();

        let md = original.to_skill_md();
        let restored = Skill::from_skill_md_content(&md).unwrap();

        assert_eq!(original.name(), restored.name());
        assert_eq!(original.description(), restored.description());
        assert_eq!(original.license(), restored.license());
        assert_eq!(original.instructions(), restored.instructions());
    }
}
