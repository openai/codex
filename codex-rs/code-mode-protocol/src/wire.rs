use std::io;

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

pub const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;

pub type RequestId = u64;
pub type SessionId = u64;
pub type CallbackId = u64;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CellId(String);

impl CellId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Request {
        id: RequestId,
        request: HostRequest,
    },
    CancelRequest {
        id: RequestId,
    },
    CallbackResponse {
        id: CallbackId,
        response: CallbackResponse,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    Response {
        id: RequestId,
        result: WireResult<HostResponse>,
    },
    CallbackRequest {
        id: CallbackId,
        session_id: SessionId,
        request: CallbackRequest,
    },
    CancelCallback {
        id: CallbackId,
    },
    CellClosed {
        session_id: SessionId,
        cell_id: CellId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum HostRequest {
    CreateSession,
    ShutdownSession {
        session_id: SessionId,
    },
    CreateCell {
        session_id: SessionId,
        request: CreateCellRequest,
    },
    Observe {
        session_id: SessionId,
        cell_id: CellId,
        mode: ObserveMode,
    },
    Terminate {
        session_id: SessionId,
        cell_id: CellId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostResponse {
    SessionCreated { session_id: SessionId },
    SessionShutdown,
    CellCreated { cell_id: CellId },
    Observed { event: CellEvent },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WireResult<T> {
    Ok { value: T },
    Err { error: Error },
}

impl<T> WireResult<T> {
    pub fn from_result(result: Result<T, Error>) -> Self {
        match result {
            Ok(value) => Self::Ok { value },
            Err(error) => Self::Err { error },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Error {
    MissingSession { session_id: SessionId },
    ShuttingDown,
    DuplicateCell { cell_id: CellId },
    MissingCell { cell_id: CellId },
    ClosedCell { cell_id: CellId },
    BusyObserver { cell_id: CellId },
    AlreadyTerminating { cell_id: CellId },
    Cancelled,
    Runtime { message: String },
    InvalidRequest { message: String },
    CallbackFailed { message: String },
    Internal { message: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateCellRequest {
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub tool_name: ToolName,
    pub description: String,
    pub kind: ToolKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolName {
    pub name: String,
    pub namespace: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObserveMode {
    YieldAfter { duration_ms: u64 },
    PendingFrontier,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CellEvent {
    Yielded {
        content_items: Vec<OutputItem>,
    },
    Pending {
        content_items: Vec<OutputItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed {
        content_items: Vec<OutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<OutputItem>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Text {
        text: String,
    },
    Image {
        image_url: String,
        detail: Option<ImageDetail>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallbackRequest {
    InvokeTool {
        invocation: NestedToolCall,
    },
    Notify {
        call_id: String,
        cell_id: CellId,
        text: String,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NestedToolCall {
    pub cell_id: CellId,
    pub runtime_tool_call_id: String,
    pub tool_name: ToolName,
    pub tool_kind: ToolKind,
    pub input: Option<JsonValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallbackResponse {
    ToolResult { result: JsonValue },
    ToolError { error_text: String },
    NotificationDelivered,
    NotificationError { error_text: String },
}

pub async fn read_frame<R, T>(reader: &mut R) -> io::Result<Option<T>>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let first_length_byte = match reader.read_u8().await {
        Ok(byte) => byte,
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut length_bytes = [first_length_byte, 0, 0, 0];
    reader.read_exact(&mut length_bytes[1..]).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("code-mode IPC frame exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(io::Error::other)
}

pub async fn write_frame<W, T>(writer: &mut W, message: &T) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(message).map_err(io::Error::other)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("code-mode IPC frame exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    let length = u32::try_from(payload.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "code-mode IPC frame length exceeds u32",
        )
    })?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    writer.flush().await
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
