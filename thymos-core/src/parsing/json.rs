//! JSON parser with fuzzy repair

use regex::Regex;
use std::sync::LazyLock;

use super::parser::{OutputParser, ParseError, ParseResult, ParserConfig};

/// JSON parser with repair capabilities
pub struct JsonParser {
    config: ParserConfig,
}

impl JsonParser {
    /// Create a new JSON parser with default config
    pub fn new() -> Self {
        Self {
            config: ParserConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Create a strict parser (no repair)
    pub fn strict() -> Self {
        Self {
            config: ParserConfig::strict(),
        }
    }

    /// Extract JSON from markdown code fences
    fn strip_code_fences(&self, input: &str) -> String {
        static CODE_FENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"```(?:json|JSON)?\s*\n?([\s\S]*?)\n?```").unwrap()
        });

        if let Some(caps) = CODE_FENCE_RE.captures(input) {
            if let Some(content) = caps.get(1) {
                return content.as_str().to_string();
            }
        }

        // Also try single backticks
        static INLINE_CODE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

        if let Some(caps) = INLINE_CODE_RE.captures(input) {
            if let Some(content) = caps.get(1) {
                let text = content.as_str();
                if text.starts_with('{') || text.starts_with('[') {
                    return text.to_string();
                }
            }
        }

        input.to_string()
    }

    /// Extract JSON object/array from surrounding text
    fn extract_json(&self, input: &str) -> Option<String> {
        // Find first { or [
        let start_obj = input.find('{');
        let start_arr = input.find('[');

        let (start, end_char) = match (start_obj, start_arr) {
            (Some(o), Some(a)) if o < a => (o, '}'),
            (Some(_), Some(a)) => (a, ']'),
            (Some(o), None) => (o, '}'),
            (None, Some(a)) => (a, ']'),
            (None, None) => return None,
        };

        // Find matching end
        let substring = &input[start..];
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, c) in substring.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' | '[' if !in_string => depth += 1,
                '}' | ']' if !in_string => {
                    depth -= 1;
                    if depth == 0 && c == end_char {
                        return Some(substring[..=i].to_string());
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Repair common JSON issues
    fn repair_json(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Remove trailing commas before } or ]
        static TRAILING_COMMA_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r",(\s*[}\]])").unwrap());
        result = TRAILING_COMMA_RE.replace_all(&result, "$1").to_string();

        // Replace single quotes with double quotes (but not in strings)
        result = self.fix_quotes(&result);

        // Fix unquoted keys
        static UNQUOTED_KEY_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(\{|,)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:").unwrap());
        result = UNQUOTED_KEY_RE
            .replace_all(&result, r#"$1"$2":"#)
            .to_string();

        // Remove comments (// and /* */)
        static LINE_COMMENT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"//[^\n]*").unwrap());
        static BLOCK_COMMENT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"/\*[\s\S]*?\*/").unwrap());
        result = LINE_COMMENT_RE.replace_all(&result, "").to_string();
        result = BLOCK_COMMENT_RE.replace_all(&result, "").to_string();

        // Fix missing closing braces/brackets
        let open_braces = result.matches('{').count();
        let close_braces = result.matches('}').count();
        let open_brackets = result.matches('[').count();
        let close_brackets = result.matches(']').count();

        for _ in close_braces..open_braces {
            result.push('}');
        }
        for _ in close_brackets..open_brackets {
            result.push(']');
        }

        result
    }

    /// Fix single quotes to double quotes
    fn fix_quotes(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut in_double_string = false;
        let mut in_single_string = false;
        let mut escape_next = false;

        for c in input.chars() {
            if escape_next {
                result.push(c);
                escape_next = false;
                continue;
            }

            match c {
                '\\' => {
                    result.push(c);
                    escape_next = true;
                }
                '"' if !in_single_string => {
                    in_double_string = !in_double_string;
                    result.push(c);
                }
                '\'' if !in_double_string => {
                    in_single_string = !in_single_string;
                    result.push('"'); // Convert to double quote
                }
                _ => result.push(c),
            }
        }

        result
    }
}

impl Default for JsonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for JsonParser {
    type Output = serde_json::Value;

    fn parse(&self, raw: &str) -> ParseResult<Self::Output> {
        if raw.trim().is_empty() {
            return Err(ParseError::EmptyInput);
        }

        let mut input = if self.config.trim_whitespace {
            raw.trim().to_string()
        } else {
            raw.to_string()
        };

        // Strip code fences
        if self.config.strip_code_fences {
            input = self.strip_code_fences(&input);
        }

        // Try direct parse first
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&input) {
            return Ok(value);
        }

        // Try to extract JSON from surrounding text
        if let Some(extracted) = self.extract_json(&input) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&extracted) {
                return Ok(value);
            }

            // Try repair on extracted JSON
            if self.config.attempt_repair {
                let repaired = self.repair_json(&extracted);
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&repaired) {
                    return Ok(value);
                }
            }
        }

        // Try repair on full input
        if self.config.attempt_repair {
            let repaired = self.repair_json(&input);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&repaired) {
                return Ok(value);
            }
        }

        Err(ParseError::InvalidFormat(
            "Failed to parse JSON after repair attempts".to_string(),
        ))
    }

    fn can_parse(&self, raw: &str) -> bool {
        let trimmed = raw.trim();
        trimmed.starts_with('{')
            || trimmed.starts_with('[')
            || trimmed.contains("```json")
            || trimmed.contains("```JSON")
    }

    fn name(&self) -> &'static str {
        "json"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_json() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"{"key": "value"}"#).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_with_code_fence() {
        let parser = JsonParser::new();
        let input = r#"Here is the JSON:
```json
{"key": "value"}
```"#;
        let result = parser.parse(input).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_trailing_comma() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"{"key": "value",}"#).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_single_quotes() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"{'key': 'value'}"#).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_unquoted_keys() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"{key: "value"}"#).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_with_comments() {
        let parser = JsonParser::new();
        let input = r#"{
            // This is a comment
            "key": "value"
        }"#;
        let result = parser.parse(input).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_missing_closing_brace() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"{"key": "value""#).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_json_in_text() {
        let parser = JsonParser::new();
        let input = r#"The result is: {"key": "value"} and that's it."#;
        let result = parser.parse(input).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_parse_array() {
        let parser = JsonParser::new();
        let result = parser.parse(r#"[1, 2, 3,]"#).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_empty_input() {
        let parser = JsonParser::new();
        let result = parser.parse("");
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_strict_parser_no_repair() {
        let parser = JsonParser::strict();
        let result = parser.parse(r#"{"key": "value",}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_can_parse() {
        let parser = JsonParser::new();
        assert!(parser.can_parse(r#"{"key": "value"}"#));
        assert!(parser.can_parse("```json\n{}```"));
        assert!(!parser.can_parse("just some text"));
    }

    #[test]
    fn test_nested_json() {
        let parser = JsonParser::new();
        let input = r#"{"outer": {"inner": "value"}}"#;
        let result = parser.parse(input).unwrap();
        assert_eq!(result["outer"]["inner"], "value");
    }
}
