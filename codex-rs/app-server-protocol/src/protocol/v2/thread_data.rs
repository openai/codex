use super::CodexErrorInfo;
use super::ThreadItem;
use super::ThreadStatus;
use super::TurnStatus;
use codex_protocol::dynamic_tools::DynamicToolSpec as CoreDynamicToolSpec;
use codex_protocol::protocol::ErrorEvent as CoreErrorEvent;
use codex_protocol::protocol::SessionSource as CoreSessionSource;
use codex_protocol::protocol::SubAgentSource as CoreSubAgentSource;
use codex_protocol::protocol::ThreadSource as CoreThreadSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use thiserror::Error;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
#[derive(Default)]
pub enum SessionSource {
    Cli,
    #[serde(rename = "vscode")]
    #[ts(rename = "vscode")]
    #[default]
    VsCode,
    Exec,
    AppServer,
    Custom(String),
    SubAgent(CoreSubAgentSource),
    #[serde(other)]
    Unknown,
}

impl From<CoreSessionSource> for SessionSource {
    fn from(value: CoreSessionSource) -> Self {
        match value {
            CoreSessionSource::Cli => SessionSource::Cli,
            CoreSessionSource::VSCode => SessionSource::VsCode,
            CoreSessionSource::Exec => SessionSource::Exec,
            CoreSessionSource::Mcp => SessionSource::AppServer,
            CoreSessionSource::Custom(source) => SessionSource::Custom(source),
            // We do not want to render those at the app-server level.
            CoreSessionSource::Internal(_) => SessionSource::Unknown,
            CoreSessionSource::SubAgent(sub) => SessionSource::SubAgent(sub),
            CoreSessionSource::Unknown => SessionSource::Unknown,
        }
    }
}

impl From<SessionSource> for CoreSessionSource {
    fn from(value: SessionSource) -> Self {
        match value {
            SessionSource::Cli => CoreSessionSource::Cli,
            SessionSource::VsCode => CoreSessionSource::VSCode,
            SessionSource::Exec => CoreSessionSource::Exec,
            SessionSource::AppServer => CoreSessionSource::Mcp,
            SessionSource::Custom(source) => CoreSessionSource::Custom(source),
            SessionSource::SubAgent(sub) => CoreSessionSource::SubAgent(sub),
            SessionSource::Unknown => CoreSessionSource::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum ThreadSource {
    User,
    Subagent,
    MemoryConsolidation,
}

impl From<CoreThreadSource> for ThreadSource {
    fn from(value: CoreThreadSource) -> Self {
        match value {
            CoreThreadSource::User => ThreadSource::User,
            CoreThreadSource::Subagent => ThreadSource::Subagent,
            CoreThreadSource::MemoryConsolidation => ThreadSource::MemoryConsolidation,
        }
    }
}

impl From<ThreadSource> for CoreThreadSource {
    fn from(value: ThreadSource) -> Self {
        match value {
            ThreadSource::User => CoreThreadSource::User,
            ThreadSource::Subagent => CoreThreadSource::Subagent,
            ThreadSource::MemoryConsolidation => CoreThreadSource::MemoryConsolidation,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GitInfo {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Thread {
    pub id: String,
    /// Session id shared by threads that belong to the same session tree.
    pub session_id: String,
    /// Source thread id when this thread was created by forking another thread.
    pub forked_from_id: Option<String>,
    /// The ID of the parent thread. This will only be set if this thread is a subagent.
    pub parent_thread_id: Option<String>,
    /// Usually the first user message in the thread, if available.
    pub preview: String,
    /// Whether the thread is ephemeral and should not be materialized on disk.
    pub ephemeral: bool,
    /// Model provider used for this thread (for example, 'openai').
    pub model_provider: String,
    /// Unix timestamp (in seconds) when the thread was created.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Unix timestamp (in seconds) when the thread was last updated.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// Current runtime status for the thread.
    pub status: ThreadStatus,
    /// [UNSTABLE] Path to the thread on disk.
    pub path: Option<PathBuf>,
    /// Working directory captured for the thread.
    pub cwd: AbsolutePathBuf,
    /// Version of the CLI that created the thread.
    pub cli_version: String,
    /// Origin of the thread (CLI, VSCode, codex exec, codex app-server, etc.).
    pub source: SessionSource,
    /// Optional analytics source classification for this thread.
    pub thread_source: Option<ThreadSource>,
    /// Optional random unique nickname assigned to an AgentControl-spawned sub-agent.
    pub agent_nickname: Option<String>,
    /// Optional role (agent_role) assigned to an AgentControl-spawned sub-agent.
    pub agent_role: Option<String>,
    /// Optional Git metadata captured when the thread was created.
    pub git_info: Option<GitInfo>,
    /// Optional user-facing thread title.
    pub name: Option<String>,
    /// Only populated on `thread/resume`, `thread/rollback`, `thread/fork`, and `thread/read`
    /// (when `includeTurns` is true) responses.
    /// For all other responses and notifications returning a Thread,
    /// the turns field will be an empty list.
    pub turns: Vec<Turn>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct Turn {
    pub id: String,
    /// Thread items currently included in this turn payload.
    pub items: Vec<ThreadItem>,
    /// Describes how much of `items` has been loaded for this turn.
    #[serde(default)]
    pub items_view: TurnItemsView,
    pub status: TurnStatus,
    /// Only populated when the Turn's status is failed.
    pub error: Option<TurnError>,
    /// Unix timestamp (in seconds) when the turn started.
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    /// Unix timestamp (in seconds) when the turn completed.
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    /// Duration between turn start and completion in milliseconds, if known.
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum TurnItemsView {
    /// `items` was not loaded for this turn. The field is intentionally empty.
    NotLoaded,
    /// `items` contains only a display summary for this turn.
    Summary,
    /// `items` contains every ThreadItem available from persisted app-server history for this turn.
    #[default]
    Full,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, Error)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
#[error("{message}")]
pub struct TurnError {
    pub message: String,
    pub codex_error_info: Option<CodexErrorInfo>,
    #[serde(default)]
    pub additional_details: Option<String>,
    #[serde(default)]
    pub data: Option<TurnErrorData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "reason", rename_all = "camelCase")]
#[ts(tag = "reason")]
#[ts(export_to = "v2/")]
pub enum TurnErrorData {
    #[serde(rename = "dynamicToolInputSchema", rename_all = "camelCase")]
    #[ts(rename = "dynamicToolInputSchema", rename_all = "camelCase")]
    DynamicToolInputSchema {
        backend: String,
        tool: Option<DynamicToolSchemaErrorTool>,
        tool_candidates: Vec<DynamicToolSchemaErrorTool>,
        schema_path: Option<String>,
        underlying_error: String,
        remediation: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DynamicToolSchemaErrorTool {
    pub index: usize,
    pub namespace: Option<String>,
    pub name: String,
    pub qualified_name: String,
}

impl TurnError {
    pub fn from_core_error_event(
        event: &CoreErrorEvent,
        dynamic_tools: Option<&[CoreDynamicToolSpec]>,
    ) -> Self {
        Self {
            message: event.message.clone(),
            codex_error_info: event.codex_error_info.clone().map(Into::into),
            additional_details: None,
            data: dynamic_tool_input_schema_error_data(
                &event.message,
                dynamic_tools.unwrap_or(&[]),
            ),
        }
    }
}

fn dynamic_tool_input_schema_error_data(
    message: &str,
    dynamic_tools: &[CoreDynamicToolSpec],
) -> Option<TurnErrorData> {
    let underlying_error = underlying_backend_error_message(message);
    let backend_function_name = backend_function_name_from_schema_error(&underlying_error)?;
    let matching_tools = dynamic_tools
        .iter()
        .enumerate()
        .filter(|(_, tool)| dynamic_tool_matches_backend_name(tool, backend_function_name))
        .collect::<Vec<_>>();

    if matching_tools.is_empty() {
        return None;
    }
    let (tool, tool_candidates, schema_path) = match matching_tools.as_slice() {
        [(index, tool)] => (
            Some(dynamic_tool_error_tool(*index, tool)),
            Vec::new(),
            Some(dynamic_tool_schema_path(*index, &tool.input_schema)),
        ),
        _ => (
            None,
            matching_tools
                .iter()
                .map(|(index, tool)| dynamic_tool_error_tool(*index, tool))
                .collect(),
            None,
        ),
    };

    Some(TurnErrorData::DynamicToolInputSchema {
        backend: "Responses API".to_string(),
        tool,
        tool_candidates,
        schema_path,
        underlying_error,
        remediation: DYNAMIC_TOOL_INPUT_SCHEMA_REMEDIATION.to_string(),
    })
}

const DYNAMIC_TOOL_INPUT_SCHEMA_REMEDIATION: &str = "Adjust the dynamic tool inputSchema to the JSON Schema subset accepted by the model backend. For discriminated unions, wrap the union in a top-level object property or flatten it into one object schema.";

fn underlying_backend_error_message(message: &str) -> String {
    let Ok(value) = serde_json::from_str::<JsonValue>(message) else {
        return message.to_string();
    };
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or(message)
        .to_string()
}

fn backend_function_name_from_schema_error(message: &str) -> Option<&str> {
    if !message.contains("Invalid schema for function")
        && !message.contains("invalid_function_parameters")
    {
        return None;
    }

    for (prefix, suffix) in [
        ("function '", "'"),
        ("function \"", "\""),
        ("function `", "`"),
    ] {
        let Some((_, rest)) = message.split_once(prefix) else {
            continue;
        };
        let Some((name, _)) = rest.split_once(suffix) else {
            continue;
        };
        if !name.is_empty() {
            return Some(name);
        }
    }

    None
}

fn dynamic_tool_schema_path(index: usize, input_schema: &JsonValue) -> String {
    let suffix = if input_schema.get("anyOf").is_some() {
        ".anyOf"
    } else if input_schema.get("oneOf").is_some() {
        ".oneOf"
    } else {
        ""
    };
    format!("dynamicTools[{index}].inputSchema{suffix}")
}

fn dynamic_tool_matches_backend_name(
    tool: &CoreDynamicToolSpec,
    backend_function_name: &str,
) -> bool {
    backend_function_name == tool.name || backend_function_name == qualified_dynamic_tool_name(tool)
}

fn dynamic_tool_error_tool(index: usize, tool: &CoreDynamicToolSpec) -> DynamicToolSchemaErrorTool {
    DynamicToolSchemaErrorTool {
        index,
        namespace: tool.namespace.clone(),
        name: tool.name.clone(),
        qualified_name: qualified_dynamic_tool_name(tool),
    }
}

fn qualified_dynamic_tool_name(tool: &CoreDynamicToolSpec) -> String {
    match tool.namespace.as_deref() {
        Some(namespace) if !namespace.is_empty() => format!("{namespace}.{}", tool.name),
        Some(_) | None => tool.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicToolSchemaErrorTool;
    use super::TurnError;
    use super::TurnErrorData;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::protocol::CodexErrorInfo;
    use codex_protocol::protocol::ErrorEvent;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn dynamic_tool_schema_error_extracts_json_backend_message() {
        let backend_error = "invalid_function_parameters ... Invalid schema for function 'create': schema must be a JSON Schema of 'type: \"object\"', got 'type: \"None\"";
        let error = TurnError::from_core_error_event(
            &ErrorEvent {
                message: json!({
                    "error": {
                        "message": backend_error,
                        "type": "invalid_request_error",
                        "code": "invalid_function_parameters"
                    }
                })
                .to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Some(&[dynamic_tool(Some("repro"), "create", json!({"anyOf": []}))]),
        );

        assert_eq!(
            error.data,
            Some(TurnErrorData::DynamicToolInputSchema {
                backend: "Responses API".to_string(),
                tool: Some(DynamicToolSchemaErrorTool {
                    index: 0,
                    namespace: Some("repro".to_string()),
                    name: "create".to_string(),
                    qualified_name: "repro.create".to_string(),
                }),
                tool_candidates: Vec::new(),
                schema_path: Some("dynamicTools[0].inputSchema.anyOf".to_string()),
                underlying_error: backend_error.to_string(),
                remediation: super::DYNAMIC_TOOL_INPUT_SCHEMA_REMEDIATION.to_string(),
            })
        );
    }

    #[test]
    fn dynamic_tool_schema_error_matches_namespaced_backend_name() {
        let error = TurnError::from_core_error_event(
            &ErrorEvent {
                message: "Invalid schema for function 'repro.create'".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Some(&[
                dynamic_tool(Some("other"), "create", json!({"anyOf": []})),
                dynamic_tool(Some("repro"), "create", json!({"oneOf": []})),
            ]),
        );

        assert_eq!(
            error.data,
            Some(TurnErrorData::DynamicToolInputSchema {
                backend: "Responses API".to_string(),
                tool: Some(DynamicToolSchemaErrorTool {
                    index: 1,
                    namespace: Some("repro".to_string()),
                    name: "create".to_string(),
                    qualified_name: "repro.create".to_string(),
                }),
                tool_candidates: Vec::new(),
                schema_path: Some("dynamicTools[1].inputSchema.oneOf".to_string()),
                underlying_error: "Invalid schema for function 'repro.create'".to_string(),
                remediation: super::DYNAMIC_TOOL_INPUT_SCHEMA_REMEDIATION.to_string(),
            })
        );
    }

    #[test]
    fn dynamic_tool_schema_error_reports_ambiguous_bare_name_candidates() {
        let error = TurnError::from_core_error_event(
            &ErrorEvent {
                message: "Invalid schema for function 'create'".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Some(&[
                dynamic_tool(Some("alpha"), "create", json!({"anyOf": []})),
                dynamic_tool(Some("beta"), "create", json!({"oneOf": []})),
            ]),
        );

        assert_eq!(
            error.data,
            Some(TurnErrorData::DynamicToolInputSchema {
                backend: "Responses API".to_string(),
                tool: None,
                tool_candidates: vec![
                    DynamicToolSchemaErrorTool {
                        index: 0,
                        namespace: Some("alpha".to_string()),
                        name: "create".to_string(),
                        qualified_name: "alpha.create".to_string(),
                    },
                    DynamicToolSchemaErrorTool {
                        index: 1,
                        namespace: Some("beta".to_string()),
                        name: "create".to_string(),
                        qualified_name: "beta.create".to_string(),
                    },
                ],
                schema_path: None,
                underlying_error: "Invalid schema for function 'create'".to_string(),
                remediation: super::DYNAMIC_TOOL_INPUT_SCHEMA_REMEDIATION.to_string(),
            })
        );
    }

    #[test]
    fn dynamic_tool_schema_error_ignores_unknown_function_name() {
        let error = TurnError::from_core_error_event(
            &ErrorEvent {
                message: "Invalid schema for function 'shell'".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Some(&[dynamic_tool(Some("repro"), "create", json!({"anyOf": []}))]),
        );

        assert_eq!(error.data, None);
    }

    fn dynamic_tool(
        namespace: Option<&str>,
        name: &str,
        input_schema: serde_json::Value,
    ) -> DynamicToolSpec {
        DynamicToolSpec {
            namespace: namespace.map(str::to_string),
            name: name.to_string(),
            description: "test".to_string(),
            input_schema,
            defer_loading: false,
        }
    }
}
