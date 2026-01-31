use crate::agent::AgentRole;
use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::features::Feature;
use crate::features::Features;
use crate::tools::handlers::PLAN_TOOL;
use crate::tools::handlers::apply_patch::create_apply_patch_freeform_tool;
use crate::tools::handlers::apply_patch::create_apply_patch_json_tool;
use crate::tools::handlers::collab::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::collab::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::collab::MIN_WAIT_TIMEOUT_MS;
use crate::tools::registry::ToolRegistryBuilder;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct ToolsConfig {
    pub shell_type: ConfigShellToolType,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_mode: Option<WebSearchMode>,
    pub collab_tools: bool,
    pub collaboration_modes_tools: bool,
    pub request_rule_enabled: bool,
    pub experimental_supported_tools: Vec<String>,
}

pub(crate) struct ToolsConfigParams<'a> {
    pub(crate) model_info: &'a ModelInfo,
    pub(crate) features: &'a Features,
    pub(crate) web_search_mode: Option<WebSearchMode>,
}

impl ToolsConfig {
    pub fn new(params: &ToolsConfigParams) -> Self {
        let ToolsConfigParams {
            model_info,
            features,
            web_search_mode,
        } = params;
        let include_apply_patch_tool = features.enabled(Feature::ApplyPatchFreeform);
        let include_collab_tools = features.enabled(Feature::Collab);
        let include_collaboration_modes_tools = features.enabled(Feature::CollaborationModes);
        let request_rule_enabled = features.enabled(Feature::RequestRule);

        let shell_type = if !features.enabled(Feature::ShellTool) {
            ConfigShellToolType::Disabled
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

        Self {
            shell_type,
            apply_patch_tool_type,
            web_search_mode: *web_search_mode,
            collab_tools: include_collab_tools,
            collaboration_modes_tools: include_collaboration_modes_tools,
            request_rule_enabled,
            experimental_supported_tools: model_info.experimental_supported_tools.clone(),
        }
    }
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

fn create_approval_parameters(include_prefix_rule: bool) -> BTreeMap<String, JsonSchema> {
    let mut properties = BTreeMap::from([
        (
            "sandbox_permissions".to_string(),
            JsonSchema::String {
                description: Some(
                    "Sandbox permissions for the command. Set to \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
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
    ]);

    if include_prefix_rule {
        properties.insert(
            "prefix_rule".to_string(),
            JsonSchema::Array {
                items: Box::new(JsonSchema::String { description: None }),
                description: Some(
                    r#"Only specify when sandbox_permissions is `require_escalated`. 
                    Suggest a prefix command pattern that will allow you to fulfill similar requests from the user in the future.
                    Should be a short but reasonable prefix, e.g. [\"git\", \"pull\"] or [\"uv\", \"run\"] or [\"pytest\"]."#.to_string(),
                ),
            });
    }

    properties
}

fn create_exec_command_tool(include_prefix_rule: bool) -> ToolSpec {
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
            "login".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Whether to run the shell with -l/-i semantics. Defaults to true.".to_string(),
                ),
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
    properties.extend(create_approval_parameters(include_prefix_rule));

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
    })
}

fn create_shell_tool(include_prefix_rule: bool) -> ToolSpec {
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
    properties.extend(create_approval_parameters(include_prefix_rule));

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
    })
}

fn create_shell_command_tool(include_prefix_rule: bool) -> ToolSpec {
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
            "login".to_string(),
            JsonSchema::Boolean {
                description: Some(
                    "Whether to run the shell with login shell semantics. Defaults to true."
                        .to_string(),
                ),
            },
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::Number {
                description: Some("The timeout for the command in milliseconds".to_string()),
            },
        ),
    ]);
    properties.extend(create_approval_parameters(include_prefix_rule));

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
    })
}

fn create_spawn_agent_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "message".to_string(),
        JsonSchema::String {
            description: Some(
                "Initial task for the new agent. Include scope, constraints, and the expected output."
                    .to_string(),
            ),
        },
    );
    properties.insert(
        "agent_type".to_string(),
        JsonSchema::String {
            description: Some(format!(
                "Optional agent type ({}). Use an explicit type when delegating.",
                AgentRole::enum_values().join(", ")
            )),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "spawn_agent".to_string(),
        description:
            "Spawn a sub-agent for a well-scoped task. Returns the agent id to use to communicate with this agent."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["message".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

fn create_send_input_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();
    properties.insert(
        "id".to_string(),
        JsonSchema::String {
            description: Some("Agent id to message (from spawn_agent).".to_string()),
        },
    );
    properties.insert(
        "message".to_string(),
        JsonSchema::String {
            description: Some("Message to send to the agent.".to_string()),
        },
    );
    properties.insert(
        "interrupt".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "When true, stop the agent's current task and handle this immediately. When false (default), queue this message."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "send_input".to_string(),
        description:
            "Send a message to an existing agent. Use interrupt=true to redirect work immediately."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string(), "message".to_string()]),
            additional_properties: Some(false.into()),
        },
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
        description: "Wait for agents to reach a final status. Completed statuses may include the agent's final message. Returns empty status when timed out."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["ids".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

fn create_request_user_input_tool() -> ToolSpec {
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
        description:
            "Request user input for one to three short questions and wait for the response."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["questions".to_string()]),
            additional_properties: Some(false.into()),
        },
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
        description: "Close an agent when it is no longer needed and return its last known status."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["id".to_string()]),
            additional_properties: Some(false.into()),
        },
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
        parameters: JsonSchema::Object {
            properties,
            required: None,
            additional_properties: Some(false.into()),
        },
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
/// Returns JSON values that are compatible with Function Calling in the
/// Chat Completions API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=chat
pub(crate) fn create_tools_json_for_chat_completions_api(
    tools: &[ToolSpec],
) -> crate::error::Result<Vec<serde_json::Value>> {
    // We start with the JSON for the Responses API and than rewrite it to match
    // the chat completions tool call format.
    let responses_api_tools_json = create_tools_json_for_responses_api(tools)?;
    let tools_json = responses_api_tools_json
        .into_iter()
        .filter_map(|mut tool| {
            if tool.get("type") != Some(&serde_json::Value::String("function".to_string())) {
                return None;
            }

            if let Some(map) = tool.as_object_mut() {
                let name = map
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                // Remove "type" field as it is not needed in chat completions.
                map.remove("type");
                Some(json!({
                    "type": "function",
                    "name": name,
                    "function": map,
                }))
            } else {
                None
            }
        })
        .collect::<Vec<serde_json::Value>>();
    Ok(tools_json)
}

pub(crate) fn mcp_tool_to_openai_tool(
    fully_qualified_name: String,
    tool: mcp_types::Tool,
) -> Result<ResponsesApiTool, serde_json::Error> {
    let mcp_types::Tool {
        description,
        mut input_schema,
        ..
    } = tool;

    // OpenAI models mandate the "properties" field in the schema. The Agents
    // SDK fixed this by inserting an empty object for "properties" if it is not
    // already present https://github.com/openai/openai-agents-python/issues/449
    // so here we do the same.
    if input_schema.properties.is_none() {
        input_schema.properties = Some(serde_json::Value::Object(serde_json::Map::new()));
    }

    // Serialize to a raw JSON value so we can sanitize schemas coming from MCP
    // servers. Some servers omit the top-level or nested `type` in JSON
    // Schemas (e.g. using enum/anyOf), or use unsupported variants like
    // `integer`. Our internal JsonSchema is a small subset and requires
    // `type`, so we coerce/sanitize here for compatibility.
    let mut serialized_input_schema = serde_json::to_value(input_schema)?;
    sanitize_json_schema(&mut serialized_input_schema);
    let input_schema = serde_json::from_value::<JsonSchema>(serialized_input_schema)?;

    Ok(ResponsesApiTool {
        name: fully_qualified_name,
        description: description.unwrap_or_default(),
        strict: false,
        parameters: input_schema,
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
    })
}

/// Parse the tool input_schema or return an error for invalid schema
pub fn parse_tool_input_schema(input_schema: &JsonValue) -> Result<JsonSchema, serde_json::Error> {
    let mut input_schema = input_schema.clone();
    sanitize_json_schema(&mut input_schema);
    serde_json::from_value::<JsonSchema>(input_schema)
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
    mcp_tools: Option<HashMap<String, mcp_types::Tool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::CollabHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::GrepFilesHandler;
    use crate::tools::handlers::ListDirHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::McpResourceHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::ReadFileHandler;
    use crate::tools::handlers::RequestUserInputHandler;
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
    let shell_command_handler = Arc::new(ShellCommandHandler);
    let request_user_input_handler = Arc::new(RequestUserInputHandler);

    match &config.shell_type {
        ConfigShellToolType::Default => {
            builder.push_spec(create_shell_tool(config.request_rule_enabled));
        }
        ConfigShellToolType::Local => {
            builder.push_spec(ToolSpec::LocalShell {});
        }
        ConfigShellToolType::UnifiedExec => {
            builder.push_spec(create_exec_command_tool(config.request_rule_enabled));
            builder.push_spec(create_write_stdin_tool());
            builder.register_handler("exec_command", unified_exec_handler.clone());
            builder.register_handler("write_stdin", unified_exec_handler);
        }
        ConfigShellToolType::Disabled => {
            // Do nothing.
        }
        ConfigShellToolType::ShellCommand => {
            builder.push_spec(create_shell_command_tool(config.request_rule_enabled));
        }
    }

    if config.shell_type != ConfigShellToolType::Disabled {
        // Always register shell aliases so older prompts remain compatible.
        builder.register_handler("shell", shell_handler.clone());
        builder.register_handler("container.exec", shell_handler.clone());
        builder.register_handler("local_shell", shell_handler);
        builder.register_handler("shell_command", shell_command_handler);
    }

    builder.push_spec_with_parallel_support(create_list_mcp_resources_tool(), true);
    builder.push_spec_with_parallel_support(create_list_mcp_resource_templates_tool(), true);
    builder.push_spec_with_parallel_support(create_read_mcp_resource_tool(), true);
    builder.register_handler("list_mcp_resources", mcp_resource_handler.clone());
    builder.register_handler("list_mcp_resource_templates", mcp_resource_handler.clone());
    builder.register_handler("read_mcp_resource", mcp_resource_handler);

    builder.push_spec(PLAN_TOOL.clone());
    builder.register_handler("update_plan", plan_handler);

    if config.collaboration_modes_tools {
        builder.push_spec(create_request_user_input_tool());
        builder.register_handler("request_user_input", request_user_input_handler);
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                builder.push_spec(create_apply_patch_freeform_tool());
            }
            ApplyPatchToolType::Function => {
                builder.push_spec(create_apply_patch_json_tool());
            }
        }
        builder.register_handler("apply_patch", apply_patch_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"grep_files".to_string())
    {
        let grep_files_handler = Arc::new(GrepFilesHandler);
        builder.push_spec_with_parallel_support(create_grep_files_tool(), true);
        builder.register_handler("grep_files", grep_files_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"read_file".to_string())
    {
        let read_file_handler = Arc::new(ReadFileHandler);
        builder.push_spec_with_parallel_support(create_read_file_tool(), true);
        builder.register_handler("read_file", read_file_handler);
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "list_dir")
    {
        let list_dir_handler = Arc::new(ListDirHandler);
        builder.push_spec_with_parallel_support(create_list_dir_tool(), true);
        builder.register_handler("list_dir", list_dir_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"test_sync_tool".to_string())
    {
        let test_sync_handler = Arc::new(TestSyncHandler);
        builder.push_spec_with_parallel_support(create_test_sync_tool(), true);
        builder.register_handler("test_sync_tool", test_sync_handler);
    }

    match config.web_search_mode {
        Some(WebSearchMode::Cached) => {
            builder.push_spec(ToolSpec::WebSearch {
                external_web_access: Some(false),
            });
        }
        Some(WebSearchMode::Live) => {
            builder.push_spec(ToolSpec::WebSearch {
                external_web_access: Some(true),
            });
        }
        Some(WebSearchMode::Disabled) | None => {}
    }

    builder.push_spec_with_parallel_support(create_view_image_tool(), true);
    builder.register_handler("view_image", view_image_handler);

    if config.collab_tools {
        let collab_handler = Arc::new(CollabHandler);
        builder.push_spec(create_spawn_agent_tool());
        builder.push_spec(create_send_input_tool());
        builder.push_spec(create_wait_tool());
        builder.push_spec(create_close_agent_tool());
        builder.register_handler("spawn_agent", collab_handler.clone());
        builder.register_handler("send_input", collab_handler.clone());
        builder.register_handler("wait", collab_handler.clone());
        builder.register_handler("close_agent", collab_handler);
    }

    if let Some(mcp_tools) = mcp_tools {
        let mut entries: Vec<(String, mcp_types::Tool)> = mcp_tools.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, tool) in entries.into_iter() {
            match mcp_tool_to_openai_tool(name.clone(), tool.clone()) {
                Ok(converted_tool) => {
                    builder.push_spec(ToolSpec::Function(converted_tool));
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
                    builder.push_spec(ToolSpec::Function(converted_tool));
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
    use crate::tools::registry::ConfiguredToolSpec;
    use mcp_types::ToolInputSchema;
    use pretty_assertions::assert_eq;

    use super::*;

    fn tool_name(tool: &ToolSpec) -> &str {
        match tool {
            ToolSpec::Function(ResponsesApiTool { name, .. }) => name,
            ToolSpec::LocalShell {} => "local_shell",
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
            ToolSpec::Freeform(_) | ToolSpec::LocalShell {} | ToolSpec::WebSearch { .. } => {}
        }
    }

    #[test]
    fn test_full_toolset_specs_for_gpt5_codex_unified_exec_web_search() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        features.enable(Feature::CollaborationModes);
        let config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
        });
        let (tools, _) = build_specs(&config, None, &[]).build();

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
            create_exec_command_tool(true),
            create_write_stdin_tool(),
            create_list_mcp_resources_tool(),
            create_list_mcp_resource_templates_tool(),
            create_read_mcp_resource_tool(),
            PLAN_TOOL.clone(),
            create_request_user_input_tool(),
            create_apply_patch_freeform_tool(),
            ToolSpec::WebSearch {
                external_web_access: Some(true),
            },
            create_view_image_tool(),
        ] {
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
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::Collab);
        features.enable(Feature::CollaborationModes);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();
        assert_contains_tool_names(
            &tools,
            &["spawn_agent", "send_input", "wait", "close_agent"],
        );
    }

    #[test]
    fn request_user_input_requires_collaboration_modes_feature() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.disable(Feature::CollaborationModes);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();
        assert!(
            !tools.iter().any(|t| t.spec.name() == "request_user_input"),
            "request_user_input should be disabled when collaboration_modes feature is off"
        );

        features.enable(Feature::CollaborationModes);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();
        assert_contains_tool_names(&tools, &["request_user_input"]);
    }

    fn assert_model_tools(
        model_slug: &str,
        features: &Features,
        web_search_mode: Option<WebSearchMode>,
        expected_tools: &[&str],
    ) {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline(model_slug, &config);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features,
            web_search_mode,
        });
        let (tools, _) = build_specs(&tools_config, Some(HashMap::new()), &[]).build();
        let tool_names = tools.iter().map(|t| t.spec.name()).collect::<Vec<_>>();
        assert_eq!(&tool_names, &expected_tools,);
    }

    #[test]
    fn web_search_mode_cached_sets_external_web_access_false() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let features = Features::with_defaults();

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(false),
            }
        );
    }

    #[test]
    fn web_search_mode_live_sets_external_web_access_true() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let features = Features::with_defaults();

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();

        let tool = find_tool(&tools, "web_search");
        assert_eq!(
            tool.spec,
            ToolSpec::WebSearch {
                external_web_access: Some(true),
            }
        );
    }

    #[test]
    fn test_build_specs_gpt5_codex_default() {
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5-codex",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "shell_command",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
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
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5.1-codex",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "shell_command",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
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
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5-codex",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
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
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5.1-codex",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_codex_mini_defaults() {
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "codex-mini-latest",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "local_shell",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_codex_5_1_mini_defaults() {
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5.1-codex-mini",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "shell_command",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
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
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "shell",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_gpt_5_1_defaults() {
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "gpt-5.1",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "shell_command",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_exp_5_1_defaults() {
        let mut features = Features::with_defaults();
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "exp-5.1",
            &features,
            Some(WebSearchMode::Cached),
            &[
                "exec_command",
                "write_stdin",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_codex_mini_unified_exec_web_search() {
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        features.enable(Feature::CollaborationModes);
        assert_model_tools(
            "codex-mini-latest",
            &features,
            Some(WebSearchMode::Live),
            &[
                "exec_command",
                "write_stdin",
                "list_mcp_resources",
                "list_mcp_resource_templates",
                "read_mcp_resource",
                "update_plan",
                "request_user_input",
                "web_search",
                "view_image",
            ],
        );
    }

    #[test]
    fn test_build_specs_default_shell_present() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
        });
        let (tools, _) = build_specs(&tools_config, Some(HashMap::new()), &[]).build();

        // Only check the shell variant and a couple of core tools.
        let mut subset = vec!["exec_command", "write_stdin", "update_plan"];
        if let Some(shell_tool) = shell_tool_name(&tools_config) {
            subset.push(shell_tool);
        }
        assert_contains_tool_names(&tools, &subset);
    }

    #[test]
    #[ignore]
    fn test_parallel_support_flags() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();

        assert!(!find_tool(&tools, "exec_command").supports_parallel_tool_calls);
        assert!(!find_tool(&tools, "write_stdin").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "grep_files").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "list_dir").supports_parallel_tool_calls);
        assert!(find_tool(&tools, "read_file").supports_parallel_tool_calls);
    }

    #[test]
    fn test_test_model_info_includes_sync_tool() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("test-gpt-5-codex", &config);
        let features = Features::with_defaults();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(&tools_config, None, &[]).build();

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
        let model_info = ModelsManager::construct_model_info_offline("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Live),
        });
        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "test_server/do_something_cool".to_string(),
                mcp_types::Tool {
                    name: "do_something_cool".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "string_argument": {
                                "type": "string",
                            },
                            "number_argument": {
                                "type": "number",
                            },
                            "object_argument": {
                                "type": "object",
                                "properties": {
                                    "string_property": { "type": "string" },
                                    "number_property": { "type": "number" },
                                },
                                "required": [
                                    "string_property",
                                    "number_property",
                                ],
                                "additionalProperties": Some(false),
                            },
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("Do something cool".to_string()),
                },
            )])),
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
            })
        );
    }

    #[test]
    fn test_build_specs_mcp_tools_sorted_by_name() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("o3", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });

        // Intentionally construct a map with keys that would sort alphabetically.
        let tools_map: HashMap<String, mcp_types::Tool> = HashMap::from([
            (
                "test_server/do".to_string(),
                mcp_types::Tool {
                    name: "a".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({})),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("a".to_string()),
                },
            ),
            (
                "test_server/something".to_string(),
                mcp_types::Tool {
                    name: "b".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({})),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("b".to_string()),
                },
            ),
            (
                "test_server/cool".to_string(),
                mcp_types::Tool {
                    name: "c".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({})),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("c".to_string()),
                },
            ),
        ]);

        let (tools, _) = build_specs(&tools_config, Some(tools_map), &[]).build();

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
    fn test_mcp_tool_property_missing_type_defaults_to_string() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/search".to_string(),
                mcp_types::Tool {
                    name: "search".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "query": {
                                "description": "search query"
                            }
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("Search docs".to_string()),
                },
            )])),
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
            })
        );
    }

    #[test]
    fn test_mcp_tool_integer_normalized_to_number() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/paginate".to_string(),
                mcp_types::Tool {
                    name: "paginate".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "page": { "type": "integer" }
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("Pagination".to_string()),
                },
            )])),
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
            })
        );
    }

    #[test]
    fn test_mcp_tool_array_without_items_gets_default_string_items() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        features.enable(Feature::ApplyPatchFreeform);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/tags".to_string(),
                mcp_types::Tool {
                    name: "tags".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "tags": { "type": "array" }
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("Tags".to_string()),
                },
            )])),
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
            })
        );
    }

    #[test]
    fn test_mcp_tool_anyof_defaults_to_string() {
        let config = test_config();
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });

        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "dash/value".to_string(),
                mcp_types::Tool {
                    name: "value".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "value": { "anyOf": [ { "type": "string" }, { "type": "number" } ] }
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("AnyOf Value".to_string()),
                },
            )])),
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
            })
        );
    }

    #[test]
    fn test_shell_tool() {
        let tool = super::create_shell_tool(true);
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
    fn test_shell_command_tool() {
        let tool = super::create_shell_command_tool(true);
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
        let model_info = ModelsManager::construct_model_info_offline("gpt-5-codex", &config);
        let mut features = Features::with_defaults();
        features.enable(Feature::UnifiedExec);
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
        });
        let (tools, _) = build_specs(
            &tools_config,
            Some(HashMap::from([(
                "test_server/do_something_cool".to_string(),
                mcp_types::Tool {
                    name: "do_something_cool".to_string(),
                    input_schema: ToolInputSchema {
                        properties: Some(serde_json::json!({
                            "string_argument": {
                                "type": "string",
                            },
                            "number_argument": {
                                "type": "number",
                            },
                            "object_argument": {
                                "type": "object",
                                "properties": {
                                    "string_property": { "type": "string" },
                                    "number_property": { "type": "number" },
                                },
                                "required": [
                                    "string_property",
                                    "number_property",
                                ],
                                "additionalProperties": {
                                    "type": "object",
                                    "properties": {
                                        "addtl_prop": { "type": "string" },
                                    },
                                    "required": [
                                        "addtl_prop",
                                    ],
                                    "additionalProperties": false,
                                },
                            },
                        })),
                        required: None,
                        r#type: "object".to_string(),
                    },
                    output_schema: None,
                    title: None,
                    annotations: None,
                    description: Some("Do something cool".to_string()),
                },
            )])),
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
            })
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

        let tools_json = create_tools_json_for_chat_completions_api(&tools).unwrap();

        assert_eq!(
            tools_json,
            vec![json!({
                "type": "function",
                "name": "demo",
                "function": {
                    "name": "demo",
                    "description": "A demo tool",
                    "strict": false,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "foo": { "type": "string" }
                        },
                    },
                }
            })]
        );
    }
}
