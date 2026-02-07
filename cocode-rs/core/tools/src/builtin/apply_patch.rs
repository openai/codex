//! Apply patch tool for batch file modifications.
//!
//! Supports both JSON function mode and freeform (Lark grammar) mode.
//! For OpenAI models (especially GPT-5), this can replace the Edit tool
//! with a more powerful batch-editing capability.

use super::prompts;
use crate::ToolDefinition;
use crate::context::FileReadState;
use crate::context::ToolContext;
use crate::error::Result;
use crate::error::tool_error;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_apply_patch::ApplyPatchFileChange;
use cocode_apply_patch::MaybeApplyPatchVerified;
use cocode_apply_patch::apply_patch as execute_patch;
use cocode_apply_patch::maybe_parse_apply_patch_verified;
use cocode_plan_mode::is_safe_file;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ContextModifier;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for applying multi-file patches.
///
/// This tool allows batch modifications to multiple files using a unified
/// diff-like format. It supports:
/// - Adding new files
/// - Deleting existing files
/// - Updating file contents with context-aware patches
/// - Moving/renaming files
///
/// The handler auto-detects input format (JSON object vs raw string).
/// Which tool **definition** is sent to a model is decided by
/// `select_tools_for_model()` based on `ModelInfo.apply_patch_tool_type`.
#[derive(Default)]
pub struct ApplyPatchTool;

impl ApplyPatchTool {
    pub fn new() -> Self {
        Self
    }

    /// Get the Function variant tool definition (JSON schema with "input" field).
    pub fn function_definition() -> ToolDefinition {
        ToolDefinition::full(
            "apply_patch",
            prompts::APPLY_PATCH_DESCRIPTION,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The entire contents of the apply_patch command"
                    }
                },
                "required": ["input"]
            }),
        )
    }

    /// Get the Freeform variant tool definition (custom tool with Lark grammar).
    pub fn freeform_definition() -> ToolDefinition {
        let lark_grammar = include_str!("tool_apply_patch.lark");
        ToolDefinition::custom(
            "apply_patch",
            prompts::APPLY_PATCH_FREEFORM_DESCRIPTION,
            serde_json::json!({
                "type": "grammar",
                "syntax": "lark",
                "definition": lark_grammar
            }),
        )
    }
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        prompts::APPLY_PATCH_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "The entire contents of the apply_patch command"
                }
            },
            "required": ["input"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        // TODO(sandbox): Current implementation executes patches directly in-process.
        //
        // codex-rs's apply_patch uses subprocess execution (unlike read_file/write_file/smart_edit):
        // 1. assess_patch_safety() determines if approval is needed
        // 2. SafetyCheck::Reject → return error directly, no execution
        // 3. SafetyCheck::AutoApprove/AskUser → DelegateToExec → ApplyPatchRuntime
        // 4. ApplyPatchRuntime spawns subprocess: codex --codex-run-as-apply-patch "<patch>"
        // 5. Subprocess can be wrapped in sandbox to restrict filesystem access
        //
        // When cocode-rs needs sandbox support, implement:
        // 1. Add InternalApplyPatchInvocation enum (Output vs DelegateToExec)
        // 2. Add assess_patch_safety() safety check
        // 3. Add ApplyPatchRuntime (build_command_spec)
        // 4. Connect arg0 dispatch (exists: cocode-rs/exec/arg0/src/lib.rs)
        // 5. Add user approval flow with caching
        //
        // Reference: codex-rs/core/src/tools/handlers/apply_patch.rs
        //            codex-rs/core/src/tools/runtimes/apply_patch.rs

        // 1. Extract patch content: auto-detect JSON object vs string input
        let patch_input = if input.is_string() {
            // Freeform mode: direct string input
            input.as_str().unwrap().to_string()
        } else {
            // Function mode: JSON object with "input" field
            input["input"]
                .as_str()
                .ok_or_else(|| {
                    tool_error::InvalidInputSnafu {
                        message: "input field must be a string",
                    }
                    .build()
                })?
                .to_string()
        };

        // 2. Parse and verify the patch
        let argv = vec!["apply_patch".to_string(), patch_input.clone()];
        let cwd = ctx.cwd.clone();

        match maybe_parse_apply_patch_verified(&argv, &cwd) {
            MaybeApplyPatchVerified::Body(action) => {
                // 3. Plan mode check: only allow modifications to plan file
                if ctx.is_plan_mode {
                    for path in action.changes().keys() {
                        if !is_safe_file(path, ctx.plan_file_path.as_deref()) {
                            return Err(tool_error::ExecutionFailedSnafu {
                                message: format!(
                                    "Plan mode: cannot modify '{}'. Only the plan file can be modified.",
                                    path.display()
                                ),
                            }
                            .build());
                        }
                    }
                }

                // 4. Execute the patch
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();

                match execute_patch(&patch_input, &mut stdout, &mut stderr) {
                    Ok(()) => {
                        // 5. Track modifications and update read state
                        let mut result_modifiers = Vec::new();

                        for (path, change) in action.changes() {
                            ctx.record_file_modified(path).await;

                            // Update read state for files that now have content
                            match change {
                                ApplyPatchFileChange::Add { content }
                                | ApplyPatchFileChange::Update {
                                    new_content: content,
                                    ..
                                } => {
                                    let mtime = tokio::fs::metadata(path)
                                        .await
                                        .ok()
                                        .and_then(|m| m.modified().ok());
                                    ctx.record_file_read_with_state(
                                        path,
                                        FileReadState::complete(content.clone(), mtime),
                                    )
                                    .await;

                                    // Add context modifier for the updated content
                                    result_modifiers.push(ContextModifier::FileRead {
                                        path: path.clone(),
                                        content: content.clone(),
                                    });
                                }
                                ApplyPatchFileChange::Delete { .. } => {
                                    // File was deleted, no content to track
                                }
                            }
                        }

                        let output_text = String::from_utf8_lossy(&stdout).to_string();
                        let mut result = ToolOutput::text(output_text);
                        result.modifiers = result_modifiers;

                        Ok(result)
                    }
                    Err(e) => {
                        let error_text = String::from_utf8_lossy(&stderr).to_string();
                        Err(tool_error::ExecutionFailedSnafu {
                            message: format!("Patch failed: {e}\n{error_text}"),
                        }
                        .build())
                    }
                }
            }
            MaybeApplyPatchVerified::CorrectnessError(e) => Err(tool_error::ExecutionFailedSnafu {
                message: format!("Patch verification failed: {e}"),
            }
            .build()),
            MaybeApplyPatchVerified::ShellParseError(e) => Err(tool_error::InvalidInputSnafu {
                message: format!("Failed to parse patch input: {e:?}"),
            }
            .build()),
            MaybeApplyPatchVerified::NotApplyPatch => Err(tool_error::InvalidInputSnafu {
                message: "Input is not a valid apply_patch command",
            }
            .build()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context(cwd: PathBuf) -> ToolContext {
        ToolContext::new("call-1", "session-1", cwd)
    }

    #[test]
    fn test_tool_properties() {
        let tool = ApplyPatchTool::new();
        assert_eq!(tool.name(), "apply_patch");
        assert!(!tool.is_concurrent_safe());
        assert!(!tool.is_read_only());
    }

    #[test]
    fn test_input_schema() {
        let tool = ApplyPatchTool::new();
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["input"].is_object());
    }

    #[test]
    fn test_function_definition() {
        let def = ApplyPatchTool::function_definition();
        assert_eq!(def.name, "apply_patch");
        assert_eq!(def.parameters["type"], "object");
        assert!(def.parameters["properties"]["input"].is_object());
    }

    #[test]
    fn test_freeform_definition() {
        let def = ApplyPatchTool::freeform_definition();
        assert_eq!(def.name, "apply_patch");
        assert!(def.custom_format.is_some());
        let format = def.custom_format.unwrap();
        assert_eq!(format["type"], "grammar");
        assert_eq!(format["syntax"], "lark");
        assert!(
            format["definition"]
                .as_str()
                .unwrap()
                .contains("Begin Patch")
        );
    }

    #[tokio::test]
    async fn test_apply_patch_add_file() {
        let dir = TempDir::new().unwrap();
        let new_file = dir.path().join("hello.txt");

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+Hello, world!\n*** End Patch",
            new_file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await.unwrap();

        assert!(!result.is_error);
        assert!(new_file.exists());
        let content = fs::read_to_string(&new_file).unwrap();
        assert_eq!(content, "Hello, world!\n");
    }

    #[tokio::test]
    async fn test_apply_patch_update_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("update.txt");
        fs::write(&file, "foo\nbar\n").unwrap();

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let patch = format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n foo\n-bar\n+baz\n*** End Patch",
            file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await.unwrap();

        assert!(!result.is_error);
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "foo\nbaz\n");
    }

    #[tokio::test]
    async fn test_apply_patch_delete_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("delete.txt");
        fs::write(&file, "to be deleted").unwrap();

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let patch = format!(
            "*** Begin Patch\n*** Delete File: {}\n*** End Patch",
            file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await.unwrap();

        assert!(!result.is_error);
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn test_apply_patch_freeform_mode() {
        let dir = TempDir::new().unwrap();
        let new_file = dir.path().join("freeform.txt");

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+Freeform content\n*** End Patch",
            new_file.display()
        );

        // Auto-detect: string input is treated as freeform
        let input = serde_json::Value::String(patch);
        let result = tool.execute(input, &mut ctx).await.unwrap();

        assert!(!result.is_error);
        assert!(new_file.exists());
        let content = fs::read_to_string(&new_file).unwrap();
        assert_eq!(content, "Freeform content\n");
    }

    #[tokio::test]
    async fn test_plan_mode_blocks_non_plan_file() {
        let dir = TempDir::new().unwrap();
        let new_file = dir.path().join("blocked.txt");
        let plan_file = dir.path().join("plan.md");
        fs::write(&plan_file, "# Plan").unwrap();

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf()).with_plan_mode(true, Some(plan_file));

        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+Should be blocked\n*** End Patch",
            new_file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Plan mode"));
        assert!(!new_file.exists());
    }

    #[tokio::test]
    async fn test_plan_mode_allows_plan_file() {
        let dir = TempDir::new().unwrap();
        let plan_file = dir.path().join("plan.md");
        fs::write(&plan_file, "# Plan\nold content\n").unwrap();

        let tool = ApplyPatchTool::new();
        let mut ctx =
            make_context(dir.path().to_path_buf()).with_plan_mode(true, Some(plan_file.clone()));

        let patch = format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n # Plan\n-old content\n+new content\n*** End Patch",
            plan_file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await.unwrap();

        assert!(!result.is_error);
        let content = fs::read_to_string(&plan_file).unwrap();
        assert!(content.contains("new content"));
    }

    #[tokio::test]
    async fn test_invalid_patch_returns_error() {
        let dir = TempDir::new().unwrap();

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({ "input": "not a valid patch" });
        let result = tool.execute(input, &mut ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_context_modifiers_added() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("modified.txt");

        let tool = ApplyPatchTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+content\n*** End Patch",
            file.display()
        );

        let input = serde_json::json!({ "input": patch });
        let result = tool.execute(input, &mut ctx).await.unwrap();

        // Should have a FileRead context modifier
        assert!(!result.modifiers.is_empty());
        let has_file_read = result
            .modifiers
            .iter()
            .any(|m| matches!(m, ContextModifier::FileRead { path, .. } if *path == file));
        assert!(has_file_read);
    }
}
