use std::io;

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use crate::CellId;
use crate::CodeModeNestedToolCall;
use crate::ExecuteRequest;
use crate::RuntimeResponse;
use crate::WaitOutcome;
use crate::WaitRequest;

// Keep allocations from untrusted frame lengths bounded while leaving enough
// room for large inline image results. These are base64 encoded and can easily
// exceed the previous 16 MiB limit before downstream output truncation runs.
const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

pub type RequestId = u64;
pub type SessionId = u64;
pub type DelegateRequestId = u64;

#[derive(Debug, Deserialize, Serialize)]
pub enum ClientMessage {
    Request {
        id: RequestId,
        request: HostRequest,
    },
    DelegateResponse {
        id: DelegateRequestId,
        response: Result<DelegateResponse, String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum HostMessage {
    Response {
        id: RequestId,
        response: Result<HostResponse, String>,
    },
    InitialResponse {
        id: RequestId,
        response: Result<RuntimeResponse, String>,
    },
    DelegateRequest {
        id: DelegateRequestId,
        session_id: SessionId,
        request: DelegateRequest,
    },
    CancelDelegateRequest {
        id: DelegateRequestId,
    },
    CellClosed {
        session_id: SessionId,
        cell_id: CellId,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum HostRequest {
    CreateSession,
    Execute {
        session_id: SessionId,
        request: ExecuteRequest,
    },
    Wait {
        session_id: SessionId,
        request: WaitRequest,
    },
    Terminate {
        session_id: SessionId,
        cell_id: CellId,
    },
    ShutdownSession {
        session_id: SessionId,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum HostResponse {
    SessionCreated { session_id: SessionId },
    ExecutionStarted { cell_id: CellId },
    WaitCompleted { outcome: WaitOutcome },
    SessionShutdown,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum DelegateRequest {
    InvokeTool(CodeModeNestedToolCall),
    Notify {
        call_id: String,
        cell_id: CellId,
        text: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum DelegateResponse {
    ToolResult(JsonValue),
    NotificationDelivered,
}

pub async fn read_frame<R, T>(reader: &mut R) -> io::Result<Option<T>>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let length = match reader.read_u32().await {
        Ok(length) => length as usize,
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    };
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
    let length = u32::try_from(payload.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "code-mode IPC frame length exceeds u32",
        )
    })?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("code-mode IPC frame exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    writer.write_u32(length).await?;
    writer.write_all(&payload).await?;
    writer.flush().await
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
