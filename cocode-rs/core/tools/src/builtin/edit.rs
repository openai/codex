//! Edit tool for exact string replacement in files.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_plan_mode::is_safe_file;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ContextModifier;
use cocode_protocol::ToolOutput;
use serde_json::Value;
use tokio::fs;

/// Tool for performing exact string replacements in files.
///
/// Requires the file to have been read first (tracked via FileTracker).
pub struct EditTool;

impl EditTool {
    /// Create a new Edit tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        prompts::EDIT_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "file_path must be a string",
            }
            .build()
        })?;
        let old_string = input["old_string"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "old_string must be a string",
            }
            .build()
        })?;
        let new_string = input["new_string"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "new_string must be a string",
            }
            .build()
        })?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        if old_string == new_string {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "old_string and new_string must be different",
            }
            .build());
        }

        let path = ctx.resolve_path(file_path);

        // Plan mode check: only allow edits to the plan file
        if ctx.is_plan_mode {
            if !is_safe_file(&path, ctx.plan_file_path.as_deref()) {
                return Err(crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!(
                        "Plan mode: cannot edit '{}'. Only the plan file can be modified during plan mode.",
                        path.display()
                    ),
                }
                .build());
            }
        }

        // Verify file was read first
        if !ctx.was_file_read(&path).await {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "File must be read before editing: {}. Use the Read tool first.",
                    path.display()
                ),
            }
            .build());
        }

        // Check file_mtime hasn't changed since last read (detect external modifications)
        let current_mtime = fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        if let Some(read_state) = ctx.file_read_state(&path).await {
            if let (Some(read_mtime), Some(curr_mtime)) = (read_state.file_mtime, current_mtime) {
                if curr_mtime > read_mtime {
                    return Err(crate::error::tool_error::ExecutionFailedSnafu {
                        message: format!(
                            "File has been modified externally since last read: {}. Read the file again before editing.",
                            path.display()
                        ),
                    }
                    .build());
                }
            }
        }

        // Read current content
        let content = fs::read_to_string(&path).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to read file: {e}"),
            }
            .build()
        })?;

        // Check that old_string exists
        if !content.contains(old_string) {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("old_string not found in file: {}", path.display()),
            }
            .build());
        }

        // Check uniqueness if not replace_all
        if !replace_all {
            let count = content.matches(old_string).count();
            if count > 1 {
                return Err(crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!(
                        "old_string is not unique in the file ({count} occurrences). \
                         Provide more context to make it unique, or use replace_all."
                    ),
                }
                .build());
            }
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        // Write back
        fs::write(&path, &new_content).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to write file: {e}"),
            }
            .build()
        })?;

        // Track modification and update read state with new content/mtime
        ctx.record_file_modified(&path).await;
        let new_mtime = fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        use crate::context::FileReadState;
        ctx.record_file_read_with_state(
            &path,
            FileReadState::complete(new_content.clone(), new_mtime),
        )
        .await;

        let mut result = ToolOutput::text(format!("Successfully edited {}", path.display()));
        result.modifiers.push(ContextModifier::FileRead {
            path: path.clone(),
            content: new_content,
        });

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_edit_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(content, "Hello Rust");
    }

    #[tokio::test]
    async fn test_edit_requires_read_first() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        // Don't read the file first

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_non_unique_string() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "foo bar foo").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "foo",
            "new_string": "baz"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "foo bar foo").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "foo",
            "new_string": "baz",
            "replace_all": true
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(content, "baz bar baz");
    }

    #[test]
    fn test_tool_properties() {
        let tool = EditTool::new();
        assert_eq!(tool.name(), "Edit");
        assert!(!tool.is_concurrent_safe());
    }

    #[tokio::test]
    async fn test_plan_mode_blocks_non_plan_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();
        let plan_file = PathBuf::from("/tmp/plan.md");

        let tool = EditTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file));
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Plan mode"));
    }

    #[tokio::test]
    async fn test_plan_mode_allows_plan_file() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let plan_file = dir.path().join("plan.md");
        std::fs::write(&plan_file, "# Plan\n\nold content").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file.clone()));
        ctx.record_file_read(&plan_file).await;

        let input = serde_json::json!({
            "file_path": plan_file.to_str().unwrap(),
            "old_string": "old content",
            "new_string": "new content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&plan_file).unwrap();
        assert!(content.contains("new content"));
    }

    #[tokio::test]
    async fn test_non_plan_mode_allows_any_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        // is_plan_mode = false (default)
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }
}
