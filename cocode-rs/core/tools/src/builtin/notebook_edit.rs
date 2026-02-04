//! NotebookEdit tool for modifying Jupyter notebook cells.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_plan_mode::is_safe_file;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ContextModifier;
use cocode_protocol::ToolOutput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::fs;

/// Tool for editing Jupyter notebook (.ipynb) cells.
///
/// Supports replacing, inserting, and deleting cells in notebooks.
pub struct NotebookEditTool;

impl NotebookEditTool {
    /// Create a new NotebookEdit tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NotebookEditTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Edit mode for notebook operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Replace the content of an existing cell.
    #[default]
    Replace,
    /// Insert a new cell.
    Insert,
    /// Delete an existing cell.
    Delete,
}

impl EditMode {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "replace" => Some(EditMode::Replace),
            "insert" => Some(EditMode::Insert),
            "delete" => Some(EditMode::Delete),
            _ => None,
        }
    }
}

/// Cell type in a Jupyter notebook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellType {
    /// Code cell.
    Code,
    /// Markdown cell.
    Markdown,
    /// Raw cell.
    Raw,
}

impl CellType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "code" => Some(CellType::Code),
            "markdown" => Some(CellType::Markdown),
            "raw" => Some(CellType::Raw),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            CellType::Code => "code",
            CellType::Markdown => "markdown",
            CellType::Raw => "raw",
        }
    }
}

/// Generate a simple cell ID using timestamp and counter.
fn generate_cell_id() -> String {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("{timestamp:x}-{counter:x}")
}

/// A Jupyter notebook cell structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotebookCell {
    cell_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    metadata: Value,
    source: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outputs: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution_count: Option<Value>,
}

impl NotebookCell {
    /// Create a new cell with the given type and source.
    fn new(cell_type: CellType, source: Vec<String>) -> Self {
        let source_value = if source.len() == 1 {
            Value::String(source.into_iter().next().unwrap_or_default())
        } else {
            Value::Array(source.into_iter().map(Value::String).collect())
        };

        Self {
            cell_type: cell_type.as_str().to_string(),
            id: Some(generate_cell_id()),
            metadata: Value::Object(serde_json::Map::new()),
            source: source_value,
            outputs: if cell_type == CellType::Code {
                Some(Vec::new())
            } else {
                None
            },
            execution_count: if cell_type == CellType::Code {
                Some(Value::Null)
            } else {
                None
            },
        }
    }

    /// Set the cell source from a string.
    fn set_source(&mut self, content: &str) {
        // Split into lines, preserving newlines for all but the last line
        let lines: Vec<String> = content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                // Add newline to all lines except possibly the last
                if i < content.lines().count() - 1 || content.ends_with('\n') {
                    format!("{line}\n")
                } else {
                    line.to_string()
                }
            })
            .collect();

        self.source = if lines.len() == 1 {
            Value::String(lines.into_iter().next().unwrap_or_default())
        } else {
            Value::Array(lines.into_iter().map(Value::String).collect())
        };
    }
}

/// A Jupyter notebook structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Notebook {
    cells: Vec<NotebookCell>,
    metadata: Value,
    nbformat: u32,
    nbformat_minor: u32,
}

impl Notebook {
    /// Find a cell by its ID.
    fn find_cell_index(&self, cell_id: &str) -> Option<usize> {
        self.cells
            .iter()
            .position(|c| c.id.as_ref().is_some_and(|id| id == cell_id))
    }

    /// Find a cell by index (0-based).
    fn cell_at_index(&mut self, index: usize) -> Option<&mut NotebookCell> {
        self.cells.get_mut(index)
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    fn description(&self) -> &str {
        prompts::NOTEBOOK_EDIT_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "notebook_path": {
                    "type": "string",
                    "description": "The absolute path to the Jupyter notebook file to edit (must be absolute, not relative)"
                },
                "cell_id": {
                    "type": "string",
                    "description": "The ID of the cell to edit. When inserting a new cell, the new cell will be inserted after the cell with this ID, or at the beginning if not specified."
                },
                "cell_number": {
                    "type": "integer",
                    "description": "The 0-indexed cell number to edit. Can be used instead of cell_id. When inserting, the new cell is inserted at this position."
                },
                "cell_type": {
                    "type": "string",
                    "enum": ["code", "markdown"],
                    "description": "The type of the cell (code or markdown). If not specified, it defaults to the current cell type. If using edit_mode=insert, this is required."
                },
                "new_source": {
                    "type": "string",
                    "description": "The new source for the cell"
                },
                "edit_mode": {
                    "type": "string",
                    "enum": ["replace", "insert", "delete"],
                    "description": "The type of edit to make (replace, insert, delete). Defaults to replace."
                }
            },
            "required": ["notebook_path", "new_source"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let notebook_path = input["notebook_path"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "notebook_path must be a string",
            }
            .build()
        })?;
        let new_source = input["new_source"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "new_source must be a string",
            }
            .build()
        })?;
        let cell_id = input["cell_id"].as_str();
        let cell_number = input["cell_number"].as_u64().map(|n| n as usize);
        let cell_type_str = input["cell_type"].as_str();
        let edit_mode_str = input["edit_mode"].as_str().unwrap_or("replace");

        let edit_mode = EditMode::from_str(edit_mode_str).ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: format!(
                    "Invalid edit_mode: {edit_mode_str}. Must be replace, insert, or delete."
                ),
            }
            .build()
        })?;

        let path = ctx.resolve_path(notebook_path);

        // Verify it's a .ipynb file
        if path.extension().is_none_or(|ext| ext != "ipynb") {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "NotebookEdit can only be used with .ipynb files",
            }
            .build());
        }

        // Plan mode check
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
                    "Notebook must be read before editing: {}. Use the Read tool first.",
                    path.display()
                ),
            }
            .build());
        }

        // Read the notebook
        let content = fs::read_to_string(&path).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to read notebook: {e}"),
            }
            .build()
        })?;

        let mut notebook: Notebook = serde_json::from_str(&content).map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to parse notebook JSON: {e}"),
            }
            .build()
        })?;

        match edit_mode {
            EditMode::Replace => {
                let cell_index = if let Some(id) = cell_id {
                    notebook.find_cell_index(id).ok_or_else(|| {
                        crate::error::tool_error::ExecutionFailedSnafu {
                            message: format!("Cell with ID '{id}' not found in notebook"),
                        }
                        .build()
                    })?
                } else if let Some(num) = cell_number {
                    if num >= notebook.cells.len() {
                        return Err(crate::error::tool_error::ExecutionFailedSnafu {
                            message: format!(
                                "Cell number {num} out of bounds (notebook has {} cells)",
                                notebook.cells.len()
                            ),
                        }
                        .build());
                    }
                    num
                } else {
                    return Err(crate::error::tool_error::InvalidInputSnafu {
                        message: "cell_id or cell_number is required for replace mode",
                    }
                    .build());
                };

                let cell = notebook.cell_at_index(cell_index).ok_or_else(|| {
                    crate::error::tool_error::ExecutionFailedSnafu {
                        message: "Cell not found at index",
                    }
                    .build()
                })?;

                // Update cell type if specified
                if let Some(ct_str) = cell_type_str {
                    if let Some(ct) = CellType::from_str(ct_str) {
                        cell.cell_type = ct.as_str().to_string();
                    }
                }

                // Update source
                cell.set_source(new_source);
            }
            EditMode::Insert => {
                // cell_type is required for insert
                let cell_type = cell_type_str
                    .and_then(CellType::from_str)
                    .ok_or_else(|| {
                        crate::error::tool_error::InvalidInputSnafu {
                            message: "cell_type is required for insert mode (must be 'code' or 'markdown')",
                        }
                        .build()
                    })?;

                // Create new cell
                let lines: Vec<String> = new_source.lines().map(|l| format!("{l}\n")).collect();
                let new_cell = NotebookCell::new(cell_type, lines);

                // Find insert position: cell_id inserts AFTER that cell, cell_number inserts AT that position
                let insert_index = if let Some(id) = cell_id {
                    notebook.find_cell_index(id).map(|i| i + 1).unwrap_or(0)
                } else if let Some(num) = cell_number {
                    // Insert at position num (clamped to valid range)
                    num.min(notebook.cells.len())
                } else {
                    // Default: insert at beginning
                    0
                };

                notebook.cells.insert(insert_index, new_cell);
            }
            EditMode::Delete => {
                let cell_index = if let Some(id) = cell_id {
                    notebook.find_cell_index(id).ok_or_else(|| {
                        crate::error::tool_error::ExecutionFailedSnafu {
                            message: format!("Cell with ID '{id}' not found in notebook"),
                        }
                        .build()
                    })?
                } else if let Some(num) = cell_number {
                    if num >= notebook.cells.len() {
                        return Err(crate::error::tool_error::ExecutionFailedSnafu {
                            message: format!(
                                "Cell number {num} out of bounds (notebook has {} cells)",
                                notebook.cells.len()
                            ),
                        }
                        .build());
                    }
                    num
                } else {
                    return Err(crate::error::tool_error::InvalidInputSnafu {
                        message: "cell_id or cell_number is required for delete mode",
                    }
                    .build());
                };

                notebook.cells.remove(cell_index);
            }
        }

        // Serialize and write back
        let new_content = serde_json::to_string_pretty(&notebook).map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to serialize notebook: {e}"),
            }
            .build()
        })?;

        fs::write(&path, &new_content).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to write notebook: {e}"),
            }
            .build()
        })?;

        // Track modification
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

        let action = match edit_mode {
            EditMode::Replace => "replaced cell in",
            EditMode::Insert => "inserted cell into",
            EditMode::Delete => "deleted cell from",
        };

        let mut result = ToolOutput::text(format!("Successfully {action} {}", path.display()));
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
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    fn create_test_notebook() -> String {
        serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "id": "cell-1",
                    "metadata": {},
                    "source": ["# Test Notebook\n"]
                },
                {
                    "cell_type": "code",
                    "id": "cell-2",
                    "metadata": {},
                    "source": ["print('hello')\n"],
                    "outputs": [],
                    "execution_count": null
                }
            ],
            "metadata": {
                "kernelspec": {
                    "display_name": "Python 3",
                    "language": "python",
                    "name": "python3"
                }
            },
            "nbformat": 4,
            "nbformat_minor": 5
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_replace_cell() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_id": "cell-2",
            "new_source": "print('modified')",
            "edit_mode": "replace"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        // Verify the content was changed
        let content = std::fs::read_to_string(&notebook_path).unwrap();
        assert!(content.contains("modified"));
        assert!(!content.contains("hello"));
    }

    #[tokio::test]
    async fn test_insert_cell() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_id": "cell-1",
            "cell_type": "code",
            "new_source": "# New cell",
            "edit_mode": "insert"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        // Verify the cell was inserted
        let content = std::fs::read_to_string(&notebook_path).unwrap();
        let notebook: Notebook = serde_json::from_str(&content).unwrap();
        assert_eq!(notebook.cells.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_cell() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_id": "cell-2",
            "new_source": "",
            "edit_mode": "delete"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        // Verify the cell was deleted
        let content = std::fs::read_to_string(&notebook_path).unwrap();
        let notebook: Notebook = serde_json::from_str(&content).unwrap();
        assert_eq!(notebook.cells.len(), 1);
    }

    #[tokio::test]
    async fn test_requires_read_first() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        // Don't read the file first

        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_id": "cell-2",
            "new_source": "print('modified')",
            "edit_mode": "replace"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rejects_non_ipynb() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.py");
        std::fs::write(&file_path, "print('hello')").unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "notebook_path": file_path.to_str().unwrap(),
            "cell_id": "cell-1",
            "new_source": "print('modified')"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains(".ipynb"));
    }

    #[test]
    fn test_tool_properties() {
        let tool = NotebookEditTool::new();
        assert_eq!(tool.name(), "NotebookEdit");
        assert!(!tool.is_concurrent_safe());
        assert!(!tool.is_read_only());
    }

    #[tokio::test]
    async fn test_replace_cell_by_number() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        // Replace cell at index 1 (the code cell)
        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_number": 1,
            "new_source": "print('replaced by number')",
            "edit_mode": "replace"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&notebook_path).unwrap();
        assert!(content.contains("replaced by number"));
        assert!(!content.contains("hello"));
    }

    #[tokio::test]
    async fn test_insert_cell_by_number() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        // Insert cell at position 1 (between markdown and code cells)
        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_number": 1,
            "cell_type": "code",
            "new_source": "# Inserted at position 1",
            "edit_mode": "insert"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&notebook_path).unwrap();
        let notebook: Notebook = serde_json::from_str(&content).unwrap();
        assert_eq!(notebook.cells.len(), 3);
        // The new cell should be at index 1
        assert!(
            notebook.cells[1]
                .source
                .to_string()
                .contains("Inserted at position 1")
        );
    }

    #[tokio::test]
    async fn test_delete_cell_by_number() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        // Delete cell at index 0 (the markdown cell)
        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_number": 0,
            "new_source": "",
            "edit_mode": "delete"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&notebook_path).unwrap();
        let notebook: Notebook = serde_json::from_str(&content).unwrap();
        assert_eq!(notebook.cells.len(), 1);
        // Only the code cell should remain
        assert_eq!(notebook.cells[0].cell_type, "code");
    }

    #[tokio::test]
    async fn test_cell_number_out_of_bounds() {
        let dir = TempDir::new().unwrap();
        let notebook_path = dir.path().join("test.ipynb");
        std::fs::write(&notebook_path, create_test_notebook()).unwrap();

        let tool = NotebookEditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&notebook_path).await;

        // Try to replace cell at index 99 (out of bounds)
        let input = serde_json::json!({
            "notebook_path": notebook_path.to_str().unwrap(),
            "cell_number": 99,
            "new_source": "should fail",
            "edit_mode": "replace"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }
}
