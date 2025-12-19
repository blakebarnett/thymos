//! Markdown section parser

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::parser::{OutputParser, ParseError, ParseResult};

/// A parsed markdown section
#[derive(Debug, Clone)]
pub struct MarkdownSection {
    /// Section heading
    pub heading: String,
    /// Heading level (1-6)
    pub level: usize,
    /// Section content
    pub content: String,
}

/// Markdown section parser
///
/// Parses markdown into sections based on headings.
pub struct MarkdownParser {
    /// Minimum heading level to parse (1-6)
    min_level: usize,
    /// Maximum heading level to parse (1-6)
    max_level: usize,
}

impl MarkdownParser {
    /// Create a new markdown parser
    pub fn new() -> Self {
        Self {
            min_level: 1,
            max_level: 6,
        }
    }

    /// Only parse headings of specific levels
    pub fn with_levels(min: usize, max: usize) -> Self {
        Self {
            min_level: min.clamp(1, 6),
            max_level: max.clamp(1, 6),
        }
    }

    /// Parse into a map of heading -> content
    pub fn parse_to_map(&self, raw: &str) -> ParseResult<HashMap<String, String>> {
        let sections = self.parse(raw)?;
        let mut map = HashMap::new();

        for section in sections {
            map.insert(section.heading.to_lowercase(), section.content);
        }

        Ok(map)
    }

    /// Get a specific section by heading
    pub fn get_section(&self, raw: &str, heading: &str) -> ParseResult<Option<String>> {
        let sections = self.parse(raw)?;
        let heading_lower = heading.to_lowercase();

        Ok(sections
            .into_iter()
            .find(|s| s.heading.to_lowercase() == heading_lower)
            .map(|s| s.content))
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for MarkdownParser {
    type Output = Vec<MarkdownSection>;

    fn parse(&self, raw: &str) -> ParseResult<Self::Output> {
        if raw.trim().is_empty() {
            return Err(ParseError::EmptyInput);
        }

        static HEADING_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+)$").unwrap());

        let mut sections = Vec::new();
        let mut current_heading: Option<(String, usize)> = None;
        let mut current_content = String::new();

        for line in raw.lines() {
            if let Some(caps) = HEADING_RE.captures(line) {
                let level = caps.get(1).unwrap().as_str().len();
                let heading = caps.get(2).unwrap().as_str().trim().to_string();

                // Check if level is in range
                if level >= self.min_level && level <= self.max_level {
                    // Save previous section
                    if let Some((prev_heading, prev_level)) = current_heading.take() {
                        sections.push(MarkdownSection {
                            heading: prev_heading,
                            level: prev_level,
                            content: current_content.trim().to_string(),
                        });
                        current_content.clear();
                    }

                    current_heading = Some((heading, level));
                    continue;
                }
            }

            // Add line to current content
            if current_heading.is_some() {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }

        // Don't forget the last section
        if let Some((heading, level)) = current_heading {
            sections.push(MarkdownSection {
                heading,
                level,
                content: current_content.trim().to_string(),
            });
        }

        if sections.is_empty() {
            return Err(ParseError::InvalidFormat(
                "No markdown sections found".to_string(),
            ));
        }

        Ok(sections)
    }

    fn can_parse(&self, raw: &str) -> bool {
        raw.lines().any(|line| line.starts_with('#'))
    }

    fn name(&self) -> &'static str {
        "markdown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_sections() {
        let parser = MarkdownParser::new();
        let input = r#"# Heading 1
Content for heading 1

## Heading 2
Content for heading 2
More content"#;

        let sections = parser.parse(input).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "Heading 1");
        assert_eq!(sections[0].level, 1);
        assert!(sections[0].content.contains("Content for heading 1"));
        assert_eq!(sections[1].heading, "Heading 2");
        assert_eq!(sections[1].level, 2);
    }

    #[test]
    fn test_parse_to_map() {
        let parser = MarkdownParser::new();
        let input = r#"# Summary
This is the summary

# Details
These are the details"#;

        let map = parser.parse_to_map(input).unwrap();
        assert!(map.contains_key("summary"));
        assert!(map.contains_key("details"));
        assert!(map["summary"].contains("This is the summary"));
    }

    #[test]
    fn test_get_section() {
        let parser = MarkdownParser::new();
        let input = r#"# Answer
42

# Explanation
The meaning of life"#;

        let answer = parser.get_section(input, "Answer").unwrap();
        assert_eq!(answer.unwrap(), "42");

        let missing = parser.get_section(input, "NotFound").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_level_filter() {
        let parser = MarkdownParser::with_levels(2, 2);
        let input = r#"# Heading 1
Content 1

## Heading 2
Content 2

### Heading 3
Content 3"#;

        let sections = parser.parse(input).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "Heading 2");
    }

    #[test]
    fn test_empty_input() {
        let parser = MarkdownParser::new();
        let result = parser.parse("");
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_no_headings() {
        let parser = MarkdownParser::new();
        let result = parser.parse("Just some text without headings");
        assert!(matches!(result, Err(ParseError::InvalidFormat(_))));
    }

    #[test]
    fn test_can_parse() {
        let parser = MarkdownParser::new();
        assert!(parser.can_parse("# Heading"));
        assert!(parser.can_parse("## Subheading"));
        assert!(!parser.can_parse("Just text"));
    }
}
