//! ReAct format parser
//!
//! Parses the ReAct (Reasoning + Acting) format commonly used for agent traces.
//!
//! Format:
//! ```text
//! Thought: I need to search for information
//! Action: search
//! Action Input: query string
//! Observation: search results
//! Thought: Now I know the answer
//! Action: final_answer
//! Action Input: The answer is 42
//! ```

use regex::Regex;
use std::sync::LazyLock;

use super::parser::{OutputParser, ParseError, ParseResult};

/// Type of ReAct step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReActStepType {
    /// Reasoning step
    Thought,
    /// Action to take
    Action,
    /// Input for the action
    ActionInput,
    /// Result of action
    Observation,
    /// Final answer
    FinalAnswer,
}

impl ReActStepType {
    /// Check if this is a terminal step
    pub fn is_terminal(&self) -> bool {
        matches!(self, ReActStepType::FinalAnswer)
    }
}

/// A single step in a ReAct trace
#[derive(Debug, Clone)]
pub struct ReActStep {
    /// Step type
    pub step_type: ReActStepType,
    /// Step content
    pub content: String,
}

impl ReActStep {
    /// Create a new ReAct step
    pub fn new(step_type: ReActStepType, content: impl Into<String>) -> Self {
        Self {
            step_type,
            content: content.into(),
        }
    }
}

/// ReAct format parser
pub struct ReActParser {
    /// Require Thought before Action
    require_thought: bool,
}

impl ReActParser {
    /// Create a new ReAct parser
    pub fn new() -> Self {
        Self {
            require_thought: false,
        }
    }

    /// Require Thought before each Action
    pub fn require_thought(mut self) -> Self {
        self.require_thought = true;
        self
    }

    /// Extract the final answer if present
    pub fn get_final_answer(&self, raw: &str) -> ParseResult<Option<String>> {
        let steps = self.parse(raw)?;

        Ok(steps
            .into_iter()
            .find(|s| s.step_type == ReActStepType::FinalAnswer)
            .map(|s| s.content))
    }

    /// Get the last action and input
    pub fn get_last_action(&self, raw: &str) -> ParseResult<Option<(String, String)>> {
        let steps = self.parse(raw)?;

        let mut action: Option<String> = None;
        let mut action_input: Option<String> = None;

        for step in steps.iter().rev() {
            match step.step_type {
                ReActStepType::ActionInput if action_input.is_none() => {
                    action_input = Some(step.content.clone());
                }
                ReActStepType::Action if action.is_none() => {
                    action = Some(step.content.clone());
                    if action_input.is_some() {
                        break;
                    }
                }
                _ => {}
            }
        }

        Ok(action.and_then(|a| action_input.map(|i| (a, i))))
    }

    /// Check if the trace is complete (has final answer)
    pub fn is_complete(&self, raw: &str) -> bool {
        self.get_final_answer(raw)
            .map(|a| a.is_some())
            .unwrap_or(false)
    }
}

impl Default for ReActParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for ReActParser {
    type Output = Vec<ReActStep>;

    fn parse(&self, raw: &str) -> ParseResult<Self::Output> {
        if raw.trim().is_empty() {
            return Err(ParseError::EmptyInput);
        }

        static STEP_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?i)^(Thought|Action|Action\s*Input|Observation|Final\s*Answer)\s*:\s*(.*)$")
                .unwrap()
        });

        let mut steps = Vec::new();
        let mut current_type: Option<ReActStepType> = None;
        let mut current_content = String::new();

        for line in raw.lines() {
            if let Some(caps) = STEP_RE.captures(line) {
                // Save previous step
                if let Some(step_type) = current_type.take() {
                    steps.push(ReActStep::new(step_type, current_content.trim()));
                    current_content.clear();
                }

                let step_type_str = caps.get(1).unwrap().as_str().to_lowercase();
                let content = caps.get(2).unwrap().as_str().to_string();

                let step_type = match step_type_str.as_str() {
                    "thought" => ReActStepType::Thought,
                    "action" => ReActStepType::Action,
                    s if s.contains("input") => ReActStepType::ActionInput,
                    "observation" => ReActStepType::Observation,
                    s if s.contains("answer") => ReActStepType::FinalAnswer,
                    _ => continue,
                };

                current_type = Some(step_type);
                current_content = content;
            } else if current_type.is_some() {
                // Continuation of previous step
                current_content.push('\n');
                current_content.push_str(line);
            }
        }

        // Don't forget the last step
        if let Some(step_type) = current_type {
            steps.push(ReActStep::new(step_type, current_content.trim()));
        }

        if steps.is_empty() {
            return Err(ParseError::InvalidFormat(
                "No ReAct steps found".to_string(),
            ));
        }

        // Validate thought-before-action if required
        if self.require_thought {
            let mut saw_thought = false;
            for step in &steps {
                match step.step_type {
                    ReActStepType::Thought => saw_thought = true,
                    ReActStepType::Action if !saw_thought => {
                        return Err(ParseError::InvalidFormat(
                            "Action without preceding Thought".to_string(),
                        ));
                    }
                    ReActStepType::Observation => saw_thought = false,
                    _ => {}
                }
            }
        }

        Ok(steps)
    }

    fn can_parse(&self, raw: &str) -> bool {
        let lower = raw.to_lowercase();
        lower.contains("thought:") || lower.contains("action:") || lower.contains("final answer:")
    }

    fn name(&self) -> &'static str {
        "react"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_react() {
        let parser = ReActParser::new();
        let input = r#"Thought: I need to search for the answer
Action: search
Action Input: what is 2+2
Observation: 2+2 equals 4
Thought: I now know the answer
Final Answer: 4"#;

        let steps = parser.parse(input).unwrap();
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0].step_type, ReActStepType::Thought);
        assert_eq!(steps[1].step_type, ReActStepType::Action);
        assert_eq!(steps[1].content, "search");
        assert_eq!(steps[5].step_type, ReActStepType::FinalAnswer);
        assert_eq!(steps[5].content, "4");
    }

    #[test]
    fn test_get_final_answer() {
        let parser = ReActParser::new();
        let input = r#"Thought: thinking
Action: search
Action Input: query
Final Answer: The answer is 42"#;

        let answer = parser.get_final_answer(input).unwrap();
        assert_eq!(answer.unwrap(), "The answer is 42");
    }

    #[test]
    fn test_get_last_action() {
        let parser = ReActParser::new();
        let input = r#"Thought: thinking
Action: search
Action Input: first query
Observation: result
Thought: need more
Action: lookup
Action Input: second query"#;

        let (action, input_str) = parser.get_last_action(input).unwrap().unwrap();
        assert_eq!(action, "lookup");
        assert_eq!(input_str, "second query");
    }

    #[test]
    fn test_is_complete() {
        let parser = ReActParser::new();

        let incomplete = "Thought: thinking\nAction: search\nAction Input: query";
        assert!(!parser.is_complete(incomplete));

        let complete = "Thought: thinking\nFinal Answer: done";
        assert!(parser.is_complete(complete));
    }

    #[test]
    fn test_multiline_content() {
        let parser = ReActParser::new();
        let input = r#"Thought: I need to think about this
This is a multi-line thought
with several lines
Action: done"#;

        let steps = parser.parse(input).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(steps[0].content.contains("multi-line"));
    }

    #[test]
    fn test_case_insensitive() {
        let parser = ReActParser::new();
        let input = "THOUGHT: caps\nACTION: test\nACTION INPUT: data";

        let steps = parser.parse(input).unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn test_empty_input() {
        let parser = ReActParser::new();
        let result = parser.parse("");
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_can_parse() {
        let parser = ReActParser::new();
        assert!(parser.can_parse("Thought: something"));
        assert!(parser.can_parse("Action: do"));
        assert!(parser.can_parse("Final Answer: done"));
        assert!(!parser.can_parse("Just regular text"));
    }
}
