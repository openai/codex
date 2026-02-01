//! TodoWrite tool for creating structured task lists.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for creating and managing a structured task list.
///
/// Replaces the entire task list atomically. Enforces max 1 in_progress task.
pub struct TodoWriteTool;

impl TodoWriteTool {
    /// Create a new TodoWrite tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    fn description(&self) -> &str {
        prompts::TODO_WRITE_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The full list of tasks",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Unique task identifier (auto-generated if omitted)"
                            },
                            "subject": {
                                "type": "string",
                                "description": "Brief task title in imperative form (e.g., 'Fix authentication bug')"
                            },
                            "description": {
                                "type": "string",
                                "description": "Detailed description of what needs to be done"
                            },
                            "content": {
                                "type": "string",
                                "description": "Task description (deprecated — use subject + description instead)"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Task status"
                            },
                            "activeForm": {
                                "type": "string",
                                "description": "Present continuous form shown in spinner when in_progress (e.g., 'Fixing authentication bug')"
                            }
                        },
                        "required": ["status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let todos = input["todos"].as_array().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "todos must be an array",
            }
            .build()
        })?;

        // Validate: max 1 in_progress task
        let in_progress_count = todos
            .iter()
            .filter(|t| t["status"].as_str() == Some("in_progress"))
            .count();

        if in_progress_count > 1 {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "At most 1 task can be in_progress at a time",
            }
            .build());
        }

        // Validate each todo has required fields
        for (i, todo) in todos.iter().enumerate() {
            let status = todo["status"].as_str().unwrap_or("");
            if !matches!(status, "pending" | "in_progress" | "completed") {
                return Err(crate::error::tool_error::InvalidInputSnafu {
                    message: format!("todo[{i}] invalid status: {status}"),
                }
                .build());
            }
            // Must have at least subject or content
            if todo["subject"].as_str().is_none() && todo["content"].as_str().is_none() {
                return Err(crate::error::tool_error::InvalidInputSnafu {
                    message: format!("todo[{i}] must have either 'subject' or 'content'"),
                }
                .build());
            }
        }

        ctx.emit_progress(format!("Updated task list ({} tasks)", todos.len()))
            .await;

        // Format output
        let mut output = String::new();
        for (i, todo) in todos.iter().enumerate() {
            let id = todo["id"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| format!("{}", i + 1));
            let title = todo["subject"]
                .as_str()
                .or_else(|| todo["content"].as_str())
                .unwrap_or("?");
            let status = todo["status"].as_str().unwrap_or("?");
            let marker = match status {
                "completed" => "[x]",
                "in_progress" => "[>]",
                _ => "[ ]",
            };
            output.push_str(&format!("{marker} {id}: {title}\n"));
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
    async fn test_todo_write() {
        let tool = TodoWriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "todos": [
                {"id": "1", "subject": "Fix bug", "status": "completed", "activeForm": "Fixing bug"},
                {"id": "2", "subject": "Add tests", "status": "in_progress", "activeForm": "Adding tests"},
                {"id": "3", "subject": "Deploy", "status": "pending", "activeForm": "Deploying"}
            ]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("[x]"));
        assert!(text.contains("[>]"));
        assert!(text.contains("[ ]"));
    }

    #[tokio::test]
    async fn test_todo_write_with_legacy_content() {
        let tool = TodoWriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "todos": [
                {"id": "1", "content": "Fix bug", "status": "completed"},
                {"id": "2", "content": "Add tests", "status": "pending"}
            ]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_todo_write_auto_id() {
        let tool = TodoWriteTool::new();
        let mut ctx = make_context();

        // No id field — should auto-generate
        let input = serde_json::json!({
            "todos": [
                {"subject": "Fix bug", "status": "pending"}
            ]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("1:"));
    }

    #[tokio::test]
    async fn test_todo_write_max_in_progress() {
        let tool = TodoWriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "todos": [
                {"subject": "Task 1", "status": "in_progress", "activeForm": "Working on Task 1"},
                {"subject": "Task 2", "status": "in_progress", "activeForm": "Working on Task 2"}
            ]
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = TodoWriteTool::new();
        assert_eq!(tool.name(), "TodoWrite");
        assert!(!tool.is_concurrent_safe());
    }
}
