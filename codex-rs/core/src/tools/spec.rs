use crate::client_common::tools::ToolSpec;
use crate::config::AgentRoleConfig;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::original_image_detail::can_request_original_image_detail;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::discoverable::DiscoverablePluginInfo;
use crate::tools::discoverable::DiscoverableTool;
use crate::tools::discoverable::DiscoverableToolAction;
use crate::tools::discoverable::DiscoverableToolType;
use crate::tools::handlers::PLAN_TOOL;
use crate::tools::handlers::TOOL_SEARCH_DEFAULT_LIMIT;
use crate::tools::handlers::TOOL_SEARCH_TOOL_NAME;
use crate::tools::handlers::TOOL_SUGGEST_TOOL_NAME;
use crate::tools::handlers::agent_jobs::BatchJobHandler;
use crate::tools::handlers::apply_patch::create_apply_patch_freeform_tool;
use crate::tools::handlers::apply_patch::create_apply_patch_json_tool;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::request_permissions_tool_description;
use crate::tools::handlers::request_user_input_tool_description;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::registry::tool_handler_key;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::WebSearchConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_tools::CommandToolOptions;
use codex_tools::FreeformTool;
use codex_tools::FreeformToolFormat;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ShellToolOptions;
use codex_tools::SpawnAgentToolOptions;
use codex_tools::ViewImageToolOptions;
use codex_tools::WaitAgentTimeoutOptions;
use codex_tools::augment_tool_spec_for_code_mode;
use codex_tools::create_assign_task_tool;
use codex_tools::create_close_agent_tool_v1;
use codex_tools::create_close_agent_tool_v2;
use codex_tools::create_exec_command_tool;
use codex_tools::create_list_agents_tool as create_list_agents_tool_v2;
use codex_tools::create_report_agent_job_result_tool;
use codex_tools::create_request_permissions_tool;
use codex_tools::create_request_user_input_tool;
use codex_tools::create_resume_agent_tool;
use codex_tools::create_send_input_tool_v1;
use codex_tools::create_send_message_tool;
use codex_tools::create_shell_command_tool;
use codex_tools::create_shell_tool;
use codex_tools::create_spawn_agent_tool_v1;
use codex_tools::create_spawn_agent_tool_v2;
use codex_tools::create_spawn_agents_on_csv_tool;
use codex_tools::create_view_image_tool;
use codex_tools::create_wait_agent_tool_v1;
use codex_tools::create_wait_agent_tool_v2;
use codex_tools::create_write_stdin_tool;
use codex_tools::dynamic_tool_to_responses_api_tool;
use codex_tools::mcp_tool_to_responses_api_tool;
use codex_tools::tool_spec_to_code_mode_tool_definition;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_template::Template;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;

pub type JsonSchema = codex_tools::JsonSchema;

#[cfg(test)]
pub(crate) use codex_tools::mcp_call_tool_result_output_schema;

const TOOL_SEARCH_DESCRIPTION_TEMPLATE_SOURCE: &str =
    include_str!("../../templates/search_tool/tool_description.md");
const TOOL_SEARCH_DESCRIPTION_TEMPLATE_KEY: &str = "app_descriptions";
const AGENT_TOOLS_NAMESPACE: &str = "agents";
const AGENT_TOOLS_NAMESPACE_DESCRIPTION: &str = "Agent collaboration tools for spawning, messaging, waiting on, listing, and closing subagents.";
static TOOL_SEARCH_DESCRIPTION_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(TOOL_SEARCH_DESCRIPTION_TEMPLATE_SOURCE)
        .unwrap_or_else(|err| panic!("tool_search description template must parse: {err}"))
});
const TOOL_SUGGEST_DESCRIPTION_TEMPLATE_SOURCE: &str =
    include_str!("../../templates/search_tool/tool_suggest_description.md");
const TOOL_SUGGEST_DESCRIPTION_TEMPLATE_KEY: &str = "discoverable_tools";
static TOOL_SUGGEST_DESCRIPTION_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(TOOL_SUGGEST_DESCRIPTION_TEMPLATE_SOURCE)
        .unwrap_or_else(|err| panic!("tool_suggest description template must parse: {err}"))
});
const WEB_SEARCH_CONTENT_TYPES: [&str; 2] = ["text", "image"];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShellCommandBackendConfig {
    Classic,
    ZshFork,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UnifiedExecShellMode {
    Direct,
    ZshFork(ZshForkConfig),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ZshForkConfig {
    pub(crate) shell_zsh_path: AbsolutePathBuf,
    pub(crate) main_execve_wrapper_exe: AbsolutePathBuf,
}

impl UnifiedExecShellMode {
    pub fn for_session(
        shell_command_backend: ShellCommandBackendConfig,
        user_shell: &Shell,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
    ) -> Self {
        if cfg!(unix)
            && shell_command_backend == ShellCommandBackendConfig::ZshFork
            && matches!(user_shell.shell_type, ShellType::Zsh)
            && let (Some(shell_zsh_path), Some(main_execve_wrapper_exe)) =
                (shell_zsh_path, main_execve_wrapper_exe)
            && let (Ok(shell_zsh_path), Ok(main_execve_wrapper_exe)) = (
                AbsolutePathBuf::try_from(shell_zsh_path.as_path())
                    .inspect_err(|e| tracing::warn!("Failed to convert shell_zsh_path `{shell_zsh_path:?}`: {e:?}")),
                AbsolutePathBuf::try_from(main_execve_wrapper_exe.as_path()).inspect_err(|e| {
                    tracing::warn!("Failed to convert main_execve_wrapper_exe `{main_execve_wrapper_exe:?}`: {e:?}")
                }),
            )
        {
            Self::ZshFork(ZshForkConfig {
                shell_zsh_path,
                main_execve_wrapper_exe,
            })
        } else {
            Self::Direct
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolsConfig {
    pub available_models: Vec<ModelPreset>,
    pub shell_type: ConfigShellToolType,
    shell_command_backend: ShellCommandBackendConfig,
    pub unified_exec_shell_mode: UnifiedExecShellMode,
    pub allow_login_shell: bool,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_mode: Option<WebSearchMode>,
    pub web_search_config: Option<WebSearchConfig>,
    pub web_search_tool_type: WebSearchToolType,
    pub image_gen_tool: bool,
    pub agent_roles: BTreeMap<String, AgentRoleConfig>,
    pub search_tool: bool,
    pub tool_suggest: bool,
    pub exec_permission_approvals_enabled: bool,
    pub request_permissions_tool_enabled: bool,
    pub code_mode_enabled: bool,
    pub code_mode_only_enabled: bool,
    pub js_repl_enabled: bool,
    pub js_repl_tools_only: bool,
    pub can_request_original_image_detail: bool,
    pub collab_tools: bool,
    pub multi_agent_v2: bool,
    pub agent_watchdog: bool,
    pub request_user_input: bool,
    pub default_mode_request_user_input: bool,
    pub experimental_supported_tools: Vec<String>,
    pub agent_jobs_tools: bool,
    pub agent_jobs_worker_tools: bool,
}

pub(crate) struct ToolsConfigParams<'a> {
    pub(crate) model_info: &'a ModelInfo,
    pub(crate) available_models: &'a Vec<ModelPreset>,
    pub(crate) features: &'a Features,
    pub(crate) web_search_mode: Option<WebSearchMode>,
    pub(crate) session_source: SessionSource,
    pub(crate) sandbox_policy: &'a SandboxPolicy,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
}

fn unified_exec_allowed_in_environment(
    is_windows: bool,
    sandbox_policy: &SandboxPolicy,
    windows_sandbox_level: WindowsSandboxLevel,
) -> bool {
    !(is_windows
        && windows_sandbox_level != WindowsSandboxLevel::Disabled
        && !matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        ))
}

impl ToolsConfig {
    pub fn new(params: &ToolsConfigParams) -> Self {
        let ToolsConfigParams {
            model_info,
            available_models: available_models_ref,
            features,
            web_search_mode,
            session_source,
            sandbox_policy,
            windows_sandbox_level,
        } = params;
        let include_apply_patch_tool = features.enabled(Feature::ApplyPatchFreeform);
        let include_code_mode = features.enabled(Feature::CodeMode);
        let include_code_mode_only = include_code_mode && features.enabled(Feature::CodeModeOnly);
        let include_js_repl = features.enabled(Feature::JsRepl);
        let include_js_repl_tools_only =
            include_js_repl && features.enabled(Feature::JsReplToolsOnly);
        let include_collab_tools = features.enabled(Feature::Collab);
        let include_multi_agent_v2 = features.enabled(Feature::MultiAgentV2);
        let include_agent_watchdog =
            include_collab_tools && features.enabled(Feature::AgentWatchdog);
        let include_agent_jobs = features.enabled(Feature::SpawnCsv);
        let include_request_user_input = !matches!(session_source, SessionSource::SubAgent(_));
        let include_default_mode_request_user_input =
            include_request_user_input && features.enabled(Feature::DefaultModeRequestUserInput);
        let include_search_tool =
            model_info.supports_search_tool && features.enabled(Feature::ToolSearch);
        let include_tool_suggest = features.enabled(Feature::ToolSuggest)
            && features.enabled(Feature::Apps)
            && features.enabled(Feature::Plugins);
        let include_original_image_detail = can_request_original_image_detail(features, model_info);
        let include_image_gen_tool =
            features.enabled(Feature::ImageGeneration) && supports_image_generation(model_info);
        let exec_permission_approvals_enabled = features.enabled(Feature::ExecPermissionApprovals);
        let request_permissions_tool_enabled = features.enabled(Feature::RequestPermissionsTool);
        let shell_command_backend =
            if features.enabled(Feature::ShellTool) && features.enabled(Feature::ShellZshFork) {
                ShellCommandBackendConfig::ZshFork
            } else {
                ShellCommandBackendConfig::Classic
            };
        let unified_exec_allowed = unified_exec_allowed_in_environment(
            cfg!(target_os = "windows"),
            sandbox_policy,
            *windows_sandbox_level,
        );
        let shell_type = if !features.enabled(Feature::ShellTool) {
            ConfigShellToolType::Disabled
        } else if features.enabled(Feature::ShellZshFork) {
            ConfigShellToolType::ShellCommand
        } else if features.enabled(Feature::UnifiedExec) && unified_exec_allowed {
            // If ConPTY not supported (for old Windows versions), fallback on ShellCommand.
            if codex_utils_pty::conpty_supported() {
                ConfigShellToolType::UnifiedExec
            } else {
                ConfigShellToolType::ShellCommand
            }
        } else if model_info.shell_type == ConfigShellToolType::UnifiedExec && !unified_exec_allowed
        {
            ConfigShellToolType::ShellCommand
        } else {
            model_info.shell_type
        };

        let apply_patch_tool_type = match model_info.apply_patch_tool_type {
            Some(ApplyPatchToolType::Freeform) => Some(ApplyPatchToolType::Freeform),
            Some(ApplyPatchToolType::Function) => Some(ApplyPatchToolType::Function),
            None => {
                if include_apply_patch_tool {
                    Some(ApplyPatchToolType::Freeform)
                } else {
                    None
                }
            }
        };

        let agent_jobs_worker_tools = include_agent_jobs
            && matches!(
                session_source,
                SessionSource::SubAgent(SubAgentSource::Other(label))
                    if label.starts_with("agent_job:")
            );

        Self {
            available_models: available_models_ref.to_vec(),
            shell_type,
            shell_command_backend,
            unified_exec_shell_mode: UnifiedExecShellMode::Direct,
            allow_login_shell: true,
            apply_patch_tool_type,
            web_search_mode: *web_search_mode,
            web_search_config: None,
            web_search_tool_type: model_info.web_search_tool_type,
            image_gen_tool: include_image_gen_tool,
            agent_roles: BTreeMap::new(),
            search_tool: include_search_tool,
            tool_suggest: include_tool_suggest,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
            code_mode_enabled: include_code_mode,
            code_mode_only_enabled: include_code_mode_only,
            js_repl_enabled: include_js_repl,
            js_repl_tools_only: include_js_repl_tools_only,
            can_request_original_image_detail: include_original_image_detail,
            collab_tools: include_collab_tools,
            multi_agent_v2: include_multi_agent_v2,
            agent_watchdog: include_agent_watchdog,
            request_user_input: include_request_user_input,
            default_mode_request_user_input: include_default_mode_request_user_input,
            experimental_supported_tools: model_info.experimental_supported_tools.clone(),
            agent_jobs_tools: include_agent_jobs,
            agent_jobs_worker_tools,
        }
    }

    pub fn with_agent_roles(mut self, agent_roles: BTreeMap<String, AgentRoleConfig>) -> Self {
        self.agent_roles = agent_roles;
        self
    }

    pub fn with_allow_login_shell(mut self, allow_login_shell: bool) -> Self {
        self.allow_login_shell = allow_login_shell;
        self
    }

    pub fn with_unified_exec_shell_mode(
        mut self,
        unified_exec_shell_mode: UnifiedExecShellMode,
    ) -> Self {
        self.unified_exec_shell_mode = unified_exec_shell_mode;
        self
    }

    pub fn with_unified_exec_shell_mode_for_session(
        mut self,
        user_shell: &Shell,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
    ) -> Self {
        self.unified_exec_shell_mode = UnifiedExecShellMode::for_session(
            self.shell_command_backend,
            user_shell,
            shell_zsh_path,
            main_execve_wrapper_exe,
        );
        self
    }

    pub fn with_web_search_config(mut self, web_search_config: Option<WebSearchConfig>) -> Self {
        self.web_search_config = web_search_config;
        self
    }

    pub fn for_code_mode_nested_tools(&self) -> Self {
        let mut nested = self.clone();
        nested.code_mode_enabled = false;
        nested.code_mode_only_enabled = false;
        nested
    }
}

fn supports_image_generation(model_info: &ModelInfo) -> bool {
    model_info.input_modalities.contains(&InputModality::Image)
}

fn create_wait_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "cell_id".to_string(),
            JsonSchema::String {
                description: Some("Identifier of the running exec cell.".to_string()),
            },
        ),
        (
            "yield_time_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "How long to wait (in milliseconds) for more output before yielding again."
                        .to_string(),
                ),
            },
        ),
        (
            "max_tokens".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Maximum number of output tokens to return for this wait call.".to_string(),
                ),
            },
        ),
        (
            "terminate".to_string(),
            JsonSchema::Boolean {
                description: Some("Whether to terminate the running exec cell.".to_string()),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: WAIT_TOOL_NAME.to_string(),
        description: format!(
            "Waits on a yielded `{PUBLIC_TOOL_NAME}` cell and returns new output or completion.\n{}",
            codex_code_mode::build_wait_tool_description().trim()
        ),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["cell_id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
        defer_loading: None,
    })
}

fn create_test_sync_tool() -> ToolSpec {
    let barrier_properties = BTreeMap::from([
        (
            "id".to_string(),
            JsonSchema::String {
                description: Some(
                    "Identifier shared by concurrent calls that should rendezvous".to_string(),
                ),
            },
        ),
        (
            "participants".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Number of tool calls that must arrive before the barrier opens".to_string(),
                ),
            },
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Maximum time in milliseconds to wait at the barrier".to_string(),
                ),
            },
        ),
    ]);

    let properties = BTreeMap::from([
        (
            "sleep_before_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Optional delay in milliseconds before any other action".to_string(),
                ),
            },
        ),
        (
            "sleep_after_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Optional delay in milliseconds after completing the barrier".to_string(),
                ),
            },
        ),
        (
            "barrier".to_string(),
            JsonSchema::Object {
                properties: barrier_properties,
                required: Some(vec!["id".to_string(), "participants".to_string()]),
                additional_properties: Some(false.into()),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "test_sync_tool".to_string(),
        description: "Internal synchronization helper used by Codex integration tests.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_tool_search_tool(app_tools: &HashMap<String, ToolInfo>) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "query".to_string(),
            JsonSchema::String {
                description: Some("Search query for apps tools.".to_string()),
            },
        ),
        (
            "limit".to_string(),
            JsonSchema::Number {
                description: Some(format!(
                    "Maximum number of tools to return (defaults to {TOOL_SEARCH_DEFAULT_LIMIT})."
                )),
            },
        ),
    ]);
    let mut app_descriptions = BTreeMap::from([(
        AGENT_TOOLS_NAMESPACE.to_string(),
        Some(AGENT_TOOLS_NAMESPACE_DESCRIPTION.to_string()),
    )]);
    for tool in app_tools.values() {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            continue;
        }

        let Some(connector_name) = tool
            .connector_name
            .as_deref()
            .map(str::trim)
            .filter(|connector_name| !connector_name.is_empty())
        else {
            continue;
        };

        let connector_description = tool
            .connector_description
            .as_deref()
            .map(str::trim)
            .filter(|connector_description| !connector_description.is_empty())
            .map(str::to_string);

        app_descriptions
            .entry(connector_name.to_string())
            .and_modify(|existing: &mut Option<String>| {
                if existing.is_none() {
                    *existing = connector_description.clone();
                }
            })
            .or_insert(connector_description);
    }

    let app_descriptions = if app_descriptions.is_empty() {
        "None currently enabled.".to_string()
    } else {
        app_descriptions
            .into_iter()
            .map(
                |(connector_name, connector_description)| match connector_description {
                    Some(connector_description) => {
                        format!("- {connector_name}: {connector_description}")
                    }
                    None => format!("- {connector_name}"),
                },
            )
            .collect::<Vec<_>>()
            .join("\n")
    };

    let description = TOOL_SEARCH_DESCRIPTION_TEMPLATE
        .render([(
            TOOL_SEARCH_DESCRIPTION_TEMPLATE_KEY,
            app_descriptions.as_str(),
        )])
        .unwrap_or_else(|err| panic!("tool_search description template must render: {err}"));

    ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["query".to_string()]),
            additional_properties: Some(false.into()),
        },
    }
}

fn create_tool_suggest_tool(discoverable_tools: &[DiscoverableTool]) -> ToolSpec {
    let discoverable_tool_ids = discoverable_tools
        .iter()
        .map(DiscoverableTool::id)
        .collect::<Vec<_>>()
        .join(", ");
    let properties = BTreeMap::from([
        (
            "tool_type".to_string(),
            JsonSchema::String {
                description: Some(
                    "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                        .to_string(),
                ),
            },
        ),
        (
            "action_type".to_string(),
            JsonSchema::String {
                description: Some(
                    "Suggested action for the tool. Use \"install\" or \"enable\".".to_string(),
                ),
            },
        ),
        (
            "tool_id".to_string(),
            JsonSchema::String {
                description: Some(format!(
                    "Connector or plugin id to suggest. Must be one of: {discoverable_tool_ids}."
                )),
            },
        ),
        (
            "suggest_reason".to_string(),
            JsonSchema::String {
                description: Some(
                    "Concise one-line user-facing reason why this tool can help with the current request."
                        .to_string(),
                ),
            },
        ),
    ]);
    let discoverable_tools = format_discoverable_tools(discoverable_tools);
    let description = TOOL_SUGGEST_DESCRIPTION_TEMPLATE
        .render([(
            TOOL_SUGGEST_DESCRIPTION_TEMPLATE_KEY,
            discoverable_tools.as_str(),
        )])
        .unwrap_or_else(|err| panic!("tool_suggest description template must render: {err}"));

    ToolSpec::Function(ResponsesApiTool {
        name: TOOL_SUGGEST_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn format_discoverable_tools(discoverable_tools: &[DiscoverableTool]) -> String {
    let mut discoverable_tools = discoverable_tools.to_vec();
    discoverable_tools.sort_by(|left, right| {
        left.name()
            .cmp(right.name())
            .then_with(|| left.id().cmp(right.id()))
    });

    discoverable_tools
        .into_iter()
        .map(|tool| {
            let description = tool
                .description()
                .filter(|description| !description.trim().is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| match &tool {
                    DiscoverableTool::Connector(_) => "No description provided.".to_string(),
                    DiscoverableTool::Plugin(plugin) => format_plugin_summary(plugin.as_ref()),
                });
            let default_action = match tool.tool_type() {
                DiscoverableToolType::Connector => DiscoverableToolAction::Install,
                DiscoverableToolType::Plugin => DiscoverableToolAction::Install,
            };
            format!(
                "- {} (id: `{}`, type: {}, action: {}): {}",
                tool.name(),
                tool.id(),
                tool.tool_type().as_str(),
                default_action.as_str(),
                description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_plugin_summary(plugin: &DiscoverablePluginInfo) -> String {
    let mut details = Vec::new();
    if plugin.has_skills {
        details.push("skills".to_string());
    }
    if !plugin.mcp_server_names.is_empty() {
        details.push(format!(
            "MCP servers: {}",
            plugin.mcp_server_names.join(", ")
        ));
    }
    if !plugin.app_connector_ids.is_empty() {
        details.push(format!(
            "app connectors: {}",
            plugin.app_connector_ids.join(", ")
        ));
    }

    if details.is_empty() {
        "No description provided.".to_string()
    } else {
        details.join("; ")
    }
}

fn create_list_dir_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "dir_path".to_string(),
            JsonSchema::String {
                description: Some("Absolute path to the directory to list.".to_string()),
            },
        ),
        (
            "offset".to_string(),
            JsonSchema::Number {
                description: Some(
                    "The entry number to start listing from. Must be 1 or greater.".to_string(),
                ),
            },
        ),
        (
            "limit".to_string(),
            JsonSchema::Number {
                description: Some("The maximum number of entries to return.".to_string()),
            },
        ),
        (
            "depth".to_string(),
            JsonSchema::Number {
                description: Some(
                    "The maximum directory depth to traverse. Must be 1 or greater.".to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_dir".to_string(),
        description:
            "Lists entries in a local directory with 1-indexed entry numbers and simple type labels."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["dir_path".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_js_repl_tool() -> ToolSpec {
    // Keep JS input freeform, but block the most common malformed payload shapes
    // (JSON wrappers, quoted strings, and markdown fences) before they reach the
    // runtime `reject_json_or_quoted_source` validation. The API's regex engine
    // does not support look-around, so this uses a "first significant token"
    // pattern rather than negative lookaheads.
    const JS_REPL_FREEFORM_GRAMMAR: &str = r#"
start: pragma_source | plain_source

pragma_source: PRAGMA_LINE NEWLINE js_source
plain_source: PLAIN_JS_SOURCE

js_source: JS_SOURCE

PRAGMA_LINE: /[ \t]*\/\/ codex-js-repl:[^\r\n]*/
NEWLINE: /\r?\n/
PLAIN_JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: "js_repl".to_string(),
        description: "Runs JavaScript in a persistent Node kernel with top-level await. This is a freeform tool: send raw JavaScript source text, optionally with a first-line pragma like `// codex-js-repl: timeout_ms=15000`; do not send JSON/quotes/markdown fences."
            .to_string(),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: JS_REPL_FREEFORM_GRAMMAR.to_string(),
        },
    })
}

fn create_js_repl_reset_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "js_repl_reset".to_string(),
        description:
            "Restarts the js_repl kernel for this run and clears persisted top-level bindings."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_code_mode_tool(
    enabled_tools: &[(String, String)],
    code_mode_only_enabled: bool,
) -> ToolSpec {
    const CODE_MODE_FREEFORM_GRAMMAR: &str = r#"
start: pragma_source | plain_source
pragma_source: PRAGMA_LINE NEWLINE SOURCE
plain_source: SOURCE

PRAGMA_LINE: /[ \t]*\/\/ @exec:[^\r\n]*/
NEWLINE: /\r?\n/
SOURCE: /[\s\S]+/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: PUBLIC_TOOL_NAME.to_string(),
        description: codex_code_mode::build_exec_tool_description(
            enabled_tools,
            code_mode_only_enabled,
        ),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: CODE_MODE_FREEFORM_GRAMMAR.to_string(),
        },
    })
}

fn create_list_mcp_resources_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional MCP server name. When omitted, lists resources from every configured server."
                        .to_string(),
                ),
            },
        ),
        (
            "cursor".to_string(),
            JsonSchema::String {
                description: Some(
                    "Opaque cursor returned by a previous list_mcp_resources call for the same server."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_mcp_resources".to_string(),
        description: "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Prefer resources over web search when possible.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_list_mcp_resource_templates_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional MCP server name. When omitted, lists resource templates from all configured servers."
                        .to_string(),
                ),
            },
        ),
        (
            "cursor".to_string(),
            JsonSchema::String {
                description: Some(
                    "Opaque cursor returned by a previous list_mcp_resource_templates call for the same server."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_mcp_resource_templates".to_string(),
        description: "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Prefer resource templates over web search when possible.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_read_mcp_resource_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "server".to_string(),
            JsonSchema::String {
                description: Some(
                    "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources."
                        .to_string(),
                ),
            },
        ),
        (
            "uri".to_string(),
            JsonSchema::String {
                description: Some(
                    "Resource URI to read. Must be one of the URIs returned by list_mcp_resources."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "read_mcp_resource".to_string(),
        description:
            "Read a specific resource from an MCP server given the server name and resource URI."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["server".to_string(), "uri".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

/// TODO(dylan): deprecate once we get rid of json tool
#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyPatchToolArgs {
    pub(crate) input: String,
}

fn push_tool_spec(
    builder: &mut ToolRegistryBuilder,
    spec: ToolSpec,
    supports_parallel_tool_calls: bool,
    code_mode_enabled: bool,
) {
    let spec = if code_mode_enabled {
        augment_tool_spec_for_code_mode(spec)
    } else {
        spec
    };
    if supports_parallel_tool_calls {
        builder.push_spec_with_parallel_support(spec, /*supports_parallel_tool_calls*/ true);
    } else {
        builder.push_spec(spec);
    }
}

fn create_agent_tools_namespace(tools: Vec<ToolSpec>) -> ToolSpec {
    let tools = tools
        .into_iter()
        .filter_map(|tool| match tool {
            ToolSpec::Function(tool) => Some(ResponsesApiNamespaceTool::Function(tool)),
            _ => None,
        })
        .collect();

    ToolSpec::Namespace(ResponsesApiNamespace {
        name: AGENT_TOOLS_NAMESPACE.to_string(),
        description: AGENT_TOOLS_NAMESPACE_DESCRIPTION.to_string(),
        tools,
    })
}

fn create_compact_parent_context_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "reason".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional short reason describing why the parent appears stuck.".to_string(),
                ),
            },
        ),
        (
            "evidence".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional concrete evidence of non-progress, such as repeated identical replies with no tool or file actions.".to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "compact_parent_context".to_string(),
        description: "Watchdog-only: request compaction for the watchdog helper's parent thread when it is idle and appears stuck."
            .to_string(),
        strict: false,
        defer_loading: Some(true),
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_list_agents_tool(agent_watchdog: bool) -> ToolSpec {
    let description = if agent_watchdog {
        "List agents spawned by an agent, optionally recursively. This is a status view; polling it will not make a watchdog fire."
    } else {
        "List agents spawned by an agent, optionally recursively."
    };
    let properties = BTreeMap::from([
        (
            "id".to_string(),
            JsonSchema::String {
                description: Some(
                    "Identifier of the parent agent whose spawned agents to list. Defaults to the current agent."
                        .to_string(),
                ),
            },
        ),
        (
            "recursive".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true (default), include all descendants recursively. When false, include only direct children."
                        .to_string(),
                ),
            },
        ),
        (
            "all".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, include completed/failed/canceled agents in addition to live agents."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_agents".to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn register_agent_tool_handler<H>(builder: &mut ToolRegistryBuilder, name: &str, handler: Arc<H>)
where
    H: crate::tools::registry::ToolHandler + 'static,
{
    builder.register_handler(name, handler.clone());
    builder.register_handler(tool_handler_key(name, Some(AGENT_TOOLS_NAMESPACE)), handler);
}

/// Builds the tool registry builder while collecting tool specs for later serialization.
#[cfg(test)]
pub(crate) fn build_specs(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    build_specs_with_discoverable_tools(
        config,
        mcp_tools,
        app_tools,
        /*discoverable_tools*/ None,
        dynamic_tools,
    )
}

pub(crate) fn build_specs_with_discoverable_tools(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::CodeModeExecuteHandler;
    use crate::tools::handlers::CodeModeWaitHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::JsReplHandler;
    use crate::tools::handlers::JsReplResetHandler;
    use crate::tools::handlers::ListDirHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::McpResourceHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::RequestPermissionsHandler;
    use crate::tools::handlers::RequestUserInputHandler;
    use crate::tools::handlers::ShellCommandHandler;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::handlers::TestSyncHandler;
    use crate::tools::handlers::ToolSearchHandler;
    use crate::tools::handlers::ToolSuggestHandler;
    use crate::tools::handlers::UnifiedExecHandler;
    use crate::tools::handlers::ViewImageHandler;
    use crate::tools::handlers::multi_agents::CloseAgentHandler;
    use crate::tools::handlers::multi_agents::CompactParentContextHandler;
    use crate::tools::handlers::multi_agents::ListAgentsHandler;
    use crate::tools::handlers::multi_agents::ResumeAgentHandler;
    use crate::tools::handlers::multi_agents::SendInputHandler;
    use crate::tools::handlers::multi_agents::SpawnAgentHandler;
    use crate::tools::handlers::multi_agents::WaitAgentHandler;
    use crate::tools::handlers::multi_agents_v2::AssignTaskHandler as AssignTaskHandlerV2;
    use crate::tools::handlers::multi_agents_v2::CloseAgentHandler as CloseAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
    let mut builder = ToolRegistryBuilder::new();

    let shell_handler = Arc::new(ShellHandler);
    let unified_exec_handler = Arc::new(UnifiedExecHandler);
    let plan_handler = Arc::new(PlanHandler);
    let apply_patch_handler = Arc::new(ApplyPatchHandler);
    let dynamic_tool_handler = Arc::new(DynamicToolHandler);
    let view_image_handler = Arc::new(ViewImageHandler);
    let mcp_handler = Arc::new(McpHandler);
    let mcp_resource_handler = Arc::new(McpResourceHandler);
    let shell_command_handler = Arc::new(ShellCommandHandler::from(config.shell_command_backend));
    let request_permissions_handler = Arc::new(RequestPermissionsHandler);
    let request_user_input_handler = Arc::new(RequestUserInputHandler {
        default_mode_request_user_input: config.default_mode_request_user_input,
    });
    let tool_suggest_handler = Arc::new(ToolSuggestHandler);
    let code_mode_handler = Arc::new(CodeModeExecuteHandler);
    let code_mode_wait_handler = Arc::new(CodeModeWaitHandler);
    let js_repl_handler = Arc::new(JsReplHandler);
    let js_repl_reset_handler = Arc::new(JsReplResetHandler);
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;

    if config.code_mode_enabled {
        let nested_config = config.for_code_mode_nested_tools();
        let (nested_specs, _) = build_specs_with_discoverable_tools(
            &nested_config,
            mcp_tools.clone(),
            app_tools.clone(),
            /*discoverable_tools*/ None,
            dynamic_tools,
        )
        .build();
        let mut enabled_tools = nested_specs
            .into_iter()
            .filter_map(|spec| tool_spec_to_code_mode_tool_definition(&spec.spec))
            .map(|tool| (tool.name, tool.description))
            .collect::<Vec<_>>();
        enabled_tools.sort_by(|left, right| left.0.cmp(&right.0));
        enabled_tools.dedup_by(|left, right| left.0 == right.0);
        push_tool_spec(
            &mut builder,
            create_code_mode_tool(&enabled_tools, config.code_mode_only_enabled),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler(PUBLIC_TOOL_NAME, code_mode_handler);
        push_tool_spec(
            &mut builder,
            create_wait_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler(WAIT_TOOL_NAME, code_mode_wait_handler);
    }

    match &config.shell_type {
        ConfigShellToolType::Default => {
            push_tool_spec(
                &mut builder,
                create_shell_tool(ShellToolOptions {
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::Local => {
            push_tool_spec(
                &mut builder,
                ToolSpec::LocalShell {},
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::UnifiedExec => {
            push_tool_spec(
                &mut builder,
                create_exec_command_tool(CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_write_stdin_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("exec_command", unified_exec_handler.clone());
            builder.register_handler("write_stdin", unified_exec_handler);
        }
        ConfigShellToolType::Disabled => {
            // Do nothing.
        }
        ConfigShellToolType::ShellCommand => {
            push_tool_spec(
                &mut builder,
                create_shell_command_tool(CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
    }

    if config.shell_type != ConfigShellToolType::Disabled {
        // Always register shell aliases so older prompts remain compatible.
        builder.register_handler("shell", shell_handler.clone());
        builder.register_handler("container.exec", shell_handler.clone());
        builder.register_handler("local_shell", shell_handler);
        builder.register_handler("shell_command", shell_command_handler);
    }

    if mcp_tools.is_some() {
        push_tool_spec(
            &mut builder,
            create_list_mcp_resources_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_list_mcp_resource_templates_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_read_mcp_resource_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("list_mcp_resources", mcp_resource_handler.clone());
        builder.register_handler("list_mcp_resource_templates", mcp_resource_handler.clone());
        builder.register_handler("read_mcp_resource", mcp_resource_handler);
    }

    push_tool_spec(
        &mut builder,
        PLAN_TOOL.clone(),
        /*supports_parallel_tool_calls*/ false,
        config.code_mode_enabled,
    );
    builder.register_handler("update_plan", plan_handler);

    if config.js_repl_enabled {
        push_tool_spec(
            &mut builder,
            create_js_repl_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_js_repl_reset_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("js_repl", js_repl_handler);
        builder.register_handler("js_repl_reset", js_repl_reset_handler);
    }

    if config.request_user_input {
        push_tool_spec(
            &mut builder,
            create_request_user_input_tool(request_user_input_tool_description(
                config.default_mode_request_user_input,
            )),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_user_input", request_user_input_handler);
    }

    if config.request_permissions_tool_enabled {
        push_tool_spec(
            &mut builder,
            create_request_permissions_tool(request_permissions_tool_description()),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_permissions", request_permissions_handler);
    }

    if config.search_tool && (app_tools.is_some() || config.collab_tools) {
        let app_tools = app_tools.unwrap_or_default();
        let search_tool_handler = Arc::new(ToolSearchHandler::new(app_tools.clone()));
        push_tool_spec(
            &mut builder,
            create_tool_search_tool(&app_tools),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler(TOOL_SEARCH_TOOL_NAME, search_tool_handler);

        for tool in app_tools.values() {
            let alias_name =
                tool_handler_key(tool.tool_name.as_str(), Some(tool.tool_namespace.as_str()));

            builder.register_handler(alias_name, mcp_handler.clone());
        }
    }

    if config.tool_suggest
        && let Some(discoverable_tools) = discoverable_tools
            .as_ref()
            .filter(|tools| !tools.is_empty())
    {
        builder.push_spec_with_parallel_support(
            create_tool_suggest_tool(discoverable_tools),
            /*supports_parallel_tool_calls*/ true,
        );
        builder.register_handler(TOOL_SUGGEST_TOOL_NAME, tool_suggest_handler);
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_freeform_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
            ApplyPatchToolType::Function => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_json_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
        }
        builder.register_handler("apply_patch", apply_patch_handler);
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "list_dir")
    {
        let list_dir_handler = Arc::new(ListDirHandler);
        push_tool_spec(
            &mut builder,
            create_list_dir_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("list_dir", list_dir_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"test_sync_tool".to_string())
    {
        let test_sync_handler = Arc::new(TestSyncHandler);
        push_tool_spec(
            &mut builder,
            create_test_sync_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("test_sync_tool", test_sync_handler);
    }

    let external_web_access = match config.web_search_mode {
        Some(WebSearchMode::Cached) => Some(false),
        Some(WebSearchMode::Live) => Some(true),
        Some(WebSearchMode::Disabled) | None => None,
    };

    if let Some(external_web_access) = external_web_access {
        let search_content_types = match config.web_search_tool_type {
            WebSearchToolType::Text => None,
            WebSearchToolType::TextAndImage => Some(
                WEB_SEARCH_CONTENT_TYPES
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            ),
        };

        push_tool_spec(
            &mut builder,
            ToolSpec::WebSearch {
                external_web_access: Some(external_web_access),
                filters: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.filters.clone().map(Into::into)),
                user_location: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.user_location.clone().map(Into::into)),
                search_context_size: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.search_context_size),
                search_content_types,
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    if config.image_gen_tool {
        push_tool_spec(
            &mut builder,
            ToolSpec::ImageGeneration {
                output_format: "png".to_string(),
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    push_tool_spec(
        &mut builder,
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
        }),
        /*supports_parallel_tool_calls*/ true,
        config.code_mode_enabled,
    );
    builder.register_handler("view_image", view_image_handler);

    if config.collab_tools {
        if config.multi_agent_v2 {
            let mut agent_tools = vec![
                create_spawn_agent_tool_v2(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description: crate::agent::role::spawn_tool_spec::build(
                        &config.agent_roles,
                    ),
                }),
                create_send_message_tool(),
                create_assign_task_tool(),
                create_wait_agent_tool_v2(WaitAgentTimeoutOptions {
                    default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
                    min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
                    max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
                }),
                create_close_agent_tool_v2(),
                create_list_agents_tool_v2(),
            ];
            if config.agent_watchdog {
                agent_tools.push(create_compact_parent_context_tool());
            }
            push_tool_spec(
                &mut builder,
                create_agent_tools_namespace(agent_tools),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            register_agent_tool_handler(&mut builder, "spawn_agent", Arc::new(SpawnAgentHandlerV2));
            register_agent_tool_handler(
                &mut builder,
                "send_message",
                Arc::new(SendMessageHandlerV2),
            );
            register_agent_tool_handler(&mut builder, "assign_task", Arc::new(AssignTaskHandlerV2));
            register_agent_tool_handler(&mut builder, "wait_agent", Arc::new(WaitAgentHandlerV2));
            register_agent_tool_handler(&mut builder, "close_agent", Arc::new(CloseAgentHandlerV2));
            register_agent_tool_handler(&mut builder, "list_agents", Arc::new(ListAgentsHandlerV2));
        } else {
            let mut agent_tools = vec![
                create_spawn_agent_tool_v1(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description: crate::agent::role::spawn_tool_spec::build(
                        &config.agent_roles,
                    ),
                }),
                create_send_input_tool_v1(),
                create_resume_agent_tool(),
                create_wait_agent_tool_v1(WaitAgentTimeoutOptions {
                    default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
                    min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
                    max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
                }),
                create_close_agent_tool_v1(),
            ];
            if config.agent_watchdog {
                agent_tools.push(create_list_agents_tool(config.agent_watchdog));
                agent_tools.push(create_compact_parent_context_tool());
            }
            push_tool_spec(
                &mut builder,
                create_agent_tools_namespace(agent_tools),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            register_agent_tool_handler(&mut builder, "spawn_agent", Arc::new(SpawnAgentHandler));
            register_agent_tool_handler(&mut builder, "send_input", Arc::new(SendInputHandler));
            register_agent_tool_handler(&mut builder, "resume_agent", Arc::new(ResumeAgentHandler));
            register_agent_tool_handler(&mut builder, "wait_agent", Arc::new(WaitAgentHandler));
            register_agent_tool_handler(&mut builder, "close_agent", Arc::new(CloseAgentHandler));
            register_agent_tool_handler(&mut builder, "list_agents", Arc::new(ListAgentsHandler));
            register_agent_tool_handler(
                &mut builder,
                "compact_parent_context",
                Arc::new(CompactParentContextHandler),
            );
        }
    }

    if config.agent_jobs_tools {
        let agent_jobs_handler = Arc::new(BatchJobHandler);
        push_tool_spec(
            &mut builder,
            create_spawn_agents_on_csv_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("spawn_agents_on_csv", agent_jobs_handler.clone());
        if config.agent_jobs_worker_tools {
            push_tool_spec(
                &mut builder,
                create_report_agent_job_result_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("report_agent_job_result", agent_jobs_handler);
        }
    }

    if let Some(mcp_tools) = mcp_tools {
        let mut entries: Vec<(String, rmcp::model::Tool)> = mcp_tools.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, tool) in entries.into_iter() {
            match mcp_tool_to_responses_api_tool(name.clone(), &tool) {
                Ok(converted_tool) => {
                    push_tool_spec(
                        &mut builder,
                        ToolSpec::Function(converted_tool),
                        /*supports_parallel_tool_calls*/ false,
                        config.code_mode_enabled,
                    );
                    builder.register_handler(name, mcp_handler.clone());
                }
                Err(e) => {
                    tracing::error!("Failed to convert {name:?} MCP tool to OpenAI tool: {e:?}");
                }
            }
        }
    }

    if !dynamic_tools.is_empty() {
        for tool in dynamic_tools {
            match dynamic_tool_to_responses_api_tool(tool) {
                Ok(converted_tool) => {
                    push_tool_spec(
                        &mut builder,
                        ToolSpec::Function(converted_tool),
                        /*supports_parallel_tool_calls*/ false,
                        config.code_mode_enabled,
                    );
                    builder.register_handler(tool.name.clone(), dynamic_tool_handler.clone());
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to convert dynamic tool {:?} to OpenAI tool: {e:?}",
                        tool.name
                    );
                }
            }
        }
    }

    builder
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
