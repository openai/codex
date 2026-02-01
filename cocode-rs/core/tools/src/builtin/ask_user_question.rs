//! AskUserQuestion tool for interactive user queries.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for asking the user questions during execution.
///
/// Supports multiple questions with selectable options,
/// including multi-select and custom "Other" input.
pub struct AskUserQuestionTool;

impl AskUserQuestionTool {
    /// Create a new AskUserQuestion tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AskUserQuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "AskUserQuestion"
    }

    fn description(&self) -> &str {
        prompts::ASK_USER_QUESTION_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "Questions to ask the user (1-4 questions)",
                    "minItems": 1,
                    "maxItems": 4,
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask the user. Should be clear, specific, and end with a question mark."
                            },
                            "header": {
                                "type": "string",
                                "description": "Very short label displayed as a chip/tag (max 12 chars). Examples: 'Auth method', 'Library', 'Approach'."
                            },
                            "options": {
                                "type": "array",
                                "description": "The available choices for this question (2-4 options).",
                                "minItems": 2,
                                "maxItems": 4,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "The display text for this option (1-5 words)."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means or what will happen if chosen."
                                        }
                                    },
                                    "required": ["label", "description"]
                                }
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "description": "Set to true to allow multiple options to be selected. Default: false.",
                                "default": false
                            }
                        },
                        "required": ["question", "header", "options", "multiSelect"]
                    }
                },
                "answers": {
                    "type": "object",
                    "description": "User answers collected by the UI (filled on callback)."
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata for tracking purposes.",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Optional identifier for the source of this question."
                        }
                    }
                }
            },
            "required": ["questions"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let questions = input["questions"].as_array().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "questions must be an array",
            }
            .build()
        })?;

        if questions.is_empty() || questions.len() > 4 {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "questions must contain 1-4 items",
            }
            .build());
        }

        // Validate each question
        for (i, q) in questions.iter().enumerate() {
            if q["question"].as_str().is_none() {
                return Err(crate::error::tool_error::InvalidInputSnafu {
                    message: format!("questions[{i}] missing required field 'question'"),
                }
                .build());
            }
            if q["header"].as_str().is_none() {
                return Err(crate::error::tool_error::InvalidInputSnafu {
                    message: format!("questions[{i}] missing required field 'header'"),
                }
                .build());
            }
            let options = q["options"].as_array().ok_or_else(|| {
                crate::error::tool_error::InvalidInputSnafu {
                    message: format!("questions[{i}] missing 'options' array"),
                }
                .build()
            })?;
            if options.len() < 2 || options.len() > 4 {
                return Err(crate::error::tool_error::InvalidInputSnafu {
                    message: format!("questions[{i}] must have 2-4 options"),
                }
                .build());
            }
        }

        // If answers are provided, this is a callback with user responses
        if let Some(answers) = input.get("answers").and_then(|a| a.as_object()) {
            let mut output = String::from("User responses:\n");
            for (key, value) in answers {
                output.push_str(&format!("- {key}: {value}\n"));
            }
            return Ok(ToolOutput::text(output));
        }

        // Otherwise, emit event for UI to display the question
        ctx.emit_progress("Asking user a question").await;

        // Format the questions for text output (stub — UI integration will handle rich display)
        let mut output = String::new();
        for q in questions {
            let question = q["question"].as_str().unwrap_or("?");
            let header = q["header"].as_str().unwrap_or("");
            output.push_str(&format!("[{header}] {question}\n"));
            if let Some(options) = q["options"].as_array() {
                for (i, opt) in options.iter().enumerate() {
                    let label = opt["label"].as_str().unwrap_or("?");
                    let desc = opt["description"].as_str().unwrap_or("");
                    output.push_str(&format!("  {}. {label} — {desc}\n", i + 1));
                }
            }
            output.push('\n');
        }

        Ok(ToolOutput::text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_ask_user_question() {
        let tool = AskUserQuestionTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "questions": [{
                "question": "Which library should we use?",
                "header": "Library",
                "options": [
                    {"label": "React", "description": "Popular UI framework"},
                    {"label": "Vue", "description": "Progressive framework"}
                ],
                "multiSelect": false
            }]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Which library"));
        assert!(text.contains("React"));
    }

    #[tokio::test]
    async fn test_ask_user_question_with_answers() {
        let tool = AskUserQuestionTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "questions": [{
                "question": "Which library?",
                "header": "Library",
                "options": [
                    {"label": "React", "description": "Popular"},
                    {"label": "Vue", "description": "Progressive"}
                ],
                "multiSelect": false
            }],
            "answers": {
                "Library": "React"
            }
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("React"));
    }

    #[tokio::test]
    async fn test_ask_user_question_validation() {
        let tool = AskUserQuestionTool::new();
        let mut ctx = make_context();

        // Too few options
        let input = serde_json::json!({
            "questions": [{
                "question": "Which?",
                "header": "Q",
                "options": [{"label": "A", "description": "a"}],
                "multiSelect": false
            }]
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = AskUserQuestionTool::new();
        assert_eq!(tool.name(), "AskUserQuestion");
        assert!(!tool.is_concurrent_safe());
        assert!(!tool.is_read_only());
    }
}
