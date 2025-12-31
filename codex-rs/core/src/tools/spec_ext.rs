//! Extension for tool loading logging
//!
//! Provides utilities to log loaded tools with metadata like shell variants
//! and execution modes. Also provides ext tool registration to minimize
//! modifications to spec.rs for easier upstream sync.

use crate::client_common::tools::ToolSpec;
use crate::subagent::AgentDefinition;
use crate::subagent::ToolAccess;
use crate::tools::names;
use crate::tools::registry::ConfiguredToolSpec;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec::ToolsConfig;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

// ============================================================================
// ToolFilter - Unified tool filtering for main session and subagents
// ============================================================================

/// Tools blocked for ALL subagents (recursive/dangerous/user-interactive).
///
/// This aligns with Claude Code's ALWAYS_EXCLUDED_TOOLS (chunks.107.mjs):
/// - Task, ExitPlanMode, EnterPlanMode, AskUserQuestion
///
/// Design principle: Only the main agent should interact with the user.
/// Subagents run in a restricted context and cannot request user input.
pub const ALWAYS_BLOCKED_TOOLS: &[&str] = &[
    names::TASK,              // Prevent recursive subagent spawning
    names::TASK_OUTPUT,       // Associated with Task
    names::UPDATE_PLAN,       // Main agent responsibility only
    names::BASH_OUTPUT,       // Background shell is main agent only
    names::KILL_SHELL,        // Background shell is main agent only
    names::ENTER_PLAN_MODE,   // Only main agent can request plan mode
    names::EXIT_PLAN_MODE,    // Only main agent can exit plan mode
    names::ASK_USER_QUESTION, // Only main agent can ask user questions
];

/// Unified tool filter - can be used by main session and subagents.
///
/// Design:
/// - Main session: `tool_filter = None` (no filtering)
/// - Subagent: `ToolFilter::from_agent_definition()` (security applied at construction)
/// - Plan Mode: `ToolFilter::for_plan_mode()` (read-only tools + plan file write)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolFilter {
    /// Whitelist: only allow these tools (None = allow all).
    pub allowed_tools: Option<HashSet<String>>,

    /// Blacklist: block these tools (includes security tiers at construction).
    pub blocked_tools: HashSet<String>,

    /// Plan file path for Plan Mode (write_file/smart_edit only allowed for this path).
    pub plan_file_path: Option<PathBuf>,
}

impl ToolFilter {
    /// Create a ToolFilter from an AgentDefinition.
    ///
    /// blocked = ALWAYS_BLOCKED + agent.disallowed_tools
    pub fn from_agent_definition(def: &AgentDefinition) -> Self {
        let mut blocked: HashSet<String> =
            ALWAYS_BLOCKED_TOOLS.iter().map(|s| s.to_string()).collect();

        // Add agent-specific disallowed tools
        blocked.extend(def.disallowed_tools.iter().cloned());

        Self {
            allowed_tools: match &def.tools {
                ToolAccess::All => None,
                ToolAccess::List(tools) => Some(tools.iter().cloned().collect()),
            },
            blocked_tools: blocked,
            plan_file_path: None,
        }
    }

    /// Create a ToolFilter for Plan Mode.
    ///
    /// Allows read-only tools + plan file write only.
    /// - think, read_file, list_dir, glob_files, grep_files
    /// - web_fetch, web_search, Task
    /// - exit_plan_mode
    /// - write_file/smart_edit only for plan file
    pub fn for_plan_mode(plan_file_path: Option<&Path>) -> Self {
        let allowed: HashSet<String> = [
            names::THINK,
            names::READ_FILE,
            names::LIST_DIR,
            names::GLOB_FILES,
            names::GREP_FILES,
            names::WEB_FETCH,
            names::WEB_SEARCH,
            names::TASK,
            names::EXIT_PLAN_MODE,
            // write_file and smart_edit are handled specially in is_allowed_in_plan_mode
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            allowed_tools: Some(allowed),
            blocked_tools: HashSet::new(),
            plan_file_path: plan_file_path.map(|p| p.to_path_buf()),
        }
    }

    /// Check if a tool is allowed by this filter.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // Step 1: Check blocked tools (includes security tiers)
        if self.blocked_tools.contains(tool_name) {
            return false;
        }

        // Step 2: Whitelist check
        match &self.allowed_tools {
            None => true,
            Some(allowed) => allowed.contains(tool_name),
        }
    }

    /// Check if a tool is allowed in Plan Mode.
    ///
    /// Special handling: write_file/smart_edit only allowed for plan file.
    pub fn is_allowed_in_plan_mode(&self, tool_name: &str, target_path: Option<&Path>) -> bool {
        // Special handling for write tools
        if tool_name == names::WRITE_FILE || tool_name == names::SMART_EDIT {
            return match (&self.plan_file_path, target_path) {
                (Some(plan_path), Some(target)) => plan_path == target,
                _ => false,
            };
        }

        self.is_allowed(tool_name)
    }
}

/// Extension trait for ToolsConfig to support tool filtering.
#[allow(dead_code)] // Extension point for tool filtering
pub trait ToolsConfigExt {
    /// Set a tool filter.
    fn with_tool_filter(self, filter: ToolFilter) -> Self;
}

impl ToolsConfigExt for ToolsConfig {
    fn with_tool_filter(mut self, filter: ToolFilter) -> Self {
        self.tool_filter = Some(filter);
        self
    }
}

/// Log loaded tools with variant annotations
pub fn log_loaded_tools(tools: &[ConfiguredToolSpec], model: &str) {
    let tool_displays: Vec<String> = tools
        .iter()
        .map(|tool| {
            let name = tool_name(&tool.spec);
            if let Some(variant) = tool_variant(&tool.spec) {
                format!("{name}[{variant}]")
            } else {
                name.to_string()
            }
        })
        .collect();

    info!(
        "[{model}] Loaded {} tools: [{}]",
        tools.len(),
        tool_displays.join(", ")
    );
}

/// Extract tool name from ToolSpec
fn tool_name(tool: &ToolSpec) -> &str {
    match tool {
        ToolSpec::Function(t) => &t.name,
        ToolSpec::LocalShell {} => "local_shell",
        ToolSpec::WebSearch {} => "web_search",
        ToolSpec::Freeform(t) => &t.name,
    }
}

/// Determine tool variant/execution mode annotation
fn tool_variant(tool: &ToolSpec) -> Option<&'static str> {
    match tool {
        ToolSpec::Function(t) => {
            match t.name.as_str() {
                // Shell variants
                "shell" => Some("array"),
                "shell_command" => Some("string"),
                "exec_command" => Some("PTY"),
                "write_stdin" => Some("PTY"),

                // MCP tools (server/tool_name format)
                name if name.contains('/') => Some("mcp"),

                // No variant annotation for other tools
                _ => None,
            }
        }
        ToolSpec::LocalShell {} => Some("API"),
        ToolSpec::WebSearch {} => Some("API"),
        ToolSpec::Freeform(_) => Some("freeform"),
    }
}

/// Try to register rich grep (ripgrep). Returns true if registered.
/// If false, caller should register the original grep_files handler.
pub fn try_register_rich_grep(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) -> bool {
    if config.include_rich_grep {
        use crate::tools::ext::ripgrep::create_ripgrep_tool;
        use crate::tools::handlers::ext::ripgrep::RipGrepHandler;
        builder.push_spec_with_parallel_support(create_ripgrep_tool(), true);
        builder.register_handler("grep_files", Arc::new(RipGrepHandler));
        true
    } else {
        false
    }
}

/// Try to register enhanced list_dir. Returns true if registered.
/// If false, caller should register the original list_dir handler.
pub fn try_register_enhanced_list_dir(
    builder: &mut ToolRegistryBuilder,
    config: &ToolsConfig,
) -> bool {
    if config.include_enhanced_list_dir {
        use crate::tools::ext::list_dir::create_enhanced_list_dir_tool;
        use crate::tools::handlers::ext::list_dir::EnhancedListDirHandler;
        builder.push_spec_with_parallel_support(create_enhanced_list_dir_tool(), true);
        builder.register_handler("list_dir", Arc::new(EnhancedListDirHandler));
        true
    } else {
        false
    }
}

/// Register smart_edit tool if enabled.
pub fn register_smart_edit(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_smart_edit {
        use crate::tools::ext::smart_edit::create_smart_edit_tool;
        use crate::tools::handlers::ext::smart_edit::SmartEditHandler;
        builder.push_spec(create_smart_edit_tool());
        builder.register_handler("smart_edit", Arc::new(SmartEditHandler));
    }
}

/// Register glob_files tool (always enabled).
pub fn register_glob_files(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::glob_files::create_glob_files_tool;
    use crate::tools::handlers::ext::glob_files::GlobFilesHandler;
    builder.push_spec_with_parallel_support(create_glob_files_tool(), true);
    builder.register_handler("glob_files", Arc::new(GlobFilesHandler));
}

/// Register think tool (always enabled for all models).
///
/// Think is a no-op tool that logs thoughts for transparency.
/// Useful for complex reasoning, brainstorming, and planning.
pub fn register_think(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::think::create_think_tool;
    use crate::tools::handlers::ext::think::ThinkHandler;
    builder.push_spec_with_parallel_support(create_think_tool(), true);
    builder.register_handler("think", Arc::new(ThinkHandler));
}

/// Register write_file tool (always enabled for all models).
///
/// Write File creates new files or overwrites existing files.
/// This is a mutating tool that requires approval.
pub fn register_write_file(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::write_file::create_write_file_tool;
    use crate::tools::handlers::ext::write_file::WriteFileHandler;
    builder.push_spec(create_write_file_tool());
    builder.register_handler("write_file", Arc::new(WriteFileHandler));
}

/// Register web_fetch tool if feature is enabled.
///
/// Web Fetch fetches content from URLs and converts HTML to plain text.
/// This is a mutating tool that requires approval.
pub fn register_web_fetch(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_web_fetch {
        use crate::tools::ext::web_fetch::create_web_fetch_tool;
        use crate::tools::handlers::ext::web_fetch::WebFetchHandler;
        builder.push_spec_with_parallel_support(create_web_fetch_tool(), true);
        builder.register_handler("web_fetch", Arc::new(WebFetchHandler));
    }
}

/// Register web_search tool if feature is enabled.
///
/// Web Search performs searches using DuckDuckGo or Tavily providers.
/// Returns formatted markdown results with sources and citations.
/// This is a mutating tool that requires approval.
pub fn register_web_search(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_web_search {
        use crate::tools::ext::web_search::create_web_search_tool;
        use crate::tools::handlers::ext::web_search::WebSearchHandler;

        builder.push_spec_with_parallel_support(create_web_search_tool(), true);
        builder.register_handler(
            "web_search",
            Arc::new(WebSearchHandler::new(config.web_search_config.clone())),
        );
    }
}

/// Register code_search tool.
///
/// Code Search searches the indexed codebase using BM25 and optional vector search.
/// Retrieval has its own independent configuration system (~/.codex/retrieval.toml).
/// Tool is always registered; handler checks availability at runtime.
pub fn register_code_search(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::code_search::create_code_search_tool;
    use crate::tools::handlers::ext::code_search::CodeSearchHandler;
    builder.push_spec_with_parallel_support(create_code_search_tool(), true);
    builder.register_handler("code_search", Arc::new(CodeSearchHandler::new()));
}

/// Register LSP tool for code intelligence.
///
/// LSP tool provides AI-friendly LSP operations using symbol name + kind matching.
/// Supports Rust (rust-analyzer), Go (gopls), and Python (pyright).
/// Requires LSP servers to be pre-installed.
pub fn register_lsp(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_lsp {
        use crate::tools::ext::lsp::create_lsp_tool;
        use crate::tools::handlers::ext::lsp::LspHandler;
        builder.push_spec_with_parallel_support(create_lsp_tool(), true);
        builder.register_handler("lsp", Arc::new(LspHandler::new()));
    }
}

/// Register subagent tools (Task, TaskOutput).
///
/// Task spawns specialized subagents for complex, multi-step tasks.
/// TaskOutput retrieves results from background subagent tasks.
///
/// Note: Stores are managed globally via conversation_id in subagent/stores.rs.
/// This avoids per-turn recreation and ensures background tasks persist across turns.
pub fn register_subagent_tools(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_subagent {
        use crate::subagent::get_builtin_agents;
        use crate::tools::ext::subagent::create_task_output_tool;
        use crate::tools::ext::subagent::create_task_tool_with_description;
        use crate::tools::ext::subagent::generate_task_description;
        use crate::tools::handlers::ext::subagent::TaskHandler;
        use crate::tools::handlers::ext::subagent::TaskOutputHandler;

        // Generate dynamic description from built-in agents
        let builtin_agents = get_builtin_agents();
        let description = generate_task_description(&builtin_agents);

        // Task tool - spawns subagents (supports parallel execution)
        builder
            .push_spec_with_parallel_support(create_task_tool_with_description(description), true);
        builder.register_handler("Task", Arc::new(TaskHandler::new()));

        // TaskOutput tool - retrieves background task results (supports parallel execution)
        builder.push_spec_with_parallel_support(create_task_output_tool(), true);
        builder.register_handler("TaskOutput", Arc::new(TaskOutputHandler::new()));
    }
}

/// Register background shell tools (BashOutput, KillShell).
///
/// BashOutput retrieves output from background shell commands.
/// KillShell terminates running background shell commands.
///
/// Note: Uses a global store (shell_background::get_global_shell_store).
/// These tools are main-agent only (blocked for subagents via ALWAYS_BLOCKED_TOOLS).
pub fn register_background_shell_tools(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    if config.include_background_shell {
        use crate::tools::ext::bash_output::create_bash_output_tool;
        use crate::tools::ext::kill_shell::create_kill_shell_tool;
        use crate::tools::handlers::ext::bash_output::BashOutputHandler;
        use crate::tools::handlers::ext::kill_shell::KillShellHandler;

        // BashOutput tool - retrieves background shell output (supports parallel execution)
        builder.push_spec_with_parallel_support(create_bash_output_tool(), true);
        builder.register_handler("BashOutput", Arc::new(BashOutputHandler::new()));

        // KillShell tool - terminates running background shells (supports parallel execution)
        builder.push_spec_with_parallel_support(create_kill_shell_tool(), true);
        builder.register_handler("KillShell", Arc::new(KillShellHandler::new()));
    }
}

/// Register exit_plan_mode tool (always enabled).
///
/// This tool allows the LLM to request exiting plan mode and trigger user approval.
pub fn register_exit_plan_mode(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::exit_plan_mode::create_exit_plan_mode_tool;
    use crate::tools::handlers::ext::exit_plan_mode::ExitPlanModeHandler;

    builder.push_spec(create_exit_plan_mode_tool());
    builder.register_handler(names::EXIT_PLAN_MODE, Arc::new(ExitPlanModeHandler));
}

/// Register enter_plan_mode tool (always enabled).
///
/// This tool allows the LLM to request entering plan mode and trigger user approval.
pub fn register_enter_plan_mode(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::enter_plan_mode::create_enter_plan_mode_tool;
    use crate::tools::handlers::ext::enter_plan_mode::EnterPlanModeHandler;

    builder.push_spec(create_enter_plan_mode_tool());
    builder.register_handler(names::ENTER_PLAN_MODE, Arc::new(EnterPlanModeHandler));
}

/// Register ask_user_question tool (always enabled).
///
/// This tool allows the LLM to ask the user questions during execution.
pub fn register_ask_user_question(builder: &mut ToolRegistryBuilder) {
    use crate::tools::ext::ask_user_question::create_ask_user_question_tool;
    use crate::tools::handlers::ext::ask_user_question::AskUserQuestionHandler;

    builder.push_spec(create_ask_user_question_tool());
    builder.register_handler(names::ASK_USER_QUESTION, Arc::new(AskUserQuestionHandler));
}

/// Register all extension tools.
/// This consolidates all ext tool registrations into a single call
/// to minimize modifications to spec.rs::build_specs().
pub fn register_ext_tools(builder: &mut ToolRegistryBuilder, config: &ToolsConfig) {
    // smart_edit: requires feature flag and model support
    register_smart_edit(builder, config);

    // glob_files: always enabled for all models
    register_glob_files(builder);

    // think: always enabled for all models
    register_think(builder);

    // write_file: always enabled for all models
    register_write_file(builder);

    // web_fetch: requires feature flag
    register_web_fetch(builder, config);

    // web_search: requires feature flag
    register_web_search(builder, config);

    // code_search: requires feature flag
    if config.include_code_search {
        register_code_search(builder);
    }

    // lsp: requires feature flag
    register_lsp(builder, config);

    // subagent tools: requires feature flag
    register_subagent_tools(builder, config);

    // background shell tools: requires feature flag
    register_background_shell_tools(builder, config);

    // exit_plan_mode: always enabled for plan mode support
    register_exit_plan_mode(builder);

    // enter_plan_mode: always enabled for plan mode support
    register_enter_plan_mode(builder);

    // ask_user_question: always enabled for user interaction
    register_ask_user_question(builder);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_common::tools::ResponsesApiTool;
    use crate::tools::spec::JsonSchema;

    #[test]
    fn test_tool_variant_detection() {
        let shell = ToolSpec::Function(ResponsesApiTool {
            name: "shell".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_variant(&shell), Some("array"));

        let shell_command = ToolSpec::Function(ResponsesApiTool {
            name: "shell_command".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_variant(&shell_command), Some("string"));

        let exec_command = ToolSpec::Function(ResponsesApiTool {
            name: "exec_command".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_variant(&exec_command), Some("PTY"));

        let local_shell = ToolSpec::LocalShell {};
        assert_eq!(tool_variant(&local_shell), Some("API"));

        let web_search = ToolSpec::WebSearch {};
        assert_eq!(tool_variant(&web_search), Some("API"));

        let mcp_tool = ToolSpec::Function(ResponsesApiTool {
            name: "github/create_pr".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_variant(&mcp_tool), Some("mcp"));

        let regular_tool = ToolSpec::Function(ResponsesApiTool {
            name: "update_plan".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_variant(&regular_tool), None);
    }

    #[test]
    fn test_tool_name_extraction() {
        let function = ToolSpec::Function(ResponsesApiTool {
            name: "test_tool".to_string(),
            description: "".to_string(),
            strict: false,
            parameters: JsonSchema::String { description: None },
        });
        assert_eq!(tool_name(&function), "test_tool");

        assert_eq!(tool_name(&ToolSpec::LocalShell {}), "local_shell");
        assert_eq!(tool_name(&ToolSpec::WebSearch {}), "web_search");
    }

    // ========================================================================
    // ToolFilter tests
    // ========================================================================

    #[test]
    fn test_tool_filter_is_allowed() {
        use std::collections::HashSet;

        // Test with no filter (all allowed)
        let filter = ToolFilter::default();
        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("shell"));
        assert!(filter.is_allowed("any_tool"));

        // Test with allowed_tools set
        let filter = ToolFilter {
            allowed_tools: Some(HashSet::from([
                "read_file".to_string(),
                "glob_files".to_string(),
            ])),
            blocked_tools: HashSet::new(),
            plan_file_path: None,
        };
        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("glob_files"));
        assert!(!filter.is_allowed("shell"));
        assert!(!filter.is_allowed("write_file"));

        // Test with blocked_tools set
        let filter = ToolFilter {
            allowed_tools: None,
            blocked_tools: HashSet::from(["shell".to_string(), "write_file".to_string()]),
            plan_file_path: None,
        };
        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("glob_files"));
        assert!(!filter.is_allowed("shell"));
        assert!(!filter.is_allowed("write_file"));

        // Test with both allowed and blocked (blocked takes precedence)
        let filter = ToolFilter {
            allowed_tools: Some(HashSet::from([
                "read_file".to_string(),
                "shell".to_string(),
            ])),
            blocked_tools: HashSet::from(["shell".to_string()]),
            plan_file_path: None,
        };
        assert!(filter.is_allowed("read_file"));
        assert!(!filter.is_allowed("shell")); // blocked takes precedence
        assert!(!filter.is_allowed("glob_files")); // not in allowed list
    }

    #[test]
    fn test_build_specs_with_tool_filter() {
        use crate::config::test_config;
        use crate::features::Features;
        use crate::models_manager::manager::ModelsManager;
        use crate::tools::spec::ToolsConfigParams;
        use crate::tools::spec::build_specs;

        let config = test_config();
        let model_family = ModelsManager::construct_model_family_offline("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let mut tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &model_family,
            features: &features,
            web_search_config: None,
        });

        // Build without filter first to get baseline
        let (tools_unfiltered, _) = build_specs(&tools_config, None).build();
        let unfiltered_names: std::collections::HashSet<_> = tools_unfiltered
            .iter()
            .map(|t| tool_name(&t.spec).to_string())
            .collect();

        // Verify we have multiple tools
        assert!(unfiltered_names.len() > 3);
        assert!(unfiltered_names.contains("glob_files"));
        assert!(unfiltered_names.contains("think"));

        // Now apply a filter that only allows glob_files and think
        tools_config = tools_config.with_tool_filter(ToolFilter {
            allowed_tools: Some(std::collections::HashSet::from([
                "glob_files".to_string(),
                "think".to_string(),
            ])),
            blocked_tools: std::collections::HashSet::new(),
            plan_file_path: None,
        });

        let (tools_filtered, _) = build_specs(&tools_config, None).build();
        let filtered_names: std::collections::HashSet<_> = tools_filtered
            .iter()
            .map(|t| tool_name(&t.spec).to_string())
            .collect();

        // Should only have the allowed tools
        assert_eq!(filtered_names.len(), 2);
        assert!(filtered_names.contains("glob_files"));
        assert!(filtered_names.contains("think"));
        assert!(!filtered_names.contains("write_file"));
        assert!(!filtered_names.contains("shell_command"));
    }

    #[test]
    fn test_build_specs_with_blocked_tools() {
        use crate::config::test_config;
        use crate::features::Features;
        use crate::models_manager::manager::ModelsManager;
        use crate::tools::spec::ToolsConfigParams;
        use crate::tools::spec::build_specs;

        let config = test_config();
        let model_family = ModelsManager::construct_model_family_offline("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let mut tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &model_family,
            features: &features,
            web_search_config: None,
        });

        // Apply a filter that blocks shell_command and write_file
        tools_config = tools_config.with_tool_filter(ToolFilter {
            allowed_tools: None, // Allow all except blocked
            blocked_tools: std::collections::HashSet::from([
                "shell_command".to_string(),
                "write_file".to_string(),
            ]),
            plan_file_path: None,
        });

        let (tools_filtered, _) = build_specs(&tools_config, None).build();
        let filtered_names: std::collections::HashSet<_> = tools_filtered
            .iter()
            .map(|t| tool_name(&t.spec).to_string())
            .collect();

        // Should have tools except the blocked ones
        assert!(filtered_names.contains("glob_files"));
        assert!(filtered_names.contains("think"));
        assert!(!filtered_names.contains("shell_command"));
        assert!(!filtered_names.contains("write_file"));
    }
}
