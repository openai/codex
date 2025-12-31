//! Centralized tool name constants.
//!
//! All tool names should be defined here to avoid hardcoded strings
//! and ensure consistency across the codebase.

#![allow(dead_code)] // Tool name constants registry for future use

// File/Directory Tools
pub const READ_FILE: &str = "read_file";
pub const LIST_DIR: &str = "list_dir";
pub const GLOB_FILES: &str = "glob_files";
pub const GREP_FILES: &str = "grep_files";
pub const WRITE_FILE: &str = "write_file";

// Shell Tools
pub const SHELL: &str = "shell";
pub const SHELL_COMMAND: &str = "shell_command";
pub const EXEC_COMMAND: &str = "exec_command";
pub const WRITE_STDIN: &str = "write_stdin";

// Code Modification Tools
pub const APPLY_PATCH: &str = "apply_patch";
pub const SMART_EDIT: &str = "smart_edit";

// Web Tools
pub const WEB_FETCH: &str = "web_fetch";
pub const WEB_SEARCH: &str = "web_search";

// Code Intelligence Tools
pub const LSP: &str = "lsp";
pub const CODE_SEARCH: &str = "code_search";

// Task/Agent Tools
pub const TASK: &str = "Task";
pub const TASK_OUTPUT: &str = "TaskOutput";

// Background Shell Tools
pub const BASH_OUTPUT: &str = "BashOutput";
pub const KILL_SHELL: &str = "KillShell";

// Utility Tools
pub const THINK: &str = "think";
pub const VIEW_IMAGE: &str = "view_image";
pub const UPDATE_PLAN: &str = "update_plan";

// Plan Mode Tools
pub const EXIT_PLAN_MODE: &str = "ExitPlanMode";
pub const ENTER_PLAN_MODE: &str = "EnterPlanMode";

// User Interaction Tools
pub const ASK_USER_QUESTION: &str = "AskUserQuestion";

// MCP Resource Tools
pub const LIST_MCP_RESOURCES: &str = "list_mcp_resources";
pub const LIST_MCP_RESOURCE_TEMPLATES: &str = "list_mcp_resource_templates";
pub const READ_MCP_RESOURCE: &str = "read_mcp_resource";

// Testing Tools
pub const TEST_SYNC_TOOL: &str = "test_sync_tool";
