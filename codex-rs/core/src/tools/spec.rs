use crate::client_common::tools::FreeformTool;
use crate::client_common::tools::FreeformToolFormat;
use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::config::AgentRoleConfig;
use crate::features::Feature;
use crate::features::Features;
use crate::mcp_connection_manager::ToolInfo;
use crate::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode_description::augment_tool_spec_for_code_mode;
use crate::tools::handlers::PLAN_TOOL;
use crate::tools::handlers::SEARCH_TOOL_BM25_DEFAULT_LIMIT;
use crate::tools::handlers::SEARCH_TOOL_BM25_TOOL_NAME;
use crate::tools::handlers::agent_jobs::BatchJobHandler;
use crate::tools::handlers::apply_patch::create_apply_patch_freeform_tool;
use crate::tools::handlers::apply_patch::create_apply_patch_json_tool;
use crate::tools::handlers::multi_agents::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::request_permissions_tool_description;
use crate::tools::handlers::request_user_input_tool_description;
use crate::tools::registry::ToolRegistryBuilder;
use codex_protocol::config_types::WebSearchConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;

const SEARCH_TOOL_BM25_DESCRIPTION_TEMPLATE: &str =
    include_str!("../../templates/search_tool/tool_description.md");
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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShellCommandBackendConfig {
    Classic,
    ZshFork,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnifiedExecBackendConfig {
    Direct,
    ZshFork,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolsConfig {
    pub available_models: Vec<ModelPreset>,
    pub shell_type: ConfigShellToolType,
    shell_command_backend: ShellCommandBackendConfig,
    pub unified_exec_backend: UnifiedExecBackendConfig,
    pub allow_login_shell: bool,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_mode: Option<WebSearchMode>,
    pub web_search_config: Option<WebSearchConfig>,
    pub web_search_tool_type: WebSearchToolType,
    pub image_gen_tool: bool,
    pub agent_roles: BTreeMap<String, AgentRoleConfig>,
    pub search_tool: bool,
    pub request_permission_enabled: bool,
    pub request_permissions_tool_enabled: bool,
    pub code_mode_enabled: bool,
    pub js_repl_enabled: bool,
    pub js_repl_tools_only: bool,
    pub collab_tools: bool,
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
}

impl ToolsConfig {
    pub fn new(params: &ToolsConfigParams) -> Self {
        let ToolsConfigParams {
            model_info,
            available_models: available_models_ref,
            features,
            web_search_mode,
            session_source,
        } = params;
        let include_apply_patch_tool = features.enabled(Feature::ApplyPatchFreeform);
        let include_code_mode = features.enabled(Feature::CodeMode);
        let include_js_repl = features.enabled(Feature::JsRepl);
        let include_js_repl_tools_only =
            include_js_repl && features.enabled(Feature::JsReplToolsOnly);
        let include_collab_tools = features.enabled(Feature::Collab);
        let include_request_user_input = !matches!(session_source, SessionSource::SubAgent(_));
        let include_default_mode_request_user_input =
            include_request_user_input && features.enabled(Feature::DefaultModeRequestUserInput);
        let include_search_tool = features.enabled(Feature::Apps);
        let include_artifact_tools =
            features.enabled(Feature::Artifact) && codex_artifacts::can_manage_artifact_runtime();
        let include_image_gen_tool =
            features.enabled(Feature::ImageGeneration) && supports_image_generation(model_info);
        let include_agent_jobs = features.enabled(Feature::SpawnCsv);
        let request_permission_enabled = features.enabled(Feature::RequestPermissions);
        let request_permissions_tool_enabled = features.enabled(Feature::RequestPermissionsTool);
        let shell_command_backend =
            if features.enabled(Feature::ShellTool) && features.enabled(Feature::ShellZshFork) {
                ShellCommandBackendConfig::ZshFork
            } else {
                ShellCommandBackendConfig::Classic
            };
        let unified_exec_backend =
            if features.enabled(Feature::ShellTool) && features.enabled(Feature::ShellZshFork) {
                UnifiedExecBackendConfig::ZshFork
            } else {
                UnifiedExecBackendConfig::Direct
            };

        let shell_type = if !features.enabled(Feature::ShellTool) {
            ConfigShellToolType::Disabled
        } else if features.enabled(Feature::ShellZshFork) {
            ConfigShellToolType::ShellCommand
        } else if features.enabled(Feature::UnifiedExec) {
            // If ConPTY not supported (for old Windows versions), fallback on ShellCommand.
            if codex_utils_pty::conpty_supported() {
                ConfigShellToolType::UnifiedExec
            } else {
                ConfigShellToolType::ShellCommand
            }
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
            unified_exec_backend,
            allow_login_shell: true,
            apply_patch_tool_type,
            web_search_mode: *web_search_mode,
            web_search_config: None,
            web_search_tool_type: model_info.web_search_tool_type,
            image_gen_tool: include_image_gen_tool,
            agent_roles: BTreeMap::new(),
            search_tool: include_search_tool,
            request_permission_enabled,
            request_permissions_tool_enabled,
            code_mode_enabled: include_code_mode,
            js_repl_enabled: include_js_repl,
            js_repl_tools_only: include_js_repl_tools_only,
            collab_tools: include_collab_tools,
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

    pub fn with_web_search_config(mut self, web_search_config: Option<WebSearchConfig>) -> Self {
        self.web_search_config = web_search_config;
        self
    }

    pub fn for_code_mode_nested_tools(&self) -> Self {
        let mut nested = self.clone();
        nested.code_mode_enabled = false;
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

fn create_macos_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([
            (
                "preferences".to_string(),
                JsonSchema::String {
                    description: Some(
                        "macOS preferences access. Supported values: `none`, `read_only`, or `read_write`."
                            .to_string(),
                    ),
                },
            ),
            (
                "automations".to_string(),
                JsonSchema::Array {
                    items: Box::new(JsonSchema::String { description: None }),
                    description: Some("macOS automation access as app bundle identifiers.".to_string()),
                },
            ),
            (
                "accessibility".to_string(),
                JsonSchema::Boolean {
                    description: Some("Whether to request macOS accessibility access.".to_string()),
                },
            ),
            (
                "calendar".to_string(),
                JsonSchema::Boolean {
                    description: Some("Whether to request macOS calendar access.".to_string()),
                },
            ),
        ]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn create_permissions_schema() -> JsonSchema {
    JsonSchema::Object {
        properties: BTreeMap::from([
            ("network".to_string(), create_network_permissions_schema()),
            (
                "file_system".to_string(),
                create_file_system_permissions_schema(),
            ),
            ("macos".to_string(), create_macos_permissions_schema()),
        ]),
        required: None,
        additional_properties: Some(false.into()),
    }
}

fn create_approval_parameters(request_permission_enabled: bool) -> BTreeMap<String, JsonSchema> {
    let mut properties = BTreeMap::from([
        (
            "sandbox_permissions".to_string(),
            JsonSchema::String {
                description: Some(
                    if request_permission_enabled {
                        "Sandbox permissions for the command. Use \"with_additional_permissions\" to request additional sandboxed filesystem, network, or macOS permissions (preferred), or \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
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

    if request_permission_enabled {
        properties.insert(
            "additional_permissions".to_string(),
            create_permissions_schema(),
        );
    }

    properties
}

fn create_exec_command_tool(allow_login_shell: bool, request_permission_enabled: bool) -> ToolSpec {
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
    properties.extend(create_approval_parameters(request_permission_enabled));

    ToolSpec::Function(ResponsesApiTool {
        name: "exec_command".to_string(),
        description:
            "Runs a command in a PTY, returning output or a session ID for ongoing interaction."
                .to_string(),
        strict: false,
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
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["session_id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: Some(unified_exec_output_schema()),
    })
}

fn create_shell_tool(request_permission_enabled: bool) -> ToolSpec {
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
    properties.extend(create_approval_parameters(request_permission_enabled));

    let description  = if cfg!(windows) {
        r#"Runs a Powershell command (Windows) and returns its output. Arguments to `shell` will be passed to CreateProcessW(). Most commands should be prefixed with ["powershell.exe", "-Command"].

Examples of valid command strings:

- ls -a (show hidden): ["powershell.exe", "-Command", "Get-ChildItem -Force"]
- recursive find by name: ["powershell.exe", "-Command", "Get-ChildItem -Recurse -Filter *.py"]
- recursive grep: ["powershell.exe", "-Command", "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"]
- ps aux | grep python: ["powershell.exe", "-Command", "Get-Process | Where-Object { $_.ProcessName -like '*python*' }"]
- setting an env var: ["powershell.exe", "-Command", "$env:FOO='bar'; echo $env:FOO"]
- running an inline Python script: ["powershell.exe", "-Command", "@'\\nprint('Hello, world!')\\n'@ | python -"]"#
    } else {
        r#"Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."#
    }.to_string();

    ToolSpec::Function(ResponsesApiTool {
        name: "shell".to_string(),
        description,
        strict: false,
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
    request_permission_enabled: bool,
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
    properties.extend(create_approval_parameters(request_permission_enabled));

    let description = if cfg!(windows) {
        r#"Runs a Powershell command (Windows) and returns its output.

Examples of valid command strings:

- ls -a (show hidden): "Get-ChildItem -Force"
- recursive find by name: "Get-ChildItem -Recurse -Filter *.py"
- recursive grep: "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"
- ps aux | grep python: "Get-Process | Where-Object { $_.ProcessName -like '*python*' }"
- setting an env var: "$env:FOO='bar'; echo $env:FOO"
- running an inline Python script: "@'\\nprint('Hello, world!')\\n'@ | python -"#
    } else {
        r#"Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."#
    }.to_string();

    ToolSpec::Function(ResponsesApiTool {
        name: "shell_command".to_string(),
        description,
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["command".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_view_image_tool() -> ToolSpec {
    // Support only local filesystem path.
    let properties = BTreeMap::from([(
        "path".to_string(),
        JsonSchema::String {
            description: Some("Local filesystem path to an image file".to_string()),
        },
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: VIEW_IMAGE_TOOL_NAME.to_string(),
        description: "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags)."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["path".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
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
    let properties = BTreeMap::from([
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

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agent".to_string(),
        description: format!(
            r#"
        Only use `spawn_agent` if and only if the user explicitly asked for sub-agents or parallel agent work. Spawn a sub-agent for a well-scoped task. Returns the agent id (and user-facing nickname when available) to use to communicate with this agent. This spawn_agent tool provides you access to smaller but more efficient sub-agents. A mini model can solve many tasks faster than the main model. You should follow the rules and guidelines below to use this tool.

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
- Call wait very sparingly. Only call wait when you need the result immediately for the next critical-path step and you are blocked until it returns.
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
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
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
            "id".to_string(),
            JsonSchema::String {
                description: Some("Agent id to message (from spawn_agent).".to_string()),
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
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
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
            "Resume a previously closed agent by id so it can receive send_input and wait calls."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_wait_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "ids".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String { description: None }),
            description: Some(
                "Agent ids to wait on. Pass multiple ids to wait for whichever finishes first."
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

    ToolSpec::Function(ResponsesApiTool {
        name: "wait".to_string(),
        description: "Wait for agents to reach a final status. Completed statuses may include the agent's final message. Returns empty status when timed out. Once the agent reaches a final status, a notification message will be received containing the same completed status."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["ids".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
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
    properties.insert("permissions".to_string(), create_permissions_schema());

    ToolSpec::Function(ResponsesApiTool {
        name: "request_permissions".to_string(),
        description: request_permissions_tool_description(),
        strict: false,
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
        "id".to_string(),
        JsonSchema::String {
            description: Some("Agent id to close (from spawn_agent).".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "close_agent".to_string(),
        description: "Close an agent when it is no longer needed and return its last known status. Don't keep agents open for too long if they are not needed anymore.".to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
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
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["pattern".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_search_tool_bm25_tool(app_tools: &HashMap<String, ToolInfo>) -> ToolSpec {
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
                    "Maximum number of tools to return (defaults to {SEARCH_TOOL_BM25_DEFAULT_LIMIT})."
                )),
            },
        ),
    ]);
    let mut app_names = app_tools
        .values()
        .filter_map(|tool| tool.connector_name.clone())
        .collect::<Vec<_>>();
    app_names.sort();
    app_names.dedup();
    let app_names = app_names.join(", ");

    let description = if app_names.is_empty() {
        SEARCH_TOOL_BM25_DESCRIPTION_TEMPLATE
            .replace("({{app_names}})", "(None currently enabled)")
            .replace("{{app_names}}", "available apps")
    } else {
        SEARCH_TOOL_BM25_DESCRIPTION_TEMPLATE.replace("{{app_names}}", app_names.as_str())
    };

    ToolSpec::Function(ResponsesApiTool {
        name: SEARCH_TOOL_BM25_TOOL_NAME.to_string(),
        description,
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["query".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
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

PRAGMA_LINE: /[ \t]*\/\/ codex-artifacts:[^\r\n]*/ | /[ \t]*\/\/ codex-artifact-tool:[^\r\n]*/
NEWLINE: /\r?\n/
PLAIN_JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: "artifacts".to_string(),
        description: "Runs raw JavaScript against the preinstalled Codex @oai/artifact-tool runtime for creating presentations or spreadsheets. This is plain JavaScript executed by a local Node-compatible runtime with top-level await, not TypeScript: do not use type annotations, `interface`, `type`, or `import type`. Author code the same way you would for `import { Presentation, Workbook, PresentationFile, SpreadsheetFile, FileBlob, ... } from \"@oai/artifact-tool\"`, but omit that import line because the package surface is already preloaded. Named exports are available directly on `globalThis`, and the full module is available as `globalThis.artifactTool` (also aliased as `globalThis.artifacts` and `globalThis.codexArtifacts`). Node built-ins such as `node:fs/promises` may still be imported when needed for saving preview bytes. This is a freeform tool: send raw JavaScript source text, optionally with a first-line pragma like `// codex-artifacts: timeout_ms=15000` or `// codex-artifact-tool: timeout_ms=15000`; do not send JSON/quotes/markdown fences."
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
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

fn create_code_mode_tool(enabled_tool_names: &[String]) -> ToolSpec {
    const CODE_MODE_FREEFORM_GRAMMAR: &str = r#"
start: source
source: /[\s\S]+/
"#;

    let enabled_list = if enabled_tool_names.is_empty() {
        "none".to_string()
    } else {
        enabled_tool_names.join(", ")
    };
    let description = format!(
        "Runs JavaScript in an embedded V8 runtime. This is a freeform tool: send raw JavaScript source text (no JSON/quotes/markdown fences). Direct tool calls remain available while `{PUBLIC_TOOL_NAME}` is enabled. Inside JavaScript, import nested tools from `tools.js`, for example `import {{ exec_command }} from \"tools.js\"`, `import {{ tools }} from \"tools.js\"`, or `import {{ ALL_TOOLS }} from \"tools.js\"` to inspect the available `{{ module, name, description }}` entries. Namespaced tools are also available from `tools/<namespace...>.js`; MCP tools use `tools/mcp/<server>.js`, for example `import {{ append_notebook_logs_chart }} from \"tools/mcp/ologs.js\"`. `tools[name]` and identifier wrappers like `await shell(args)` remain available for compatibility when the tool name is a valid JS identifier. Nested tool calls resolve to their code-mode result values. Import `{{ output_text, output_image, set_max_output_tokens_per_exec_call, store, load }}` from `\"@openai/code_mode\"` (or `\"openai/code_mode\"`); `output_text(value)` surfaces text back to the model and stringifies non-string objects when possible, `output_image(imageUrl)` appends an `input_image` content item for `http(s)` or `data:` URLs, `store(key, value)` persists JSON-serializable values across `{PUBLIC_TOOL_NAME}` calls in the current session, `load(key)` returns a cloned stored value or `undefined`, and `set_max_output_tokens_per_exec_call(value)` sets the token budget used to truncate the final Rust-side result of the current `{PUBLIC_TOOL_NAME}` execution. The default is `10000`. This guards the overall `{PUBLIC_TOOL_NAME}` output, not individual nested tool invocations. The returned content starts with a separate `Script completed` or `Script failed` text item that includes wall time. When truncation happens, the final text may include `Total output lines:` and the usual `…N tokens truncated…` marker. Function tools require JSON object arguments. Freeform tools require raw strings. `add_content(value)` remains available for compatibility with a content item, content-item array, or string. Structured nested-tool results should be converted to text first, for example with `JSON.stringify(...)`. Only content passed to `output_text(...)`, `output_image(...)`, or `add_content(value)` is surfaced back to the model. Enabled nested tools: {enabled_list}."
    );

    ToolSpec::Freeform(FreeformTool {
        name: PUBLIC_TOOL_NAME.to_string(),
        description,
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: CODE_MODE_FREEFORM_GRAMMAR.to_string(),
        },
    })
}

fn is_code_mode_nested_tool(spec: &ToolSpec) -> bool {
    spec.name() != PUBLIC_TOOL_NAME && matches!(spec, ToolSpec::Function(_) | ToolSpec::Freeform(_))
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
        builder.push_spec_with_parallel_support(spec, true);
    } else {
        builder.push_spec(spec);
    }
}

pub(crate) fn mcp_tool_to_openai_tool(
    fully_qualified_name: String,
    tool: rmcp::model::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
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

    Ok(ResponsesApiTool {
        name: fully_qualified_name,
        description: description.map(Into::into).unwrap_or_default(),
        strict: false,
        parameters: input_schema,
        output_schema,
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
pub(crate) fn build_specs(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::ArtifactsHandler;
    use crate::tools::handlers::CodeModeHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::GrepFilesHandler;
    use crate::tools::handlers::JsReplHandler;
    use crate::tools::handlers::JsReplResetHandler;
    use crate::tools::handlers::ListDirHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::McpResourceHandler;
    use crate::tools::handlers::MultiAgentHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::ReadFileHandler;
    use crate::tools::handlers::RequestPermissionsHandler;
    use crate::tools::handlers::RequestUserInputHandler;
    use crate::tools::handlers::SearchToolBm25Handler;
    use crate::tools::handlers::ShellCommandHandler;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::handlers::TestSyncHandler;
    use crate::tools::handlers::UnifiedExecHandler;
    use crate::tools::handlers::ViewImageHandler;
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
    let search_tool_handler = Arc::new(SearchToolBm25Handler);
    let code_mode_handler = Arc::new(CodeModeHandler);
    let js_repl_handler = Arc::new(JsReplHandler);
    let js_repl_reset_handler = Arc::new(JsReplResetHandler);
    let artifacts_handler = Arc::new(ArtifactsHandler);
    let request_permission_enabled = config.request_permission_enabled;

    if config.code_mode_enabled {
        let nested_config = config.for_code_mode_nested_tools();
        let (nested_specs, _) = build_specs(
            &nested_config,
            mcp_tools.clone(),
            app_tools.clone(),
            dynamic_tools,
        )
        .build();
        let mut enabled_tool_names = nested_specs
            .into_iter()
            .map(|spec| spec.spec)
            .filter(is_code_mode_nested_tool)
            .map(|spec| spec.name().to_string())
            .collect::<Vec<_>>();
        enabled_tool_names.sort();
        enabled_tool_names.dedup();
        push_tool_spec(
            &mut builder,
            create_code_mode_tool(&enabled_tool_names),
            false,
            config.code_mode_enabled,
        );
        builder.register_handler(PUBLIC_TOOL_NAME, code_mode_handler);
    }

    match &config.shell_type {
        ConfigShellToolType::Default => {
            push_tool_spec(
                &mut builder,
                create_shell_tool(request_permission_enabled),
                true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::Local => {
            push_tool_spec(
                &mut builder,
                ToolSpec::LocalShell {},
                true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::UnifiedExec => {
            push_tool_spec(
                &mut builder,
                create_exec_command_tool(config.allow_login_shell, request_permission_enabled),
                true,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_write_stdin_tool(),
                false,
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
                create_shell_command_tool(config.allow_login_shell, request_permission_enabled),
                true,
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
            true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_list_mcp_resource_templates_tool(),
            true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_read_mcp_resource_tool(),
            true,
            config.code_mode_enabled,
        );
        builder.register_handler("list_mcp_resources", mcp_resource_handler.clone());
        builder.register_handler("list_mcp_resource_templates", mcp_resource_handler.clone());
        builder.register_handler("read_mcp_resource", mcp_resource_handler);
    }

    push_tool_spec(
        &mut builder,
        PLAN_TOOL.clone(),
        false,
        config.code_mode_enabled,
    );
    builder.register_handler("update_plan", plan_handler);

    if config.js_repl_enabled {
        push_tool_spec(
            &mut builder,
            create_js_repl_tool(),
            false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_js_repl_reset_tool(),
            false,
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
            false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_user_input", request_user_input_handler);
    }

    if config.request_permissions_tool_enabled {
        push_tool_spec(
            &mut builder,
            create_request_permissions_tool(),
            false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_permissions", request_permissions_handler);
    }

    if config.search_tool {
        let app_tools = app_tools.unwrap_or_default();
        push_tool_spec(
            &mut builder,
            create_search_tool_bm25_tool(&app_tools),
            true,
            config.code_mode_enabled,
        );
        builder.register_handler(SEARCH_TOOL_BM25_TOOL_NAME, search_tool_handler);
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_freeform_tool(),
                    false,
                    config.code_mode_enabled,
                );
            }
            ApplyPatchToolType::Function => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_json_tool(),
                    false,
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
            true,
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
            true,
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
            true,
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
            true,
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
            false,
            config.code_mode_enabled,
        );
    }

    if config.image_gen_tool {
        push_tool_spec(
            &mut builder,
            ToolSpec::ImageGeneration {
                output_format: "png".to_string(),
            },
            false,
            config.code_mode_enabled,
        );
    }

    push_tool_spec(
        &mut builder,
        create_view_image_tool(),
        true,
        config.code_mode_enabled,
    );
    builder.register_handler("view_image", view_image_handler);

    if config.artifact_tools {
        push_tool_spec(
            &mut builder,
            create_artifacts_tool(),
            false,
            config.code_mode_enabled,
        );
        builder.register_handler("artifacts", artifacts_handler);
    }

    if config.collab_tools {
        let multi_agent_handler = Arc::new(MultiAgentHandler);
        push_tool_spec(
            &mut builder,
            create_spawn_agent_tool(config),
            false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_send_input_tool(),
            false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_resume_agent_tool(),
            false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_wait_tool(),
            false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_close_agent_tool(),
            false,
            config.code_mode_enabled,
        );
        builder.register_handler("spawn_agent", multi_agent_handler.clone());
        builder.register_handler("send_input", multi_agent_handler.clone());
        builder.register_handler("resume_agent", multi_agent_handler.clone());
        builder.register_handler("wait", multi_agent_handler.clone());
        builder.register_handler("close_agent", multi_agent_handler);
    }

    if config.agent_jobs_tools {
        let agent_jobs_handler = Arc::new(BatchJobHandler);
        push_tool_spec(
            &mut builder,
            create_spawn_agents_on_csv_tool(),
            false,
            config.code_mode_enabled,
        );
        builder.register_handler("spawn_agents_on_csv", agent_jobs_handler.clone());
        if config.agent_jobs_worker_tools {
            push_tool_spec(
                &mut builder,
                create_report_agent_job_result_tool(),
                false,
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
                        false,
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
                        false,
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
mod tests {
    use crate::client_common::tools::FreeformTool;
    use crate::config::test_config;
    use crate::models_manager::manager::ModelsManager;
    use crate::models_manager::model_info::with_config_overrides;
    use crate::tools::registry::ConfiguredToolSpec;
    use codex_protocol::openai_models::InputModality;
    use codex_protocol::openai_models::ModelInfo;
    use codex_protocol::openai_models::ModelsResponse;
    use pretty_assertions::assert_eq;

    use super::*;

    fn mcp_tool(
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
    ) -> rmcp::model::Tool {
        rmcp::model::Tool {
            name: name.to_string().into(),
            title: None,
            description: Some(description.to_string().into()),
            input_schema: std::sync::Arc::new(rmcp::model::object(input_schema)),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        }
    }

    #[test]
    fn mcp_tool_to_openai_tool_inserts_empty_properties() {
        let mut schema = rmcp::model::JsonObject::new();
        schema.insert("type".to_string(), serde_json::json!("object"));

        let tool = rmcp::model::Tool {
            name: "no_props".to_string().into(),
            title: None,
            description: Some("No properties".to_string().into()),
            input_schema: std::sync::Arc::new(schema),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        };

        let openai_tool =
            mcp_tool_to_openai_tool("server/no_props".to_string(), tool).expect("convert tool");
        let parameters = serde_json::to_value(openai_tool.parameters).expect("serialize schema");

        assert_eq!(parameters.get("properties"), Some(&serde_json::json!({})));
    }

    #[test]
    fn mcp_tool_to_openai_tool_preserves_top_level_output_schema() {
        let mut input_schema = rmcp::model::JsonObject::new();
        input_schema.insert("type".to_string(), serde_json::json!("object"));

        let mut output_schema = rmcp::model::JsonObject::new();
        output_schema.insert(
            "properties".to_string(),
            serde_json::json!({
                "result": {
                    "properties": {
                        "nested": {}
                    }
                }
            }),
        );
        output_schema.insert("required".to_string(), serde_json::json!(["result"]));

        let tool = rmcp::model::Tool {
            name: "with_output".to_string().into(),
            title: None,
            description: Some("Has output schema".to_string().into()),
            input_schema: std::sync::Arc::new(input_schema),
            output_schema: Some(std::sync::Arc::new(output_schema)),
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        };

        let openai_tool = mcp_tool_to_openai_tool("mcp__server__with_output".to_string(), tool)
            .expect("convert tool");

        assert_eq!(
            openai_tool.output_schema,
            Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "array",
                        "items": {}
                    },
                    "structuredContent": {
                        "properties": {
                            "result": {
                                "properties": {
                                    "nested": {}
                                }
                            }
                        },
                        "required": ["result"]
                    },
                    "isError": {
                        "type": "boolean"
                    },
                    "_meta": {}
                },
                "required": ["content"],
                "additionalProperties": false
            }))
        );
    }

    #[test]
    fn mcp_tool_to_openai_tool_preserves_output_schema_without_inferred_type() {
        let mut input_schema = rmcp::model::JsonObject::new();
        input_schema.insert("type".to_string(), serde_json::json!("object"));

        let mut output_schema = rmcp::model::JsonObject::new();
        output_schema.insert("enum".to_string(), serde_json::json!(["ok", "error"]));

        let tool = rmcp::model::Tool {
            name: "with_enum_output".to_string().into(),
            title: None,
            description: Some("Has enum output schema".to_string().into()),
            input_schema: std::sync::Arc::new(input_schema),
            output_schema: Some(std::sync::Arc::new(output_schema)),
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        };

        let openai_tool =
            mcp_tool_to_openai_tool("mcp__server__with_enum_output".to_string(), tool)
                .expect("convert tool");

        assert_eq!(
            openai_tool.output_schema,
            Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "array",
                        "items": {}
                    },
                    "structuredContent": {
                        "enum": ["ok", "error"]
                    },
                    "isError": {
                        "type": "boolean"
                    },
                    "_meta": {}
                },
                "required": ["content"],
                "additionalProperties": false
            }))
        );
    }

    fn tool_name(tool: &ToolSpec) -> &str {
        match tool {
            ToolSpec::Function(ResponsesApiTool { name, .. }) => name,
            ToolSpec::LocalShell {} => "local_shell",
            ToolSpec::ImageGeneration { .. } => "image_generation",
            ToolSpec::WebSearch { .. } => "web_search",
            ToolSpec::Freeform(FreeformTool { name, .. }) => name,
        }
    }

    // Avoid order-based assertions; compare via set containment instead.
    fn assert_contains_tool_names(tools: &[ConfiguredToolSpec], expected_subset: &[&str]) {
        use std::collections::HashSet;
        let mut names = HashSet::new();
        let mut duplicates = Vec::new();
        for name in tools.iter().map(|t| tool_name(&t.spec)) {
            if !names.insert(name) {
                duplicates.push(name);
            }
        }
        assert!(
            duplicates.is_empty(),
            "duplicate tool entries detected: {duplicates:?}"
        );
        for expected in expected_subset {
            assert!(
                names.contains(expected),
                "expected tool {expected} to be present; had: {names:?}"
            );
        }
    }

    fn assert_lacks_tool_name(tools: &[ConfiguredToolSpec], expected_absent: &str) {
        let names = tools
            .iter()
            .map(|tool| tool_name(&tool.spec))
            .collect::<Vec<_>>();
        assert!(
            !names.contains(&expected_absent),
            "expected tool {expected_absent} to be absent; had: {names:?}"
        );
    }

    fn shell_tool_name(config: &ToolsConfig) -> Option<&'static str> {
        match config.shell_type {
            ConfigShellToolType::Default => Some("shell"),
            ConfigShellToolType::Local => Some("local_shell"),
            ConfigShellToolType::UnifiedExec => None,
            ConfigShellToolType::Disabled => None,
            ConfigShellToolType::ShellCommand => Some("shell_command"),
        }
    }

    fn find_tool<'a>(
        tools: &'a [ConfiguredToolSpec],
        expected_name: &str,
    ) -> &'a ConfiguredToolSpec {
        tools
            .iter()
            .find(|tool| tool_name(&tool.spec) == expected_name)
            .unwrap_or_else(|| panic!("expected tool {expected_name}"))
    }

    fn strip_descriptions_schema(schema: &mut JsonSchema) {
        match schema {
            JsonSchema::Boolean { description }
            | JsonSchema::String { description }
            | JsonSchema::Number { description } => {
                *description = None;
            }
            JsonSchema::Array { items, description } => {
                strip_descriptions_schema(items);
                *description = None;
            }
            JsonSchema::Object {
                properties,
                required: _,
                additional_properties,
            } => {
                for v in properties.values_mut() {
                    strip_descriptions_schema(v);
                }
                if let Some(AdditionalProperties::Schema(s)) = additional_properties {
                    strip_descriptions_schema(s);
                }
            }
        }
    }

    fn strip_descriptions_tool(spec: &mut ToolSpec) {
        match spec {
            ToolSpec::Function(ResponsesApiTool { parameters, .. }) => {
                strip_descriptions_schema(parameters);
            }
            ToolSpec::Freeform(_)
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. } => {}
        }
    }

    fn model_info_from_models_json(slug: &str) -> ModelInfo {
        let config = test_config();
        let response: ModelsResponse =
            serde_json::from_str(include_str!("../../models.json")).expect("valid models.json");
        let model = response
            .models
            .into_iter()
            .find(|candidate| candidate.slug == slug)
            .unwrap_or_else(|| panic!("model slug {slug} is missing from models.json"));
        with_config_overrides(model, &config)
    }

    #[test]
    fn test_full_toolset_specs_for_gpt5_codex_unified_exec_web_search() {
        let model_info = model_info_from_models_json("gpt-5-codex");
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&config, None, None, &[]).build();

        // Build actual map name -> spec
        use std::collections::BTreeMap;
        use std::collections::HashSet;
        let mut actual: BTreeMap<String, ToolSpec> = BTreeMap::from([]);
        let mut duplicate_names = Vec::new();
        for t in &tools {
            let name = tool_name(&t.spec).to_string();
            if actual.insert(name.clone(), t.spec.clone()).is_some() {
                duplicate_names.push(name);
            }
        }
        assert!(
            duplicate_names.is_empty(),
            "duplicate tool entries detected: {duplicate_names:?}"
        );

        // Build expected from the same helpers used by the builder.
        let mut expected: BTreeMap<String, ToolSpec> = BTreeMap::from([]);
        for spec in [
            create_exec_command_tool(true, false),
            create_write_stdin_tool(),
            PLAN_TOOL.clone(),
            create_request_user_input_tool(CollaborationModesConfig::default()),
            create_apply_patch_freeform_tool(),
            ToolSpec::WebSearch {
                external_web_access: Some(true),
                filters: None,
                user_location: None,
                search_context_size: None,
                search_content_types: None,
            },
            create_view_image_tool(),
        ] {
            expected.insert(tool_name(&spec).to_string(), spec);
        }

        if config.request_permission_enabled {
            let spec = create_request_permissions_tool();
            expected.insert(tool_name(&spec).to_string(), spec);
        }

        // Exact name set match — this is the only test allowed to fail when tools change.
        let actual_names: HashSet<_> = actual.keys().cloned().collect();
        let expected_names: HashSet<_> = expected.keys().cloned().collect();
        assert_eq!(actual_names, expected_names, "tool name set mismatch");

        // Compare specs ignoring human-readable descriptions.
        for name in expected.keys() {
            let mut a = actual.get(name).expect("present").clone();
            let mut e = expected.get(name).expect("present").clone();
            strip_descriptions_tool(&mut a);
            strip_descriptions_tool(&mut e);
            assert_eq!(a, e, "spec mismatch for {name}");
        }
    }

    #[test]
    fn test_build_specs_collab_tools_enabled() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::Collab);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_contains_tool_names(
            &tools,
            &["spawn_agent", "send_input", "wait", "close_agent"],
        );
        assert_lacks_tool_name(&tools, "spawn_agents_on_csv");
    }

    #[test]
    fn test_build_specs_spawn_csv_enables_agent_jobs_and_collab_tools() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::SpawnCsv);
        features.normalize_dependencies();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_contains_tool_names(
            &tools,
            &[
                "spawn_agent",
                "send_input",
                "wait",
                "close_agent",
                "spawn_agents_on_csv",
            ],
        );
    }

    #[test]
    fn test_build_specs_artifact_tool_enabled() {
        let mut config = test_config();
        let runtime_root = tempfile::TempDir::new().expect("create temp codex home");
        config.codex_home = runtime_root.path().to_path_buf();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::Artifact);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_contains_tool_names(&tools, &["artifacts"]);
    }

    #[test]
    fn test_build_specs_agent_job_worker_tools_enabled() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::SpawnCsv);
        features.normalize_dependencies();
        features.enable(Feature::Sqlite);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::SubAgent(SubAgentSource::Other(
                "agent_job:test".to_string(),
            )),
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_contains_tool_names(
            &tools,
            &[
                "spawn_agent",
                "send_input",
                "resume_agent",
                "wait",
                "close_agent",
                "spawn_agents_on_csv",
                "report_agent_job_result",
            ],
        );
        assert_lacks_tool_name(&tools, "request_user_input");
    }

    #[test]
    fn request_user_input_description_reflects_default_mode_feature_flag() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        let request_user_input_tool = find_tool(&tools, "request_user_input");
        assert_eq!(
            request_user_input_tool.spec,
            create_request_user_input_tool(CollaborationModesConfig::default())
        );

        features.enable(Feature::DefaultModeRequestUserInput);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        let request_user_input_tool = find_tool(&tools, "request_user_input");
        assert_eq!(
            request_user_input_tool.spec,
            create_request_user_input_tool(CollaborationModesConfig {
                default_mode_request_user_input: true,
            })
        );
    }

    #[test]
    fn request_permissions_requires_feature_flag() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_lacks_tool_name(&tools, "request_permissions");

        let mut features = Features::with_defaults();
        features.enable(Feature::RequestPermissionsTool);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        let request_permissions_tool = find_tool(&tools, "request_permissions");
        assert_eq!(
            request_permissions_tool.spec,
            create_request_permissions_tool()
        );
    }

    #[test]
    fn request_permissions_tool_is_independent_from_additional_permissions() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::RequestPermissions);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        assert_lacks_tool_name(&tools, "request_permissions");
    }

    #[test]
    fn get_memory_requires_feature_flag() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.disable(Feature::MemoryTool);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert!(
            !tools.iter().any(|t| t.spec.name() == "get_memory"),
            "get_memory should be disabled when memory_tool feature is off"
        );
    }

    #[test]
    fn js_repl_requires_feature_flag() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        assert!(
            !tools.iter().any(|tool| tool.spec.name() == "js_repl"),
            "js_repl should be disabled when the feature is off"
        );
        assert!(
            !tools.iter().any(|tool| tool.spec.name() == "js_repl_reset"),
            "js_repl_reset should be disabled when the feature is off"
        );
    }

    #[test]
    fn js_repl_enabled_adds_tools() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::JsRepl);

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert_contains_tool_names(&tools, &["js_repl", "js_repl_reset"]);
    }

    #[test]
    fn image_generation_tools_require_feature_and_supported_model() {
        let config = test_config();
        let mut supported_model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5.2", &config);
        supported_model_info.slug = "custom/gpt-5.2-variant".to_string();
        let mut unsupported_model_info = supported_model_info.clone();
        unsupported_model_info.input_modalities = vec![InputModality::Text];
        let default_features = Features::with_defaults();
        let mut image_generation_features = default_features.clone();
        image_generation_features.enable(Feature::ImageGeneration);

        let available_models = Vec::new();
        let default_tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &supported_model_info,
            available_models: &available_models,
            features: &default_features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (default_tools, _) = build_specs(&default_tools_config, None, None, &[]).build();
        assert!(
            !default_tools
                .iter()
                .any(|tool| tool.spec.name() == "image_generation"),
            "image_generation should be disabled by default"
        );

        let supported_tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &supported_model_info,
            available_models: &available_models,
            features: &image_generation_features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (supported_tools, _) = build_specs(&supported_tools_config, None, None, &[]).build();
        assert_contains_tool_names(&supported_tools, &["image_generation"]);
        let image_generation_tool = find_tool(&supported_tools, "image_generation");
        assert_eq!(
            serde_json::to_value(&image_generation_tool.spec).expect("serialize image tool"),
            serde_json::json!({
                "type": "image_generation",
                "output_format": "png"
            })
        );

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &unsupported_model_info,
            available_models: &available_models,
            features: &image_generation_features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        assert!(
            !tools
                .iter()
                .any(|tool| tool.spec.name() == "image_generation"),
            "image_generation should be disabled for unsupported models"
        );
    }

    #[test]
    fn js_repl_freeform_grammar_blocks_common_non_js_prefixes() {
        let ToolSpec::Freeform(FreeformTool { format, .. }) = create_js_repl_tool() else {
            panic!("js_repl should use a freeform tool spec");
        };

        assert_eq!(format.syntax, "lark");
        assert!(format.definition.contains("PRAGMA_LINE"));
        assert!(format.definition.contains("`[^`]"));
        assert!(format.definition.contains("``[^`]"));
        assert!(format.definition.contains("PLAIN_JS_SOURCE"));
        assert!(format.definition.contains("codex-js-repl:"));
        assert!(!format.definition.contains("(?!"));
    }

    fn assert_model_tools(
        model_slug: &str,
        features: &Features,
        web_search_mode: Option<WebSearchMode>,
        expected_tools: &[&str],
    ) {
        let _config = test_config();
        let model_info = model_info_from_models_json(model_slug);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features,
            web_search_mode,
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        let tool_names = tools.iter().map(|t| t.spec.name()).collect::<Vec<_>>();
        assert_eq!(&tool_names, &expected_tools,);
    }

    fn assert_default_model_tools(
        model_slug: &str,
        features: &Features,
        web_search_mode: Option<WebSearchMode>,
        shell_tool: &'static str,
        expected_tail: &[&str],
    ) {
        let mut expected = if features.enabled(Feature::UnifiedExec) {
            vec!["exec_command", "write_stdin"]
        } else {
            vec![shell_tool]
        };
        expected.extend(expected_tail);
        assert_model_tools(model_slug, features, web_search_mode, &expected);
    }

    #[test]
    fn web_search_mode_cached_sets_external_web_access_false() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(false),
                filters: None,
                user_location: None,
                search_context_size: None,
                search_content_types: None,
            }
        );
    }

    #[test]
    fn web_search_mode_live_sets_external_web_access_true() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(true),
                filters: None,
                user_location: None,
                search_context_size: None,
                search_content_types: None,
            }
        );
    }

    #[test]
    fn web_search_config_is_forwarded_to_tool_spec() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let web_search_config = WebSearchConfig {
            filters: Some(codex_protocol::config_types::WebSearchFilters {
                allowed_domains: Some(vec!["example.com".to_string()]),
            }),
            user_location: Some(codex_protocol::config_types::WebSearchUserLocation {
                r#type: codex_protocol::config_types::WebSearchUserLocationType::Approximate,
                country: Some("US".to_string()),
                region: Some("California".to_string()),
                city: Some("San Francisco".to_string()),
                timezone: Some("America/Los_Angeles".to_string()),
            }),
            search_context_size: Some(codex_protocol::config_types::WebSearchContextSize::High),
        };

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        })
        .with_web_search_config(Some(web_search_config.clone()));
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(true),
                filters: web_search_config
                    .filters
                    .map(crate::client_common::tools::ResponsesApiWebSearchFilters::from),
                user_location: web_search_config
                    .user_location
                    .map(crate::client_common::tools::ResponsesApiWebSearchUserLocation::from),
                search_context_size: web_search_config.search_context_size,
                search_content_types: None,
            }
        );
    }

    #[test]
    fn web_search_tool_type_text_and_image_sets_search_content_types() {
        let config = test_config();
        let mut model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        model_info.web_search_tool_type = WebSearchToolType::TextAndImage;
        let features = Features::with_defaults();

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(true),
                filters: None,
                user_location: None,
                search_context_size: None,
                search_content_types: Some(
                    WEB_SEARCH_CONTENT_TYPES
                        .into_iter()
                        .map(str::to_string)
                        .collect()
                ),
            }
        );
    }

    #[test]
    fn mcp_resource_tools_are_hidden_without_mcp_servers() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        assert!(
            !tools.iter().any(|tool| matches!(
                tool.spec.name(),
                "list_mcp_resources" | "list_mcp_resource_templates" | "read_mcp_resource"
            )),
            "MCP resource tools should be omitted when no MCP servers are configured"
        );
    }

    #[test]
    fn mcp_resource_tools_are_included_when_mcp_servers_are_present() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, Some(HashMap::new()), None, &[]).build();

        assert_contains_tool_names(
            &tools,
            &[
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
            ],
        );
    }

    #[test]
    fn test_build_specs_gpt5_codex_default() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5-codex",
            &features,
            Some(WebSearchMode::Cached),
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_build_specs_gpt51_codex_default() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5.1-codex",
            &features,
            Some(WebSearchMode::Cached),
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_build_specs_gpt5_codex_unified_exec_web_search() {
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        assert_model_tools(
            "gpt-5-codex",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_build_specs_gpt51_codex_unified_exec_web_search() {
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        assert_model_tools(
            "gpt-5.1-codex",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_gpt_5_1_codex_max_defaults() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5.1-codex-max",
            &features,
            Some(WebSearchMode::Cached),
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_codex_5_1_mini_defaults() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5.1-codex-mini",
            &features,
            Some(WebSearchMode::Cached),
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_gpt_5_defaults() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5",
            &features,
            Some(WebSearchMode::Cached),
            "shell",
            &[
                "update_plan",
                "request_user_input",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_gpt_5_1_defaults() {
        let features = Features::with_defaults();
        assert_default_model_tools(
            "gpt-5.1",
            &features,
            Some(WebSearchMode::Cached),
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_gpt_5_1_codex_max_unified_exec_web_search() {
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        assert_model_tools(
            "gpt-5.1-codex-max",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_build_specs_default_shell_present() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline_for_tests("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, Some(HashMap::new()), None, &[]).build();

        // Only check the shell variant and a couple of core tools.
        let mut subset = vec!["exec_command", "write_stdin", "update_plan"];
        if let Some(shell_tool) = shell_tool_name(&tools_config) {
            subset.push(shell_tool);
        }
        assert_contains_tool_names(&tools, &subset);
    }

    #[test]
    fn shell_zsh_fork_prefers_shell_command_over_unified_exec() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline_for_tests("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        features.enable(Feature::ShellZshFork);

        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });

        assert_eq!(tools_config.shell_type, ConfigShellToolType::ShellCommand);
        assert_eq!(
            tools_config.shell_command_backend,
            ShellCommandBackendConfig::ZshFork
        );
        assert_eq!(
            tools_config.unified_exec_backend,
            UnifiedExecBackendConfig::ZshFork
        );
    }

    #[test]
    #[ignore]
    fn test_parallel_support_flags() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        assert!(find_tool(&tools, "exec_command").supports_parallel_tool_calls);
        assert!(!find_tool(&tools, "write_stdin").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "grep_files").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "list_dir").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "read_file").supports_parallel_tool_calls);
    }

    #[test]
    fn test_test_model_info_includes_sync_tool() {
        let _config = test_config();
        let mut model_info = model_info_from_models_json("gpt-5-codex");
        model_info.experimental_supported_tools = vec![
            "test_sync_tool".to_string(),
            "read_file".to_string(),
            "grep_files".to_string(),
            "list_dir".to_string(),
        ];
        let features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();

        assert!(
            tools
                .iter()
                .any(|tool| tool_name(&tool.spec) == "test_sync_tool")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool_name(&tool.spec) == "read_file")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool_name(&tool.spec) == "grep_files")
        );
        assert!(tools.iter().any(|tool| tool_name(&tool.spec) == "list_dir"));
    }

    #[test]
    fn test_build_specs_mcp_tools_converted() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline_for_tests("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "test_server/do_something_cool".to_string(),
                mcp_tool(
                    "do_something_cool",
                    "Do something cool",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "string_argument": { "type": "string" },
                            "number_argument": { "type": "number" },
                            "object_argument": {
                                "type": "object",
                                "properties": {
                                    "string_property": { "type": "string" },
                                    "number_property": { "type": "number" },
                                },
                                "required": ["string_property", "number_property"],
                                "additionalProperties": false,
                            },
                        },
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "test_server/do_something_cool");
        assert_eq!(
            &tool.spec,
            &ToolSpec::Function(ResponsesApiTool {
                name: "test_server/do_something_cool".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([
                        (
                            "string_argument".to_string(),
                            JsonSchema::String { description: None }
                        ),
                        (
                            "number_argument".to_string(),
                            JsonSchema::Number { description: None }
                        ),
                        (
                            "object_argument".to_string(),
                            JsonSchema::Object {
                                properties: BTreeMap::from([
                                    (
                                        "string_property".to_string(),
                                        JsonSchema::String { description: None }
                                    ),
                                    (
                                        "number_property".to_string(),
                                        JsonSchema::Number { description: None }
                                    ),
                                ]),
                                required: Some(vec![
                                    "string_property".to_string(),
                                    "number_property".to_string(),
                                ]),
                                additional_properties: Some(false.into()),
                            },
                        ),
                    ]),
                    required: None,
                    additional_properties: None,
                },
                description: "Do something cool".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn test_build_specs_mcp_tools_sorted_by_name() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline_for_tests("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        // Intentionally construct a map with keys that would sort alphabetically.
        let tools_map: HashMap<String, rmcp::model::Tool> = HashMap::from([
            (
                "test_server/do".to_string(),
                mcp_tool("a", "a", serde_json::json!({"type": "object"})),
            ),
            (
                "test_server/something".to_string(),
                mcp_tool("b", "b", serde_json::json!({"type": "object"})),
            ),
            (
                "test_server/cool".to_string(),
                mcp_tool("c", "c", serde_json::json!({"type": "object"})),
            ),
        ]);

        let (tools, _) = build_specs(&tools_config, Some(tools_map), None, &[]).build();

        // Only assert that the MCP tools themselves are sorted by fully-qualified name.
        let mcp_names: Vec<_> = tools
            .iter()
            .map(|t| tool_name(&t.spec).to_string())
            .filter(|n| n.starts_with("test_server/"))
            .collect();
        let expected = vec![
            "test_server/cool".to_string(),
            "test_server/do".to_string(),
            "test_server/something".to_string(),
        ];
        assert_eq!(mcp_names, expected);
    }

    #[test]
    fn search_tool_description_includes_only_codex_apps_connector_names() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::Apps);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([
                (
                    "mcp__codex_apps__calendar_create_event".to_string(),
                    mcp_tool(
                        "calendar_create_event",
                        "Create calendar event",
                        serde_json::json!({"type": "object"}),
                    ),
                ),
                (
                    "mcp__rmcp__echo".to_string(),
                    mcp_tool("echo", "Echo", serde_json::json!({"type": "object"})),
                ),
            ])),
            Some(HashMap::from([
                (
                    "mcp__codex_apps__calendar_create_event".to_string(),
                    ToolInfo {
                        server_name: crate::mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
                        tool_name: "calendar_create_event".to_string(),
                        tool: mcp_tool(
                            "calendar_create_event",
                            "Create calendar event",
                            serde_json::json!({"type": "object"}),
                        ),
                        connector_id: Some("calendar".to_string()),
                        connector_name: Some("Calendar".to_string()),
                        plugin_display_names: Vec::new(),
                    },
                ),
                (
                    "mcp__rmcp__echo".to_string(),
                    ToolInfo {
                        server_name: "rmcp".to_string(),
                        tool_name: "echo".to_string(),
                        tool: mcp_tool("echo", "Echo", serde_json::json!({"type": "object"})),
                        connector_id: None,
                        connector_name: None,
                        plugin_display_names: Vec::new(),
                    },
                ),
            ])),
            &[],
        )
        .build();

        let search_tool = find_tool(&tools, SEARCH_TOOL_BM25_TOOL_NAME);
        let ToolSpec::Function(ResponsesApiTool { description, .. }) = &search_tool.spec else {
            panic!("expected function tool");
        };
        assert!(description.contains("Calendar"));
        assert!(!description.contains("mcp__rmcp__echo"));
    }

    #[test]
    fn search_tool_requires_apps_feature_flag_only() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let app_tools = Some(HashMap::from([(
            "mcp__codex_apps__calendar_create_event".to_string(),
            ToolInfo {
                server_name: crate::mcp::CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "calendar_create_event".to_string(),
                tool: mcp_tool(
                    "calendar_create_event",
                    "Create calendar event",
                    serde_json::json!({"type": "object"}),
                ),
                connector_id: Some("calendar".to_string()),
                connector_name: Some("Calendar".to_string()),
                plugin_display_names: Vec::new(),
            },
        )]));

        let features = Features::with_defaults();
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, app_tools.clone(), &[]).build();
        assert_lacks_tool_name(&tools, SEARCH_TOOL_BM25_TOOL_NAME);

        let mut features = Features::with_defaults();
        features.enable(Feature::Apps);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(&tools_config, None, app_tools, &[]).build();
        assert_contains_tool_names(&tools, &[SEARCH_TOOL_BM25_TOOL_NAME]);
    }

    #[test]
    fn search_tool_description_handles_no_enabled_apps() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::Apps);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(&tools_config, None, Some(HashMap::new()), &[]).build();
        let search_tool = find_tool(&tools, SEARCH_TOOL_BM25_TOOL_NAME);
        let ToolSpec::Function(ResponsesApiTool { description, .. }) = &search_tool.spec else {
            panic!("expected function tool");
        };

        assert!(description.contains("(None currently enabled)"));
        assert!(description.contains("available apps."));
        assert!(!description.contains("{{app_names}}"));
    }

    #[test]
    fn test_mcp_tool_property_missing_type_defaults_to_string() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/search".to_string(),
                mcp_tool(
                    "search",
                    "Search docs",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": {"description": "search query"}
                        }
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "dash/search");
        assert_eq!(
            tool.spec,
            ToolSpec::Function(ResponsesApiTool {
                name: "dash/search".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([(
                        "query".to_string(),
                        JsonSchema::String {
                            description: Some("search query".to_string())
                        }
                    )]),
                    required: None,
                    additional_properties: None,
                },
                description: "Search docs".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn test_mcp_tool_integer_normalized_to_number() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/paginate".to_string(),
                mcp_tool(
                    "paginate",
                    "Pagination",
                    serde_json::json!({
                        "type": "object",
                        "properties": {"page": {"type": "integer"}}
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "dash/paginate");
        assert_eq!(
            tool.spec,
            ToolSpec::Function(ResponsesApiTool {
                name: "dash/paginate".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([(
                        "page".to_string(),
                        JsonSchema::Number { description: None }
                    )]),
                    required: None,
                    additional_properties: None,
                },
                description: "Pagination".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn test_mcp_tool_array_without_items_gets_default_string_items() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        features.enable(Feature::ApplyPatchFreeform);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/tags".to_string(),
                mcp_tool(
                    "tags",
                    "Tags",
                    serde_json::json!({
                        "type": "object",
                        "properties": {"tags": {"type": "array"}}
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "dash/tags");
        assert_eq!(
            tool.spec,
            ToolSpec::Function(ResponsesApiTool {
                name: "dash/tags".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([(
                        "tags".to_string(),
                        JsonSchema::Array {
                            items: Box::new(JsonSchema::String { description: None }),
                            description: None
                        }
                    )]),
                    required: None,
                    additional_properties: None,
                },
                description: "Tags".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn test_mcp_tool_anyof_defaults_to_string() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/value".to_string(),
                mcp_tool(
                    "value",
                    "AnyOf Value",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "value": {"anyOf": [{"type": "string"}, {"type": "number"}]}
                        }
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "dash/value");
        assert_eq!(
            tool.spec,
            ToolSpec::Function(ResponsesApiTool {
                name: "dash/value".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([(
                        "value".to_string(),
                        JsonSchema::String { description: None }
                    )]),
                    required: None,
                    additional_properties: None,
                },
                description: "AnyOf Value".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn test_shell_tool() {
        let tool = super::create_shell_tool(false);
        let ToolSpec::Function(ResponsesApiTool {
            description, name, ..
        }) = &tool
        else {
            panic!("expected function tool");
        };
        assert_eq!(name, "shell");

        let expected = if cfg!(windows) {
            r#"Runs a Powershell command (Windows) and returns its output. Arguments to `shell` will be passed to CreateProcessW(). Most commands should be prefixed with ["powershell.exe", "-Command"].

Examples of valid command strings:

- ls -a (show hidden): ["powershell.exe", "-Command", "Get-ChildItem -Force"]
- recursive find by name: ["powershell.exe", "-Command", "Get-ChildItem -Recurse -Filter *.py"]
- recursive grep: ["powershell.exe", "-Command", "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"]
- ps aux | grep python: ["powershell.exe", "-Command", "Get-Process | Where-Object { $_.ProcessName -like '*python*' }"]
- setting an env var: ["powershell.exe", "-Command", "$env:FOO='bar'; echo $env:FOO"]
- running an inline Python script: ["powershell.exe", "-Command", "@'\\nprint('Hello, world!')\\n'@ | python -"]"#
        } else {
            r#"Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."#
        }.to_string();
        assert_eq!(description, &expected);
    }

    #[test]
    fn shell_tool_with_request_permission_includes_additional_permissions() {
        let tool = super::create_shell_tool(true);
        let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = tool else {
            panic!("expected function tool");
        };
        let JsonSchema::Object { properties, .. } = parameters else {
            panic!("expected object parameters");
        };

        assert!(properties.contains_key("additional_permissions"));

        let Some(JsonSchema::String {
            description: Some(description),
        }) = properties.get("sandbox_permissions")
        else {
            panic!("expected sandbox_permissions description");
        };
        assert!(description.contains("with_additional_permissions"));
        assert!(description.contains("macOS permissions"));

        let Some(JsonSchema::Object {
            properties: additional_properties,
            ..
        }) = properties.get("additional_permissions")
        else {
            panic!("expected additional_permissions schema");
        };
        assert!(additional_properties.contains_key("network"));
        assert!(additional_properties.contains_key("file_system"));
        assert!(additional_properties.contains_key("macos"));
    }

    #[test]
    fn request_permissions_tool_includes_full_permission_schema() {
        let tool = super::create_request_permissions_tool();
        let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = tool else {
            panic!("expected function tool");
        };
        let JsonSchema::Object { properties, .. } = parameters else {
            panic!("expected object parameters");
        };
        let Some(JsonSchema::Object {
            properties: permission_properties,
            additional_properties,
            ..
        }) = properties.get("permissions")
        else {
            panic!("expected permissions object");
        };

        assert_eq!(additional_properties, &Some(false.into()));
        assert!(permission_properties.contains_key("network"));
        assert!(permission_properties.contains_key("file_system"));
        assert!(permission_properties.contains_key("macos"));

        let Some(JsonSchema::Object {
            properties: network_properties,
            additional_properties,
            ..
        }) = permission_properties.get("network")
        else {
            panic!("expected network object");
        };
        assert_eq!(additional_properties, &Some(false.into()));
        assert!(network_properties.contains_key("enabled"));

        let Some(JsonSchema::Object {
            properties: file_system_properties,
            additional_properties,
            ..
        }) = permission_properties.get("file_system")
        else {
            panic!("expected file_system object");
        };
        assert_eq!(additional_properties, &Some(false.into()));
        assert!(file_system_properties.contains_key("read"));
        assert!(file_system_properties.contains_key("write"));

        let Some(JsonSchema::Object {
            properties: macos_properties,
            additional_properties,
            ..
        }) = permission_properties.get("macos")
        else {
            panic!("expected macos object");
        };
        assert_eq!(additional_properties, &Some(false.into()));
        assert!(macos_properties.contains_key("preferences"));
        assert!(macos_properties.contains_key("automations"));
        assert!(macos_properties.contains_key("accessibility"));
        assert!(macos_properties.contains_key("calendar"));
    }

    #[test]
    fn test_shell_command_tool() {
        let tool = super::create_shell_command_tool(true, false);
        let ToolSpec::Function(ResponsesApiTool {
            description, name, ..
        }) = &tool
        else {
            panic!("expected function tool");
        };
        assert_eq!(name, "shell_command");

        let expected = if cfg!(windows) {
            r#"Runs a Powershell command (Windows) and returns its output.

Examples of valid command strings:

- ls -a (show hidden): "Get-ChildItem -Force"
- recursive find by name: "Get-ChildItem -Recurse -Filter *.py"
- recursive grep: "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"
- ps aux | grep python: "Get-Process | Where-Object { $_.ProcessName -like '*python*' }"
- setting an env var: "$env:FOO='bar'; echo $env:FOO"
- running an inline Python script: "@'\\nprint('Hello, world!')\\n'@ | python -"#.to_string()
        } else {
            r#"Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."#.to_string()
        };
        assert_eq!(description, &expected);
    }

    #[test]
    fn test_get_openai_tools_mcp_tools_with_additional_properties_schema() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });
        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "test_server/do_something_cool".to_string(),
                mcp_tool(
                    "do_something_cool",
                    "Do something cool",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "string_argument": {"type": "string"},
                            "number_argument": {"type": "number"},
                            "object_argument": {
                                "type": "object",
                                "properties": {
                                    "string_property": {"type": "string"},
                                    "number_property": {"type": "number"}
                                },
                                "required": ["string_property", "number_property"],
                                "additionalProperties": {
                                    "type": "object",
                                    "properties": {
                                        "addtl_prop": {"type": "string"}
                                    },
                                    "required": ["addtl_prop"],
                                    "additionalProperties": false
                                }
                            }
                        }
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let tool = find_tool(&tools, "test_server/do_something_cool");
        assert_eq!(
            tool.spec,
            ToolSpec::Function(ResponsesApiTool {
                name: "test_server/do_something_cool".to_string(),
                parameters: JsonSchema::Object {
                    properties: BTreeMap::from([
                        (
                            "string_argument".to_string(),
                            JsonSchema::String { description: None }
                        ),
                        (
                            "number_argument".to_string(),
                            JsonSchema::Number { description: None }
                        ),
                        (
                            "object_argument".to_string(),
                            JsonSchema::Object {
                                properties: BTreeMap::from([
                                    (
                                        "string_property".to_string(),
                                        JsonSchema::String { description: None }
                                    ),
                                    (
                                        "number_property".to_string(),
                                        JsonSchema::Number { description: None }
                                    ),
                                ]),
                                required: Some(vec![
                                    "string_property".to_string(),
                                    "number_property".to_string(),
                                ]),
                                additional_properties: Some(
                                    JsonSchema::Object {
                                        properties: BTreeMap::from([(
                                            "addtl_prop".to_string(),
                                            JsonSchema::String { description: None }
                                        ),]),
                                        required: Some(vec!["addtl_prop".to_string(),]),
                                        additional_properties: Some(false.into()),
                                    }
                                    .into()
                                ),
                            },
                        ),
                    ]),
                    required: None,
                    additional_properties: None,
                },
                description: "Do something cool".to_string(),
                strict: false,
                output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            })
        );
    }

    #[test]
    fn code_mode_augments_builtin_tool_descriptions_with_typed_sample() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::CodeMode);
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(&tools_config, None, None, &[]).build();
        let ToolSpec::Function(ResponsesApiTool { description, .. }) =
            &find_tool(&tools, "view_image").spec
        else {
            panic!("expected function tool");
        };

        assert_eq!(
            description,
            "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags).\n\nCode mode declaration:\n```ts\nimport { view_image } from \"tools.js\";\ndeclare function view_image(args: {\n  path: string;\n}): Promise<unknown>;\n```"
        );
    }

    #[test]
    fn code_mode_augments_mcp_tool_descriptions_with_namespaced_sample() {
        let config = test_config();
        let model_info =
            ModelsManager::construct_model_info_offline_for_tests("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::CodeMode);
        features.enable(Feature::UnifiedExec);
        let available_models = Vec::new();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "mcp__sample__echo".to_string(),
                mcp_tool(
                    "echo",
                    "Echo text",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"}
                        },
                        "required": ["message"],
                        "additionalProperties": false
                    }),
                ),
            )])),
            None,
            &[],
        )
        .build();

        let ToolSpec::Function(ResponsesApiTool { description, .. }) =
            &find_tool(&tools, "mcp__sample__echo").spec
        else {
            panic!("expected function tool");
        };

        assert_eq!(
            description,
            "Echo text\n\nCode mode declaration:\n```ts\nimport { echo } from \"tools/mcp/sample.js\";\ndeclare function echo(args: {\n  message: string;\n}): Promise<{\n  _meta?: unknown;\n  content: Array<unknown>;\n  isError?: boolean;\n  structuredContent?: unknown;\n}>;\n```"
        );
    }

    #[test]
    fn chat_tools_include_top_level_name() {
        let properties =
            BTreeMap::from([("foo".to_string(), JsonSchema::String { description: None })]);
        let tools = vec![ToolSpec::Function(ResponsesApiTool {
            name: "demo".to_string(),
            description: "A demo tool".to_string(),
            strict: false,
            parameters: JsonSchema::Object {
                properties,
                required: None,
                additional_properties: None,
            },
            output_schema: None,
        })];

        let responses_json = create_tools_json_for_responses_api(&tools).unwrap();
        assert_eq!(
            responses_json,
            vec![json!({
                "type": "function",
                "name": "demo",
                "description": "A demo tool",
                "strict": false,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "foo": { "type": "string" }
                    },
                },
            })]
        );
    }
}
