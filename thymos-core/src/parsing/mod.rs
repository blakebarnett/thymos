//! Structured Output Parsing
//!
//! Robust parsing of LLM outputs with fuzzy repair and validation.
//!
//! # Features
//!
//! - **Fuzzy JSON parsing**: Handles markdown fences, trailing commas, etc.
//! - **Schema validation**: Validate against expected structure
//! - **Multiple formats**: JSON, Markdown sections, ReAct
//! - **Graceful degradation**: Falls back to raw output on failure
//!
//! # Example
//!
//! ```rust,ignore
//! use thymos_core::parsing::{JsonParser, OutputParser};
//!
//! let parser = JsonParser::new();
//! let result = parser.parse("```json\n{\"key\": \"value\",}\n```")?;
//! assert_eq!(result["key"], "value");
//! ```

mod json;
mod markdown;
mod parser;
mod react;

pub use json::JsonParser;
pub use markdown::{MarkdownParser, MarkdownSection};
pub use parser::{OutputParser, ParseError, ParseResult};
pub use react::{ReActParser, ReActStep, ReActStepType};
