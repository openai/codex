use serde::Deserialize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest<T: Serialize> {
    pub jsonrpc: &'static str,
    pub id: String,
    pub method: String,
    pub params: T,
}

impl<T: Serialize> JsonRpcRequest<T> {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: T) -> Self {
        Self {
            jsonrpc: "2.0",
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[allow(dead_code)]
    pub id: Option<String>,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// task/create
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CreateTaskParams {
    pub name: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: String,
}

// ---------------------------------------------------------------------------
// message/send
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MessageSendParams {
    pub task_id: String,
    pub messages: Vec<TaskMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    pub role: String,
    pub content: Vec<TaskMessageContent>,
}

// ---------------------------------------------------------------------------
// Message content types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskMessageContent {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        author: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<String>,
    },
    ToolRequest {
        tool_call_id: String,
        name: String,
        arguments: String,
    },
    ToolResponse {
        tool_call_id: String,
        name: String,
        content: String,
    },
    Reasoning {
        #[serde(default)]
        summary: Vec<ReasoningSummaryEntry>,
        #[serde(default)]
        content: Vec<ReasoningContentEntry>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummaryEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningContentEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Streaming response types (from Agentex)
// ---------------------------------------------------------------------------

/// Envelope for streamed task-message updates (NDJSON lines).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskMessageUpdate {
    Start {
        message: TaskMessage,
    },
    Delta {
        delta: StreamDelta,
    },
    Full {
        message: TaskMessage,
    },
    Done {
        message: TaskMessage,
    },
    Error {
        #[serde(default)]
        message: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamDelta {
    TextDelta {
        text: String,
    },
    ToolRequestDelta {
        tool_call_id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        arguments: Option<String>,
    },
    ReasoningSummaryDelta {
        text: String,
    },
    ReasoningContentDelta {
        text: String,
    },
}

// ---------------------------------------------------------------------------
// Non-streaming message/send response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct MessageSendResult {
    pub messages: Vec<TaskMessage>,
}
