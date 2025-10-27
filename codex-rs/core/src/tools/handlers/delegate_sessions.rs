use std::collections::BTreeMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::delegate_tool::DelegateSessionMessages;
use crate::delegate_tool::DelegateSessionMode;
use crate::delegate_tool::DelegateSessionsList;
use crate::delegate_tool::DelegateToolError;
use crate::function_tool::FunctionCallError;
use crate::openai_tools::JsonSchema;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

#[derive(Debug, Deserialize)]
struct DelegateSessionsArgs {
    operation: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize, Default)]
struct DelegateSessionsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    sessions: Option<Vec<DelegateSessionListEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<DelegateSessionMessageEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
struct DelegateSessionsResponse {
    status: &'static str,
    #[serde(flatten)]
    payload: DelegateSessionsPayload,
}

#[derive(Debug, Serialize)]
struct DelegateSessionListEntry {
    conversation_id: String,
    agent_id: String,
    mode: String,
    cwd: String,
    last_interacted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    shadow: Option<DelegateSessionShadowSummary>,
}

#[derive(Debug, Serialize)]
struct DelegateSessionShadowSummary {
    events: usize,
    user_inputs: usize,
    agent_outputs: usize,
    turns: usize,
    raw_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    compressed_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
struct DelegateSessionMessageEntry {
    id: String,
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

pub struct DelegateSessionsHandler;

pub static DELEGATE_SESSIONS_TOOL: LazyLock<ToolSpec> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();
    properties.insert(
        "operation".to_string(),
        JsonSchema::String {
            description: Some("Operation to perform: list, messages, or dismiss".to_string()),
        },
    );
    properties.insert(
        "conversation_id".to_string(),
        JsonSchema::String {
            description: Some("Target conversation id for messages or dismiss".to_string()),
        },
    );
    properties.insert(
        "cursor".to_string(),
        JsonSchema::String {
            description: Some("Opaque pagination cursor".to_string()),
        },
    );
    properties.insert(
        "limit".to_string(),
        JsonSchema::Number {
            description: Some("Maximum number of entries to return (default 3)".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "delegate_sessions".to_string(),
        description: "Inspect or manage reusable delegate sessions".to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["operation".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
});

#[async_trait]
impl ToolHandler for DelegateSessionsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "delegate_sessions handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: DelegateSessionsArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e}"))
        })?;

        let adapter = session.delegate_adapter().ok_or_else(|| {
            FunctionCallError::RespondToModel("delegate tool is not available".to_string())
        })?;

        let limit = args.limit.unwrap_or(3).max(1);
        let response = match args.operation.as_str() {
            "list" => {
                let result: DelegateSessionsList = adapter
                    .list_sessions(args.cursor.clone(), limit)
                    .await
                    .map_err(map_adapter_error)?;
                let sessions = result
                    .sessions
                    .into_iter()
                    .map(|session| DelegateSessionListEntry {
                        conversation_id: session.conversation_id,
                        agent_id: session.agent_id,
                        mode: mode_to_string(session.mode),
                        cwd: session.cwd,
                        last_interacted_at: session.last_interacted_at,
                        shadow: session.shadow.map(|shadow| DelegateSessionShadowSummary {
                            events: shadow.events,
                            user_inputs: shadow.user_inputs,
                            agent_outputs: shadow.agent_outputs,
                            turns: shadow.turns,
                            raw_bytes: shadow.raw_bytes,
                            compressed_bytes: shadow.compressed_bytes,
                        }),
                    })
                    .collect();
                DelegateSessionsResponse {
                    status: "ok",
                    payload: DelegateSessionsPayload {
                        sessions: Some(sessions),
                        next_cursor: result.next_cursor,
                        ..DelegateSessionsPayload::default()
                    },
                }
            }
            "messages" => {
                let conversation_id = args.conversation_id.ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "`conversation_id` is required for operation messages".to_string(),
                    )
                })?;
                let result: DelegateSessionMessages = adapter
                    .session_messages(&conversation_id, args.cursor.clone(), limit)
                    .await
                    .map_err(map_adapter_error)?;
                let messages = result
                    .messages
                    .into_iter()
                    .map(|message| DelegateSessionMessageEntry {
                        id: message.id,
                        role: message.role,
                        content: message.content,
                        timestamp: message.timestamp,
                    })
                    .collect();
                DelegateSessionsResponse {
                    status: "ok",
                    payload: DelegateSessionsPayload {
                        messages: Some(messages),
                        next_cursor: result.next_cursor,
                        ..DelegateSessionsPayload::default()
                    },
                }
            }
            "dismiss" => {
                let conversation_id = args.conversation_id.ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "`conversation_id` is required for operation dismiss".to_string(),
                    )
                })?;
                adapter
                    .dismiss_session(&conversation_id)
                    .await
                    .map_err(map_adapter_error)?;
                DelegateSessionsResponse {
                    status: "ok",
                    payload: DelegateSessionsPayload::default(),
                }
            }
            other => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "unknown operation `{other}`"
                )));
            }
        };

        let content = serde_json::to_string(&response)
            .map_err(|e| FunctionCallError::Fatal(format!("failed to serialize response: {e}")))?;

        Ok(ToolOutput::Function {
            content,
            success: Some(true),
        })
    }
}

fn map_adapter_error(err: DelegateToolError) -> FunctionCallError {
    match err {
        DelegateToolError::DelegateInProgress => FunctionCallError::RespondToModel(
            "another delegate is already running; wait before listing sessions".to_string(),
        ),
        DelegateToolError::QueueFull => {
            FunctionCallError::RespondToModel("delegate queue is full; try again later".to_string())
        }
        DelegateToolError::AgentNotFound(agent_id) => FunctionCallError::RespondToModel(format!(
            "delegate agent `{agent_id}` is not configured"
        )),
        DelegateToolError::SetupFailed(reason) => {
            FunctionCallError::RespondToModel(format!("delegate operation failed: {reason}"))
        }
        DelegateToolError::SessionNotFound(conversation_id) => FunctionCallError::RespondToModel(
            format!("delegate session `{conversation_id}` is not available"),
        ),
        DelegateToolError::AgentBusy => FunctionCallError::RespondToModel(
            "delegate session is busy; wait for it to finish".to_string(),
        ),
        DelegateToolError::InvalidCursor => {
            FunctionCallError::RespondToModel("invalid delegate pagination cursor".to_string())
        }
        DelegateToolError::HistoryUnavailable(conversation_id) => {
            FunctionCallError::RespondToModel(format!(
                "delegate history is unavailable for session `{conversation_id}`"
            ))
        }
    }
}

fn mode_to_string(mode: DelegateSessionMode) -> String {
    match mode {
        DelegateSessionMode::Standard => "standard".to_string(),
        DelegateSessionMode::Detached => "detached".to_string(),
    }
}
