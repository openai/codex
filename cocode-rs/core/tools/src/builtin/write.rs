//! Write tool for creating or overwriting files.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ContextModifier, ToolOutput};
use serde_json::Value;
use tokio::fs;

/// Tool for writing files to the local filesystem.
///
/// For existing files, requires the file to have been read first.
pub struct WriteTool;

impl WriteTool {
    /// Create a new Write tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        prompts::WRITE_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let file_path = input["file_path"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("file_path must be a string"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("content must be a string"))?;

        let path = ctx.resolve_path(file_path);

        // If file exists, must have been read first
        if path.exists() {
            if !ctx.was_file_read(&path).await {
                return Err(ToolError::execution_failed(format!(
                    "Existing file must be read before overwriting: {}. Use the Read tool first.",
                    path.display()
                )));
            }

            // Check file_mtime hasn't changed since last read (detect external modifications)
            let current_mtime = fs::metadata(&path)
                .await
                .ok()
                .and_then(|m| m.modified().ok());
            if let Some(read_state) = ctx.file_read_state(&path).await {
                if let (Some(read_mtime), Some(curr_mtime)) = (read_state.file_mtime, current_mtime)
                {
                    if curr_mtime > read_mtime {
                        return Err(ToolError::execution_failed(format!(
                            "File has been modified externally since last read: {}. Read the file again before writing.",
                            path.display()
                        )));
                    }
                }
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    ToolError::execution_failed(format!("Failed to create directory: {e}"))
                })?;
            }
        }

        // Write file
        fs::write(&path, content)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Failed to write file: {e}")))?;

        // Track modification and update read state with new content/mtime
        ctx.record_file_modified(&path).await;
        let new_mtime = fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        use crate::context::FileReadState;
        ctx.record_file_read_with_state(
            &path,
            FileReadState::complete(content.to_string(), new_mtime),
        )
        .await;

        let mut result = ToolOutput::text(format!("Successfully wrote to {}", path.display()));
        result.modifiers.push(ContextModifier::FileRead {
            path: path.clone(),
            content: content.to_string(),
        });

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");

        let tool = WriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "Hello World"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello World");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("sub").join("dir").join("test.txt");

        let tool = WriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "nested content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "nested content");
    }

    #[tokio::test]
    async fn test_write_existing_requires_read() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "original").unwrap();

        let tool = WriteTool::new();
        let mut ctx = make_context();
        // Don't read the file first

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "overwritten"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_existing_after_read() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "original").unwrap();

        let tool = WriteTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "overwritten"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "overwritten");
    }

    #[test]
    fn test_tool_properties() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "Write");
        assert!(!tool.is_concurrent_safe());
    }
}
