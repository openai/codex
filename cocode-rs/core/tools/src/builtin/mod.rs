//! Built-in tools for the agent.
//!
//! This module provides the standard set of 20 built-in tools:
//! - [`ReadTool`] - Read file contents
//! - [`GlobTool`] - Pattern-based file search
//! - [`GrepTool`] - Content search with regex
//! - [`EditTool`] - Exact string replacement in files
//! - [`WriteTool`] - Write/create files
//! - [`BashTool`] - Execute shell commands
//! - [`ShellTool`] - Execute commands via array format (direct exec)
//! - [`TaskTool`] - Launch sub-agents
//! - [`TaskOutputTool`] - Get background task output
//! - [`KillShellTool`] - Stop background tasks
//! - [`TodoWriteTool`] - Manage task lists
//! - [`EnterPlanModeTool`] - Enter plan mode
//! - [`ExitPlanModeTool`] - Exit plan mode
//! - [`AskUserQuestionTool`] - Ask interactive questions
//! - [`WebFetchTool`] - Fetch and process web content
//! - [`WebSearchTool`] - Search the web
//! - [`SkillTool`] - Execute named skills (slash commands)
//! - [`LspTool`] - Language Server Protocol operations (feature-gated)
//! - [`McpSearchTool`] - Search MCP tools by keyword (dynamic, for auto-search mode)
//! - [`LsTool`] - List directory contents with tree-style output
//! - [`ApplyPatchTool`] - Apply multi-file patches (optional, for GPT-5)
//!
//! ## Utilities
//!
//! - [`path_extraction::LlmPathExtractor`] - LLM-based file path extraction from command output

mod prompts;

mod apply_patch;
mod ask_user_question;
mod bash;
mod edit;
mod enter_plan_mode;
mod exit_plan_mode;
mod glob;
mod grep;
mod kill_shell;
mod ls;
mod lsp;
pub mod mcp_search;
mod notebook_edit;
pub mod path_extraction;
mod read;
mod shell;
mod skill;
mod task;
mod task_output;
mod todo_write;
mod web_fetch;
mod web_search;
mod write;

pub use apply_patch::ApplyPatchTool;
pub use ask_user_question::AskUserQuestionTool;
pub use bash::BashTool;
pub use edit::EditTool;
pub use enter_plan_mode::EnterPlanModeTool;
pub use exit_plan_mode::ExitPlanModeTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use kill_shell::KillShellTool;
pub use ls::LsTool;
pub use lsp::LspTool;
pub use mcp_search::McpSearchTool;
pub use notebook_edit::NotebookEditTool;
pub use read::ReadTool;
pub use shell::ShellTool;
pub use skill::SkillTool;
pub use task::TaskTool;
pub use task_output::TaskOutputTool;
pub use todo_write::TodoWriteTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearchTool;
pub use write::WriteTool;

use crate::registry::ToolRegistry;

/// Register all built-in tools with a registry.
///
/// All tools including `apply_patch` are always registered. Which tool
/// definitions are sent to a model is decided at request time by
/// `select_tools_for_model()` based on `ModelInfo.apply_patch_tool_type`.
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(ReadTool::new());
    registry.register(GlobTool::new());
    registry.register(GrepTool::new());
    registry.register(EditTool::new());
    registry.register(WriteTool::new());
    registry.register(BashTool::new());
    registry.register(TaskTool::new());
    registry.register(TaskOutputTool::new());
    registry.register(KillShellTool::new());
    registry.register(TodoWriteTool::new());
    registry.register(EnterPlanModeTool::new());
    registry.register(ExitPlanModeTool::new());
    registry.register(AskUserQuestionTool::new());
    registry.register(WebFetchTool::new());
    registry.register(WebSearchTool::new());
    registry.register(SkillTool::new());
    registry.register(LsTool::new());
    registry.register(LspTool::new());
    registry.register(NotebookEditTool::new());
    registry.register(ApplyPatchTool::new());
    registry.register(ShellTool::new());
}

/// Get a list of built-in tool names.
pub fn builtin_tool_names() -> Vec<&'static str> {
    vec![
        "Read",
        "Glob",
        "Grep",
        "Edit",
        "Write",
        "Bash",
        "Task",
        "TaskOutput",
        "TaskStop",
        "TodoWrite",
        "EnterPlanMode",
        "ExitPlanMode",
        "AskUserQuestion",
        "WebFetch",
        "WebSearch",
        "Skill",
        "LS",
        "Lsp",
        "NotebookEdit",
        "apply_patch",
        "shell",
    ]
}
