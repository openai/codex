use crate::client_common::tools::FreeformTool;
use crate::client_common::tools::FreeformToolFormat;
use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::config::AgentRoleConfig;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use crate::original_image_detail::can_request_original_image_detail;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::code_mode_description::augment_tool_spec_for_code_mode;
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
use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

const TOOL_SEARCH_DESCRIPTION_TEMPLATE: &str =
    include_str!("../../templates/search_tool/tool_description.md");
const TOOL_SUGGEST_DESCRIPTION_TEMPLATE: &str =
    include_str!("../../templates/search_tool/tool_suggest_description.md");
const WEB_SEARCH_CONTENT_TYPES: [&str; 2] = ["text", "image"];

fn unified_exec_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "chunk_id": {
                "type": "string",
                "description": "Chunk identifier included when the response reports one."
            },
            "wall_time_seconds": {
                "type": "number",
                "description": "Elapsed wall time spent waiting for output in seconds."
            },
            "exit_code": {
                "type": "number",
                "description": "Process exit code when the command finished during this call."
            },
            "session_id": {
                "type": "number",
                "description": "Session identifier to pass to write_stdin when the process is still running."
            },
            "original_token_count": {
                "type": "number",
                "description": "Approximate token count before output truncation."
            },
            "output": {
                "type": "string",
                "description": "Command output text, possibly truncated."
            }
        },
        "required": ["wall_time_seconds", "output"],
        "additionalProperties": false
    })
}

fn agent_status_output_schema() -> JsonValue {
    json!({
        "oneOf": [
            {
                "type": "string",
                "enum": ["pending_init", "running", "shutdown", "not_found"]
            },
            {
                "type": "object",
                "properties": {
                    "completed": {
                        "type": ["string", "null"]
                    }
                },
                "required": ["completed"],
                "additionalProperties": false
            },
            {
                "type": "object",
                "properties": {
                    "errored": {
                        "type": "string"
                    }
                },
                "required": ["errored"],
                "additionalProperties": false
            }
        ]
    })
}

fn spawn_agent_output_schema(multi_agent_v2: bool) -> JsonValue {
    let task_name_description = if multi_agent_v2 {
        "Canonical task name for the spawned agent."
    } else {
        "Canonical task name for the spawned agent when one was assigned."
    };
    json!({
        "type": "object",
        "properties": {
            "agent_id": {
                "type": ["string", "null"],
                "description": "Thread identifier for the spawned agent when no task name was assigned."
            },
            "task_name": {
                "type": ["string", "null"],
                "description": task_name_description
            },
            "nickname": {
                "type": ["string", "null"],
                "description": "User-facing nickname for the spawned agent when available."
            }
        },
        "required": ["agent_id", "task_name", "nickname"],
        "additionalProperties": false
    })
}

fn send_input_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "submission_id": {
                "type": "string",
                "description": "Identifier for the queued input submission."
            }
        },
        "required": ["submission_id"],
        "additionalProperties": false
    })
}

fn list_agents_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "agents": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "agent_name": {
                            "type": "string",
                            "description": "Canonical task name for the agent when available, otherwise the agent id."
                        },
                        "agent_status": {
                            "description": "Last known status of the agent.",
                            "allOf": [agent_status_output_schema()]
                        },
                        "last_task_message": {
                            "type": ["string", "null"],
                            "description": "Most recent user or inter-agent instruction received by the agent, when available."
                        }
                    },
                    "required": ["agent_name", "agent_status", "last_task_message"],
                    "additionalProperties": false
                },
                "description": "Live agents visible in the current root thread tree."
            }
        },
        "required": ["agents"],
        "additionalProperties": false
    })
}

fn resume_agent_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "status": agent_status_output_schema()
        },
        "required": ["status"],
        "additionalProperties": false
    })
}

fn wait_output_schema_v1() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "object",
                "description": "Final statuses keyed by canonical task name when available, otherwise by agent id.",
                "additionalProperties": agent_status_output_schema()
            },
            "timed_out": {
                "type": "boolean",
                "description": "Whether the wait call returned due to timeout before any agent reached a final status."
            }
        },
        "required": ["status", "timed_out"],
        "additionalProperties": false
    })
}

fn wait_output_schema_v2() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "message": {
                "type": "string",
                "description": "Brief wait summary without the agent's final content."
            },
            "timed_out": {
                "type": "boolean",
                "description": "Whether the wait call returned due to timeout before any agent reached a final status."
            }
        },
        "required": ["message", "timed_out"],
        "additionalProperties": false
    })
}

fn close_agent_output_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "previous_status": {
                "description": "The agent status observed before shutdown was requested.",
                "allOf": [agent_status_output_schema()]
            }
        },
        "required": ["previous_status"],
        "additionalProperties": false
    })
}

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
    pub artifact_tools: bool,
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
        let include_agent_jobs = features.enabled(Feature::SpawnCsv);
        let include_request_user_input = !matches!(session_source, SessionSource::SubAgent(_));
        let include_default_mode_request_user_input =
            include_request_user_input && features.enabled(Feature::DefaultModeRequestUserInput);
        let include_search_tool =
            model_info.supports_search_tool && features.enabled(Feature::ToolSearch);
        let include_tool_suggest = include_search_tool
            && features.enabled(Feature::ToolSuggest)
            && features.enabled(Feature::Apps)
            && features.enabled(Feature::Plugins);
        let include_original_image_detail = can_request_original_image_detail(features, model_info);
        let include_artifact_tools =
            features.enabled(Feature::Artifact) && codex_artifacts::can_manage_artifact_runtime();
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
            artifact_tools: include_artifact_tools,
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

/// Generic JSON‑Schema subset needed for our tool definitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonSchema {
    Boolean {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    String {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// MCP schema allows "number" | "integer" for Number
    #[serde(alias = "integer")]
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Array {
        items: Box<JsonSchema>,

        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Object {
        properties: BTreeMap<String, JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<Vec<String>>,
        #[serde(
            rename = "additionalProperties",
            skip_serializing_if = "Option::is_none"
        )]
        additional_properties: Option<AdditionalProperties>,
    },
}

/// Whether additional properties are allowed, and if so, any required schema
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Boolean(bool),
    Schema(Box<JsonSchema>),
}

impl From<bool> for AdditionalProperties {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<JsonSchema> for AdditionalProperties {
    fn from(s: JsonSchema) -> Self {
        Self::Schema(Box::new(s))
    }
}

fn create_network_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([(
            "enabled".to_string(),
            JsonSchema::Boolean {
                description: Some("Set to true to request network access.".to_string()),
            },
        )]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn create_file_system_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([
            (
                "read".to_string(),
                JsonSchema::Array {
                    items: Box::new(JsonSchema::String { description: None }),
                    description: Some("Absolute paths to grant read access to.".to_string()),
                },
            ),
            (
                "write".to_string(),
                JsonSchema::Array {
                    items: Box::new(JsonSchema::String { description: None }),
                    description: Some("Absolute paths to grant write access to.".to_string()),
                },
            ),
        ]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn create_additional_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([
            ("network".to_string(), create_network_permissions_schema()),
            (
                "file_system".to_string(),
                create_file_system_permissions_schema(),
            ),
        ]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn create_request_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([
            ("network".to_string(), create_network_permissions_schema()),
            (
                "file_system".to_string(),
                create_file_system_permissions_schema(),
            ),
        ]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn windows_destructive_filesystem_guidance() -> &'static str {
    r#"Windows safety rules:
- Do not compose destructive filesystem commands across shells. Do not enumerate paths in PowerShell and then pass them to `cmd /c`, batch builtins, or another shell for deletion or moving. Use one shell end-to-end, prefer native PowerShell cmdlets such as `Remove-Item` / `Move-Item` with `-LiteralPath`, and avoid string-built shell commands for file operations.
- Before any recursive delete or move on Windows, verify the resolved absolute target paths stay within the intended workspace or explicitly named target directory. Never issue a recursive delete or move against a computed path if the final target has not been checked."#
}

fn create_approval_parameters(
    exec_permission_approvals_enabled: bool,
) -> BTreeMap<String, JsonSchema> {
    let mut properties = BTreeMap::from([
        (
            "sandbox_permissions".to_string(),
            JsonSchema::String {
                description: Some(
                    if exec_permission_approvals_enabled {
                        "Sandbox permissions for the command. Use \"with_additional_permissions\" to request additional sandboxed filesystem or network permissions (preferred), or \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
                    } else {
                        "Sandbox permissions for the command. Set to \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
                    }
                    .to_string(),
                ),
            },
        ),
        (
            "justification".to_string(),
            JsonSchema::String {
                description: Some(
                    r#"Only set if sandbox_permissions is \"require_escalated\".
                    Request approval from the user to run this command outside the sandbox.
                    Phrased as a simple question that summarizes the purpose of the
                    command as it relates to the task at hand - e.g. 'Do you want to
                    fetch and pull the latest version of this git branch?'"#
                    .to_string(),
                ),
            },
        ),
        (
            "prefix_rule".to_string(),
            JsonSchema::Array {
                items: Box::new(JsonSchema::String { description: None }),
                description: Some(
                    r#"Only specify when sandbox_permissions is `require_escalated`.
                        Suggest a prefix command pattern that will allow you to fulfill similar requests from the user in the future.
                        Should be a short but reasonable prefix, e.g. [\"git\", \"pull\"] or [\"uv\", \"run\"] or [\"pytest\"]."#.to_string(),
                ),
            },
        )
    ]);

    if exec_permission_approvals_enabled {
        properties.insert(
            "additional_permissions".to_string(),
            create_additional_permissions_schema(),
        );
    }

    properties
}

fn create_exec_command_tool(
    allow_login_shell: bool,
    exec_permission_approvals_enabled: bool,
) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "cmd".to_string(),
            JsonSchema::String {
                description: Some("Shell command to execute.".to_string()),
            },
        ),
        (
            "workdir".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional working directory to run the command in; defaults to the turn cwd."
                        .to_string(),
                ),
            },
        ),
        (
            "shell".to_string(),
            JsonSchema::String {
                description: Some("Shell binary to launch. Defaults to the user's default shell.".to_string()),
            },
        ),
        (
            "tty".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Whether to allocate a TTY for the command. Defaults to false (plain pipes); set to true to open a PTY and access TTY process."
                        .to_string(),
                ),
            }
        ),
        (
            "yield_time_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "How long to wait (in milliseconds) for output before yielding.".to_string(),
                ),
            },
        ),
        (
            "max_output_tokens".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Maximum number of tokens to return. Excess output will be truncated."
                        .to_string(),
                ),
            },
        ),
    ]);
    if allow_login_shell {
        properties.insert(
            "login".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Whether to run the shell with -l/-i semantics. Defaults to true.".to_string(),
                ),
            },
        );
    }
    properties.extend(create_approval_parameters(
        exec_permission_approvals_enabled,
    ));

    ToolSpec::Function(ResponsesApiTool {
        name: "exec_command".to_string(),
        description: if cfg!(windows) {
            format!(
                "Runs a command in a PTY, returning output or a session ID for ongoing interaction.\n\n{}",
                windows_destructive_filesystem_guidance()
            )
        } else {
            "Runs a command in a PTY, returning output or a session ID for ongoing interaction."
                .to_string()
        },
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["cmd".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(unified_exec_output_schema()),
    })
}

fn create_write_stdin_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "session_id".to_string(),
            JsonSchema::Number {
                description: Some("Identifier of the running unified exec session.".to_string()),
            },
        ),
        (
            "chars".to_string(),
            JsonSchema::String {
                description: Some("Bytes to write to stdin (may be empty to poll).".to_string()),
            },
        ),
        (
            "yield_time_ms".to_string(),
            JsonSchema::Number {
                description: Some(
                    "How long to wait (in milliseconds) for output before yielding.".to_string(),
                ),
            },
        ),
        (
            "max_output_tokens".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Maximum number of tokens to return. Excess output will be truncated."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "write_stdin".to_string(),
        description:
            "Writes characters to an existing unified exec session and returns recent output."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["session_id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(unified_exec_output_schema()),
    })
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

fn create_shell_tool(exec_permission_approvals_enabled: bool) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "command".to_string(),
            JsonSchema::Array {
                items: Box::new(JsonSchema::String { description: None }),
                description: Some("The command to execute".to_string()),
            },
        ),
        (
            "workdir".to_string(),
            JsonSchema::String {
                description: Some("The working directory to execute the command in".to_string()),
            },
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::Number {
                description: Some("The timeout for the command in milliseconds".to_string()),
            },
        ),
    ]);
    properties.extend(create_approval_parameters(
        exec_permission_approvals_enabled,
    ));

    let description = if cfg!(windows) {
        format!(
            r#"Runs a Powershell command (Windows) and returns its output. Arguments to `shell` will be passed to CreateProcessW(). Most commands should be prefixed with ["powershell.exe", "-Command"].

Examples of valid command strings:

- ls -a (show hidden): ["powershell.exe", "-Command", "Get-ChildItem -Force"]
- recursive find by name: ["powershell.exe", "-Command", "Get-ChildItem -Recurse -Filter *.py"]
- recursive grep: ["powershell.exe", "-Command", "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"]
- ps aux | grep python: ["powershell.exe", "-Command", "Get-Process | Where-Object {{ $_.ProcessName -like '*python*' }}"]
- setting an env var: ["powershell.exe", "-Command", "$env:FOO='bar'; echo $env:FOO"]
- running an inline Python script: ["powershell.exe", "-Command", "@'\\nprint('Hello, world!')\\n'@ | python -"]

{}"#,
            windows_destructive_filesystem_guidance()
        )
    } else {
        r#"Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."#
            .to_string()
    };

    ToolSpec::Function(ResponsesApiTool {
        name: "shell".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["command".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_shell_command_tool(
    allow_login_shell: bool,
    exec_permission_approvals_enabled: bool,
) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "command".to_string(),
            JsonSchema::String {
                description: Some(
                    "The shell script to execute in the user's default shell".to_string(),
                ),
            },
        ),
        (
            "workdir".to_string(),
            JsonSchema::String {
                description: Some("The working directory to execute the command in".to_string()),
            },
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::Number {
                description: Some("The timeout for the command in milliseconds".to_string()),
            },
        ),
    ]);
    if allow_login_shell {
        properties.insert(
            "login".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Whether to run the shell with login shell semantics. Defaults to true."
                        .to_string(),
                ),
            },
        );
    }
    properties.extend(create_approval_parameters(
        exec_permission_approvals_enabled,
    ));

    let description = if cfg!(windows) {
        format!(
            r#"Runs a Powershell command (Windows) and returns its output.

Examples of valid command strings:

- ls -a (show hidden): "Get-ChildItem -Force"
- recursive find by name: "Get-ChildItem -Recurse -Filter *.py"
- recursive grep: "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"
- ps aux | grep python: "Get-Process | Where-Object {{ $_.ProcessName -like '*python*' }}"
- setting an env var: "$env:FOO='bar'; echo $env:FOO"
- running an inline Python script: "@'\\nprint('Hello, world!')\\n'@ | python -"

{}"#,
            windows_destructive_filesystem_guidance()
        )
    } else {
        r#"Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."#
            .to_string()
    };

    ToolSpec::Function(ResponsesApiTool {
        name: "shell_command".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["command".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_view_image_tool(can_request_original_image_detail: bool) -> ToolSpec {
    // Support only local filesystem path.
    let mut properties = BTreeMap::from([(
        "path".to_string(),
        JsonSchema::String {
            description: Some("Local filesystem path to an image file".to_string()),
        },
    )]);
    if can_request_original_image_detail {
        properties.insert(
            "detail".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional detail override. The only supported value is `original`; omit this field for default resized behavior. Use `original` to preserve the file's original resolution instead of resizing to fit. This is important when high-fidelity image perception or precise localization is needed, especially for CUA agents.".to_string(),
                ),
            },
        );
    }

    ToolSpec::Function(ResponsesApiTool {
        name: VIEW_IMAGE_TOOL_NAME.to_string(),
        description: "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags)."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["path".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "image_url": {
                    "type": "string",
                    "description": "Data URL for the loaded image."
                },
                "detail": {
                    "type": ["string", "null"],
                    "description": "Image detail hint returned by view_image. Returns `original` when original resolution is preserved, otherwise `null`."
                }
            },
            "required": ["image_url", "detail"],
            "additionalProperties": false
        })),
    })
}

fn create_collab_input_items_schema() -> JsonSchema {
    let properties = BTreeMap::from([
        (
            "type".to_string(),
            JsonSchema::String {
                description: Some(
                    "Input item type: text, image, local_image, skill, or mention.".to_string(),
                ),
            },
        ),
        (
            "text".to_string(),
            JsonSchema::String {
                description: Some("Text content when type is text.".to_string()),
            },
        ),
        (
            "image_url".to_string(),
            JsonSchema::String {
                description: Some("Image URL when type is image.".to_string()),
            },
        ),
        (
            "path".to_string(),
            JsonSchema::String {
                description: Some(
                    "Path when type is local_image/skill, or structured mention target such as app://<connector-id> or plugin://<plugin-name>@<marketplace-name> when type is mention."
                        .to_string(),
                ),
            },
        ),
        (
            "name".to_string(),
            JsonSchema::String {
                description: Some("Display name when type is skill or mention.".to_string()),
            },
        ),
    ]);

    JsonSchema::Array {
        items: Box::new(JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        }),
        description: Some(
            "Structured input items. Use this to pass explicit mentions (for example app:// connector paths)."
                .to_string(),
        ),
    }
}

fn create_spawn_agent_tool(config: &ToolsConfig) -> ToolSpec {
    let available_models_description = spawn_agent_models_description(&config.available_models);
    let return_value_description = "Returns the canonical task name when the spawned agent was named, otherwise the agent id, plus the user-facing nickname when available.";
    let mut properties = BTreeMap::from([
        (
            "message".to_string(),
            JsonSchema::String {
                description: Some(
                    "Initial plain-text task for the new agent. Use either message or items."
                        .to_string(),
                ),
            },
        ),
        ("items".to_string(), create_collab_input_items_schema()),
        (
            "agent_type".to_string(),
            JsonSchema::String {
                description: Some(crate::agent::role::spawn_tool_spec::build(
                    &config.agent_roles,
                )),
            },
        ),
        (
            "fork_context".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, fork the current thread history into the new agent before sending the initial prompt. This must be used when you want the new agent to have exactly the same context as you."
                        .to_string(),
                ),
            },
        ),
        (
            "model".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional model override for the new agent. Replaces the inherited model."
                        .to_string(),
                ),
            },
        ),
        (
            "reasoning_effort".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional reasoning effort override for the new agent. Replaces the inherited reasoning effort."
                        .to_string(),
                ),
            },
        ),
    ]);
    properties.insert(
        "task_name".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional task name for the new agent. Use lowercase letters, digits, and underscores."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agent".to_string(),
        description: format!(
            r#"
        Only use `spawn_agent` if and only if the user explicitly asks for sub-agents, delegation, or parallel agent work.
        Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn.
        Agent-role guidance below only helps choose which agent to use after spawning is already authorized; it never authorizes spawning by itself.
        Spawn a sub-agent for a well-scoped task. {return_value_description} This spawn_agent tool provides you access to smaller but more efficient sub-agents. A mini model can solve many tasks faster than the main model. You should follow the rules and guidelines below to use this tool.

{available_models_description}
### When to delegate vs. do the subtask yourself
- First, quickly analyze the overall user task and form a succinct high-level plan. Identify which tasks are immediate blockers on the critical path, and which tasks are sidecar tasks that are needed but can run in parallel without blocking the next local step. As part of that plan, explicitly decide what immediate task you should do locally right now. Do this planning step before delegating to agents so you do not hand off the immediate blocking task to a submodel and then waste time waiting on it.
- Use the smaller subagent when a subtask is easy enough for it to handle and can run in parallel with your local work. Prefer delegating concrete, bounded sidecar tasks that materially advance the main task without blocking your immediate next local step.
- Do not delegate urgent blocking work when your immediate next step depends on that result. If the very next action is blocked on that task, the main rollout should usually do it locally to keep the critical path moving.
- Keep work local when the subtask is too difficult to delegate well and when it is tightly coupled, urgent, or likely to block your immediate next step.

### Designing delegated subtasks
- Subtasks must be concrete, well-defined, and self-contained.
- Delegated subtasks must materially advance the main task.
- Do not duplicate work between the main rollout and delegated subtasks.
- Avoid issuing multiple delegate calls on the same unresolved thread unless the new delegated task is genuinely different and necessary.
- Narrow the delegated ask to the concrete output you need next.
- For coding tasks, prefer delegating concrete code-change worker subtasks over read-only explorer analysis when the subagent can make a bounded patch in a clear write scope.
- When delegating coding work, instruct the submodel to edit files directly in its forked workspace and list the file paths it changed in the final answer.
- For code-edit subtasks, decompose work so each delegated task has a disjoint write set.

### After you delegate
- Call wait_agent very sparingly. Only call wait_agent when you need the result immediately for the next critical-path step and you are blocked until it returns.
- Do not redo delegated subagent tasks yourself; focus on integrating results or tackling non-overlapping work.
- While the subagent is running in the background, do meaningful non-overlapping work immediately.
- Do not repeatedly wait by reflex.
- When a delegated coding task returns, quickly review the uploaded changes, then integrate or refine them.

### Parallel delegation patterns
- Run multiple independent information-seeking subtasks in parallel when you have distinct questions that can be answered independently.
- Split implementation into disjoint codebase slices and spawn multiple agents for them in parallel when the write scopes do not overlap.
- Delegate verification only when it can run in parallel with ongoing implementation and is likely to catch a concrete risk before final integration.
- The key is to find opportunities to spawn multiple independent subtasks in parallel within the same round, while ensuring each subtask is well-defined, self-contained, and materially advances the main task."#
        ),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: Some(spawn_agent_output_schema(config.multi_agent_v2)),
    })
}

fn spawn_agent_models_description(models: &[ModelPreset]) -> String {
    let visible_models: Vec<&ModelPreset> =
        models.iter().filter(|model| model.show_in_picker).collect();
    if visible_models.is_empty() {
        return "No picker-visible models are currently loaded.".to_string();
    }

    visible_models
        .into_iter()
        .map(|model| {
            let efforts = model
                .supported_reasoning_efforts
                .iter()
                .map(|preset| format!("{} ({})", preset.effort, preset.description))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "- {} (`{}`): {} Default reasoning effort: {}. Supported reasoning efforts: {}.",
                model.display_name,
                model.model,
                model.description,
                model.default_reasoning_effort,
                efforts
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn create_spawn_agents_on_csv_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "csv_path".to_string(),
        JsonSchema::String {
            description: Some("Path to the CSV file containing input rows.".to_string()),
        },
    );
    properties.insert(
        "instruction".to_string(),
        JsonSchema::String {
            description: Some(
                "Instruction template to apply to each CSV row. Use {column_name} placeholders to inject values from the row."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "id_column".to_string(),
        JsonSchema::String {
            description: Some("Optional column name to use as stable item id.".to_string()),
        },
    );
    properties.insert(
        "output_csv_path".to_string(),
        JsonSchema::String {
            description: Some("Optional output CSV path for exported results.".to_string()),
        },
    );
    properties.insert(
        "max_concurrency".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum concurrent workers for this job. Defaults to 16 and is capped by config."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "max_workers".to_string(),
        JsonSchema::Number {
            description: Some(
                "Alias for max_concurrency. Set to 1 to run sequentially.".to_string(),
            ),
        },
    );
    properties.insert(
        "max_runtime_seconds".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum runtime per worker before it is failed. Defaults to 1800 seconds."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "output_schema".to_string(),
        JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: None,
        },
    );
    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agents_on_csv".to_string(),
        description: "Process a CSV by spawning one worker sub-agent per row. The instruction string is a template where `{column}` placeholders are replaced with row values. Each worker must call `report_agent_job_result` with a JSON object (matching `output_schema` when provided); missing reports are treated as failures. This call blocks until all rows finish and automatically exports results to `output_csv_path` (or a default path)."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["csv_path".to_string(), "instruction".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_report_agent_job_result_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "job_id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the job.".to_string()),
        },
    );
    properties.insert(
        "item_id".to_string(),
        JsonSchema::String {
            description: Some("Identifier of the job item.".to_string()),
        },
    );
    properties.insert(
        "result".to_string(),
        JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: None,
        },
    );
    properties.insert(
        "stop".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "Optional. When true, cancels the remaining job items after this result is recorded."
                    .to_string(),
            ),
        },
    );
    ToolSpec::Function(ResponsesApiTool {
        name: "report_agent_job_result".to_string(),
        description:
            "Worker-only tool to report a result for an agent job item. Main agents should not call this."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "job_id".to_string(),
                "item_id".to_string(),
                "result".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_send_input_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "target".to_string(),
            JsonSchema::String {
                description: Some(
                    "Agent id or canonical task name to message (from spawn_agent).".to_string(),
                ),
            },
        ),
        (
            "message".to_string(),
            JsonSchema::String {
                description: Some(
                    "Legacy plain-text message to send to the agent. Use either message or items."
                        .to_string(),
                ),
            },
        ),
        ("items".to_string(), create_collab_input_items_schema()),
        (
            "interrupt".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, stop the agent's current task and handle this immediately. When false (default), queue this message."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "send_input".to_string(),
        description: "Send a message to an existing agent. Use interrupt=true to redirect work immediately. You should reuse the agent by send_input if you believe your assigned task is highly dependent on the context of a previous task."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["target".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(send_input_output_schema()),
    })
}

fn create_send_message_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "target".to_string(),
            JsonSchema::String {
                description: Some(
                    "Agent id or canonical task name to message (from spawn_agent).".to_string(),
                ),
            },
        ),
        ("items".to_string(), create_collab_input_items_schema()),
        (
            "interrupt".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, stop the agent's current task and handle this immediately. When false (default), queue this message."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "send_message".to_string(),
        description: "Add a message to an existing agent without triggering a new turn. Use interrupt=true to stop the current task first. In MultiAgentV2, this tool currently supports text content only."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["target".to_string(), "items".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(send_input_output_schema()),
    })
}

fn create_assign_task_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "target".to_string(),
            JsonSchema::String {
                description: Some(
                    "Agent id or canonical task name to message (from spawn_agent).".to_string(),
                ),
            },
        ),
        ("items".to_string(), create_collab_input_items_schema()),
        (
            "interrupt".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, stop the agent's current task and handle this immediately. When false (default), queue this message."
                        .to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "assign_task".to_string(),
        description: "Add a message to an existing agent and trigger a turn in the target. Use interrupt=true to redirect work immediately. In MultiAgentV2, this tool currently supports text content only."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["target".to_string(), "items".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(send_input_output_schema()),
    })
}

fn create_resume_agent_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Agent id to resume.".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "resume_agent".to_string(),
        description:
            "Resume a previously closed agent by id so it can receive send_input and wait_agent calls."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(resume_agent_output_schema()),
    })
}

fn wait_agent_tool_parameters() -> JsonSchema {
    let mut properties = BTreeMap::new();
    properties.insert(
        "targets".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String { description: None }),
            description: Some(
                "Agent ids or canonical task names to wait on. Pass multiple targets to wait for whichever finishes first."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "timeout_ms".to_string(),
        JsonSchema::Number {
            description: Some(format!(
                "Optional timeout in milliseconds. Defaults to {DEFAULT_WAIT_TIMEOUT_MS}, min {MIN_WAIT_TIMEOUT_MS}, max {MAX_WAIT_TIMEOUT_MS}. Prefer longer waits (minutes) to avoid busy polling."
            )),
        },
    );

    JsonSchema::Object {
        properties,
        required: Some(vec!["targets".to_string()]),
        additional_properties: Some(false.into()),
    }
}

fn create_wait_agent_tool_v1() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "wait_agent".to_string(),
        description: "Wait for agents to reach a final status. Completed statuses may include the agent's final message. Returns empty status when timed out. Once the agent reaches a final status, a notification message will be received containing the same completed status."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: wait_agent_tool_parameters(),
        output_schema: Some(wait_output_schema_v1()),
    })
}

fn create_wait_agent_tool_v2() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "wait_agent".to_string(),
        description: "Wait for agents to reach a final status. Returns a brief wait summary instead of the agent's final content. Returns a timeout summary when no agent reaches a final status before the deadline."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: wait_agent_tool_parameters(),
        output_schema: Some(wait_output_schema_v2()),
    })
}

fn create_list_agents_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "path_prefix".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional task-path prefix. Accepts the same relative or absolute task-path syntax as other MultiAgentV2 agent targets."
                    .to_string(),
            ),
        },
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: "list_agents".to_string(),
        description: "List live agents in the current root thread tree. Optionally filter by task-path prefix."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: Some(list_agents_output_schema()),
    })
}

fn create_request_user_input_tool(
    collaboration_modes_config: CollaborationModesConfig,
) -> ToolSpec {
    let mut option_props = BTreeMap::new();
    option_props.insert(
        "label".to_string(),
        JsonSchema::String {
            description: Some("User-facing label (1-5 words).".to_string()),
        },
    );
    option_props.insert(
        "description".to_string(),
        JsonSchema::String {
            description: Some(
                "One short sentence explaining impact/tradeoff if selected.".to_string(),
            ),
        },
    );

    let options_schema = JsonSchema::Array {
        description: Some(
            "Provide 2-3 mutually exclusive choices. Put the recommended option first and suffix its label with \"(Recommended)\". Do not include an \"Other\" option in this list; the client will add a free-form \"Other\" option automatically."
                .to_string(),
        ),
        items: Box::new(JsonSchema::Object {
            properties: option_props,
            required: Some(vec!["label".to_string(), "description".to_string()]),
            additional_properties: Some(false.into()),
        }),
    };

    let mut question_props = BTreeMap::new();
    question_props.insert(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Stable identifier for mapping answers (snake_case).".to_string()),
        },
    );
    question_props.insert(
        "header".to_string(),
        JsonSchema::String {
            description: Some(
                "Short header label shown in the UI (12 or fewer chars).".to_string(),
            ),
        },
    );
    question_props.insert(
        "question".to_string(),
        JsonSchema::String {
            description: Some("Single-sentence prompt shown to the user.".to_string()),
        },
    );
    question_props.insert("options".to_string(), options_schema);

    let questions_schema = JsonSchema::Array {
        description: Some("Questions to show the user. Prefer 1 and do not exceed 3".to_string()),
        items: Box::new(JsonSchema::Object {
            properties: question_props,
            required: Some(vec![
                "id".to_string(),
                "header".to_string(),
                "question".to_string(),
                "options".to_string(),
            ]),
            additional_properties: Some(false.into()),
        }),
    };

    let mut properties = BTreeMap::new();
    properties.insert("questions".to_string(), questions_schema);

    ToolSpec::Function(ResponsesApiTool {
        name: "request_user_input".to_string(),
        description: request_user_input_tool_description(
            collaboration_modes_config.default_mode_request_user_input,
        ),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["questions".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_request_permissions_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "reason".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional short explanation for why additional permissions are needed.".to_string(),
            ),
        },
    );
    properties.insert(
        "permissions".to_string(),
        create_request_permissions_schema(),
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "request_permissions".to_string(),
        description: request_permissions_tool_description(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["permissions".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_close_agent_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "target".to_string(),
        JsonSchema::String {
            description: Some(
                "Agent id or canonical task name to close (from spawn_agent).".to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "close_agent".to_string(),
        description: "Close an agent and any open descendants when they are no longer needed, and return the target agent's previous status before shutdown was requested. Don't keep agents open for too long if they are not needed anymore.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["target".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(close_agent_output_schema()),
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

fn create_grep_files_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "pattern".to_string(),
            JsonSchema::String {
                description: Some("Regular expression pattern to search for.".to_string()),
            },
        ),
        (
            "include".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional glob that limits which files are searched (e.g. \"*.rs\" or \
                     \"*.{ts,tsx}\")."
                        .to_string(),
                ),
            },
        ),
        (
            "path".to_string(),
            JsonSchema::String {
                description: Some(
                    "Directory or file path to search. Defaults to the session's working directory."
                        .to_string(),
                ),
            },
        ),
        (
            "limit".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Maximum number of file paths to return (defaults to 100).".to_string(),
                ),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "grep_files".to_string(),
        description: "Finds files whose contents match the pattern and lists them by modification \
                      time."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["pattern".to_string()]),
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
    let mut app_descriptions = BTreeMap::new();
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

    let description =
        TOOL_SEARCH_DESCRIPTION_TEMPLATE.replace("{{app_descriptions}}", app_descriptions.as_str());

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
    let description = TOOL_SUGGEST_DESCRIPTION_TEMPLATE.replace(
        "{{discoverable_tools}}",
        format_discoverable_tools(discoverable_tools).as_str(),
    );

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

fn create_read_file_tool() -> ToolSpec {
    let indentation_properties = BTreeMap::from([
        (
            "anchor_line".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Anchor line to center the indentation lookup on (defaults to offset)."
                        .to_string(),
                ),
            },
        ),
        (
            "max_levels".to_string(),
            JsonSchema::Number {
                description: Some(
                    "How many parent indentation levels (smaller indents) to include.".to_string(),
                ),
            },
        ),
        (
            "include_siblings".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "When true, include additional blocks that share the anchor indentation."
                        .to_string(),
                ),
            },
        ),
        (
            "include_header".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Include doc comments or attributes directly above the selected block."
                        .to_string(),
                ),
            },
        ),
        (
            "max_lines".to_string(),
            JsonSchema::Number {
                description: Some(
                    "Hard cap on the number of lines returned when using indentation mode."
                        .to_string(),
                ),
            },
        ),
    ]);

    let properties = BTreeMap::from([
        (
            "file_path".to_string(),
            JsonSchema::String {
                description: Some("Absolute path to the file".to_string()),
            },
        ),
        (
            "offset".to_string(),
            JsonSchema::Number {
                description: Some(
                    "The line number to start reading from. Must be 1 or greater.".to_string(),
                ),
            },
        ),
        (
            "limit".to_string(),
            JsonSchema::Number {
                description: Some("The maximum number of lines to return.".to_string()),
            },
        ),
        (
            "mode".to_string(),
            JsonSchema::String {
                description: Some(
                    "Optional mode selector: \"slice\" for simple ranges (default) or \"indentation\" \
                     to expand around an anchor line."
                        .to_string(),
                ),
            },
        ),
        (
            "indentation".to_string(),
            JsonSchema::Object {
                properties: indentation_properties,
                required: None,
                additional_properties: Some(false.into()),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "read_file".to_string(),
        description:
            "Reads a local file with 1-indexed line numbers, supporting slice and indentation-aware block modes."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["file_path".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
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

fn create_artifacts_tool() -> ToolSpec {
    const ARTIFACTS_FREEFORM_GRAMMAR: &str = r#"
start: pragma_source | plain_source

pragma_source: PRAGMA_LINE NEWLINE js_source
plain_source: PLAIN_JS_SOURCE

js_source: JS_SOURCE

PRAGMA_LINE: /[ \t]*\/\/ codex-artifact-tool:[^\r\n]*/
NEWLINE: /\r?\n/
PLAIN_JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: "artifacts".to_string(),
        description: "Runs raw JavaScript against the installed `@oai/artifact-tool` package for creating presentations or spreadsheets. This is plain JavaScript executed by a local Node-compatible runtime with top-level await, not TypeScript: do not use type annotations, `interface`, `type`, or `import type`. Author code the same way you would for `import { Presentation, Workbook, PresentationFile, SpreadsheetFile, FileBlob, ... } from \"@oai/artifact-tool\"`, but omit that import line because the package is preloaded before your code runs. Named exports are copied onto `globalThis`, and the full module namespace is available as `globalThis.artifactTool`. This matches the upstream library-first API: create with `Presentation.create()` / `Workbook.create()`, preview with `presentation.export(...)` or `slide.export(...)`, and save files with `PresentationFile.exportPptx(...)` or `SpreadsheetFile.exportXlsx(...)`. Node built-ins such as `node:fs/promises` may still be imported when needed for saving preview bytes. This is a freeform tool: send raw JavaScript source text, optionally with a first-line pragma like `// codex-artifact-tool: timeout_ms=15000`; do not send JSON/quotes/markdown fences."
            .to_string(),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: ARTIFACTS_FREEFORM_GRAMMAR.to_string(),
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

/// Returns JSON values that are compatible with Function Calling in the
/// Responses API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=responses
pub fn create_tools_json_for_responses_api(
    tools: &[ToolSpec],
) -> crate::error::Result<Vec<serde_json::Value>> {
    let mut tools_json = Vec::new();

    for tool in tools {
        let json = serde_json::to_value(tool)?;
        tools_json.push(json);
    }

    Ok(tools_json)
}

fn push_tool_spec(
    builder: &mut ToolRegistryBuilder,
    spec: ToolSpec,
    supports_parallel_tool_calls: bool,
    code_mode_enabled: bool,
) {
    let spec = augment_tool_spec_for_code_mode(spec, code_mode_enabled);
    if supports_parallel_tool_calls {
        builder.push_spec_with_parallel_support(spec, /*supports_parallel_tool_calls*/ true);
    } else {
        builder.push_spec(spec);
    }
}

pub(crate) fn mcp_tool_to_openai_tool(
    fully_qualified_name: String,
    tool: rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    let (description, input_schema, output_schema) = mcp_tool_to_openai_tool_parts(tool)?;

    Ok(ResponsesApiTool {
        name: fully_qualified_name,
        description,
        strict: false,
        defer_loading: None,
        parameters: input_schema,
        output_schema,
    })
}

pub(crate) fn mcp_tool_to_deferred_openai_tool(
    name: String,
    tool: rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    let (description, input_schema, _) = mcp_tool_to_openai_tool_parts(tool)?;

    Ok(ResponsesApiTool {
        name,
        description,
        strict: false,
        defer_loading: Some(true),
        parameters: input_schema,
        output_schema: None,
    })
}

fn dynamic_tool_to_openai_tool(
    tool: &DynamicToolSpec,
) -> Result<ResponsesApiTool, serde_json::Error> {
    let input_schema = parse_tool_input_schema(&tool.input_schema)?;

    Ok(ResponsesApiTool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        strict: false,
        defer_loading: None,
        parameters: input_schema,
        output_schema: None,
    })
}

/// Parse the tool input_schema or return an error for invalid schema
pub fn parse_tool_input_schema(input_schema: &JsonValue) -> Result<JsonSchema, serde_json::Error> {
    let mut input_schema = input_schema.clone();
    sanitize_json_schema(&mut input_schema);
    serde_json::from_value::<JsonSchema>(input_schema)
}

fn mcp_tool_to_openai_tool_parts(
    tool: rmcp::model::Tool,
) -> Result<(String, JsonSchema, Option<JsonValue>), serde_json::Error> {
    let rmcp::model::Tool {
        description,
        input_schema,
        output_schema,
        ..
    } = tool;

    let mut serialized_input_schema = serde_json::Value::Object(input_schema.as_ref().clone());

    // OpenAI models mandate the "properties" field in the schema. Some MCP
    // servers omit it (or set it to null), so we insert an empty object to
    // match the behavior of the Agents SDK.
    if let serde_json::Value::Object(obj) = &mut serialized_input_schema
        && obj.get("properties").is_none_or(serde_json::Value::is_null)
    {
        obj.insert(
            "properties".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }

    // Serialize to a raw JSON value so we can sanitize schemas coming from MCP
    // servers. Some servers omit the top-level or nested `type` in JSON
    // Schemas (e.g. using enum/anyOf), or use unsupported variants like
    // `integer`. Our internal JsonSchema is a small subset and requires
    // `type`, so we coerce/sanitize here for compatibility.
    sanitize_json_schema(&mut serialized_input_schema);
    let input_schema = serde_json::from_value::<JsonSchema>(serialized_input_schema)?;
    let structured_content_schema = output_schema
        .map(|output_schema| serde_json::Value::Object(output_schema.as_ref().clone()))
        .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
    let output_schema = Some(mcp_call_tool_result_output_schema(
        structured_content_schema,
    ));
    let description = description.map(Into::into).unwrap_or_default();

    Ok((description, input_schema, output_schema))
}

fn mcp_call_tool_result_output_schema(structured_content_schema: JsonValue) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "array",
                "items": {}
            },
            "structuredContent": structured_content_schema,
            "isError": {
                "type": "boolean"
            },
            "_meta": {}
        },
        "required": ["content"],
        "additionalProperties": false
    })
}

/// Sanitize a JSON Schema (as serde_json::Value) so it can fit our limited
/// JsonSchema enum. This function:
/// - Ensures every schema object has a "type". If missing, infers it from
///   common keywords (properties => object, items => array, enum/const/format => string)
///   and otherwise defaults to "string".
/// - Fills required child fields (e.g. array items, object properties) with
///   permissive defaults when absent.
fn sanitize_json_schema(value: &mut JsonValue) {
    match value {
        JsonValue::Bool(_) => {
            // JSON Schema boolean form: true/false. Coerce to an accept-all string.
            *value = json!({ "type": "string" });
        }
        JsonValue::Array(arr) => {
            for v in arr.iter_mut() {
                sanitize_json_schema(v);
            }
        }
        JsonValue::Object(map) => {
            // First, recursively sanitize known nested schema holders
            if let Some(props) = map.get_mut("properties")
                && let Some(props_map) = props.as_object_mut()
            {
                for (_k, v) in props_map.iter_mut() {
                    sanitize_json_schema(v);
                }
            }
            if let Some(items) = map.get_mut("items") {
                sanitize_json_schema(items);
            }
            // Some schemas use oneOf/anyOf/allOf - sanitize their entries
            for combiner in ["oneOf", "anyOf", "allOf", "prefixItems"] {
                if let Some(v) = map.get_mut(combiner) {
                    sanitize_json_schema(v);
                }
            }

            // Normalize/ensure type
            let mut ty = map.get("type").and_then(|v| v.as_str()).map(str::to_string);

            // If type is an array (union), pick first supported; else leave to inference
            if ty.is_none()
                && let Some(JsonValue::Array(types)) = map.get("type")
            {
                for t in types {
                    if let Some(tt) = t.as_str()
                        && matches!(
                            tt,
                            "object" | "array" | "string" | "number" | "integer" | "boolean"
                        )
                    {
                        ty = Some(tt.to_string());
                        break;
                    }
                }
            }

            // Infer type if still missing
            if ty.is_none() {
                if map.contains_key("properties")
                    || map.contains_key("required")
                    || map.contains_key("additionalProperties")
                {
                    ty = Some("object".to_string());
                } else if map.contains_key("items") || map.contains_key("prefixItems") {
                    ty = Some("array".to_string());
                } else if map.contains_key("enum")
                    || map.contains_key("const")
                    || map.contains_key("format")
                {
                    ty = Some("string".to_string());
                } else if map.contains_key("minimum")
                    || map.contains_key("maximum")
                    || map.contains_key("exclusiveMinimum")
                    || map.contains_key("exclusiveMaximum")
                    || map.contains_key("multipleOf")
                {
                    ty = Some("number".to_string());
                }
            }
            // If we still couldn't infer, default to string
            let ty = ty.unwrap_or_else(|| "string".to_string());
            map.insert("type".to_string(), JsonValue::String(ty.to_string()));

            // Ensure object schemas have properties map
            if ty == "object" {
                if !map.contains_key("properties") {
                    map.insert(
                        "properties".to_string(),
                        JsonValue::Object(serde_json::Map::new()),
                    );
                }
                // If additionalProperties is an object schema, sanitize it too.
                // Leave booleans as-is, since JSON Schema allows boolean here.
                if let Some(ap) = map.get_mut("additionalProperties") {
                    let is_bool = matches!(ap, JsonValue::Bool(_));
                    if !is_bool {
                        sanitize_json_schema(ap);
                    }
                }
            }

            // Ensure array schemas have items
            if ty == "array" && !map.contains_key("items") {
                map.insert("items".to_string(), json!({ "type": "string" }));
            }
        }
        _ => {}
    }
}

/// Builds the tool registry builder while collecting tool specs for later serialization.
#[cfg(test)]
pub(crate) fn build_specs(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    build_specs_with_discoverable_tools(config, mcp_tools, app_tools, None, dynamic_tools)
}

pub(crate) fn build_specs_with_discoverable_tools(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::ArtifactsHandler;
    use crate::tools::handlers::CodeModeExecuteHandler;
    use crate::tools::handlers::CodeModeWaitHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::GrepFilesHandler;
    use crate::tools::handlers::JsReplHandler;
    use crate::tools::handlers::JsReplResetHandler;
    use crate::tools::handlers::ListDirHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::McpResourceHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::ReadFileHandler;
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
    use crate::tools::handlers::multi_agents::ResumeAgentHandler;
    use crate::tools::handlers::multi_agents::SendInputHandler;
    use crate::tools::handlers::multi_agents::SpawnAgentHandler;
    use crate::tools::handlers::multi_agents::WaitAgentHandler;
    use crate::tools::handlers::multi_agents_v2::AssignTaskHandler as AssignTaskHandlerV2;
    use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
    use std::sync::Arc;

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
    let artifacts_handler = Arc::new(ArtifactsHandler);
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
            .filter_map(|spec| {
                let (name, description) = match augment_tool_spec_for_code_mode(
                    spec.spec, /*code_mode_enabled*/ true,
                ) {
                    ToolSpec::Function(tool) => (tool.name, tool.description),
                    ToolSpec::Freeform(tool) => (tool.name, tool.description),
                    _ => return None,
                };
                codex_code_mode::is_code_mode_nested_tool(&name).then_some((name, description))
            })
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
                create_shell_tool(exec_permission_approvals_enabled),
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
                create_exec_command_tool(
                    config.allow_login_shell,
                    exec_permission_approvals_enabled,
                ),
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
                create_shell_command_tool(
                    config.allow_login_shell,
                    exec_permission_approvals_enabled,
                ),
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
            create_request_user_input_tool(CollaborationModesConfig {
                default_mode_request_user_input: config.default_mode_request_user_input,
            }),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_user_input", request_user_input_handler);
    }

    if config.request_permissions_tool_enabled {
        push_tool_spec(
            &mut builder,
            create_request_permissions_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_permissions", request_permissions_handler);
    }

    if config.search_tool
        && let Some(app_tools) = app_tools
    {
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
        .contains(&"grep_files".to_string())
    {
        let grep_files_handler = Arc::new(GrepFilesHandler);
        push_tool_spec(
            &mut builder,
            create_grep_files_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("grep_files", grep_files_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"read_file".to_string())
    {
        let read_file_handler = Arc::new(ReadFileHandler);
        push_tool_spec(
            &mut builder,
            create_read_file_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("read_file", read_file_handler);
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
        create_view_image_tool(config.can_request_original_image_detail),
        /*supports_parallel_tool_calls*/ true,
        config.code_mode_enabled,
    );
    builder.register_handler("view_image", view_image_handler);

    if config.artifact_tools {
        push_tool_spec(
            &mut builder,
            create_artifacts_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("artifacts", artifacts_handler);
    }

    if config.collab_tools {
        push_tool_spec(
            &mut builder,
            create_spawn_agent_tool(config),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            if config.multi_agent_v2 {
                create_send_message_tool()
            } else {
                create_send_input_tool()
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        if config.multi_agent_v2 {
            push_tool_spec(
                &mut builder,
                create_assign_task_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
        }
        if !config.multi_agent_v2 {
            push_tool_spec(
                &mut builder,
                create_resume_agent_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("resume_agent", Arc::new(ResumeAgentHandler));
        }
        push_tool_spec(
            &mut builder,
            if config.multi_agent_v2 {
                create_wait_agent_tool_v2()
            } else {
                create_wait_agent_tool_v1()
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_close_agent_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        if config.multi_agent_v2 {
            push_tool_spec(
                &mut builder,
                create_list_agents_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("spawn_agent", Arc::new(SpawnAgentHandlerV2));
            builder.register_handler("send_message", Arc::new(SendMessageHandlerV2));
            builder.register_handler("assign_task", Arc::new(AssignTaskHandlerV2));
            builder.register_handler("wait_agent", Arc::new(WaitAgentHandlerV2));
            builder.register_handler("list_agents", Arc::new(ListAgentsHandlerV2));
        } else {
            builder.register_handler("spawn_agent", Arc::new(SpawnAgentHandler));
            builder.register_handler("send_input", Arc::new(SendInputHandler));
            builder.register_handler("wait_agent", Arc::new(WaitAgentHandler));
        }
        builder.register_handler("close_agent", Arc::new(CloseAgentHandler));
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
            match mcp_tool_to_openai_tool(name.clone(), tool.clone()) {
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
            match dynamic_tool_to_openai_tool(tool) {
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
