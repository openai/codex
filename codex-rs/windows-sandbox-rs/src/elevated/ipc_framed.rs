//! Framed IPC protocol used between the parent (CLI) and the elevated command runner.
//!
//! This module defines the JSON message schema (spawn request/ready, output, stdin,
//! exit, error, terminate) plus length‑prefixed framing helpers for a byte stream.
//! It is **elevated-path only**: the parent uses it to bootstrap the runner and
//! stream unified_exec I/O over named pipes. The legacy restricted‑token path does
//! not use this protocol, and non‑unified exec capture uses it only when running
//! through the elevated runner.

use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

/// Protocol version shared by the parent process and elevated command runner.
pub const IPC_PROTOCOL_VERSION: u8 = 6;

/// Length-prefixed, JSON-encoded frame.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FramedMessage {
    pub version: u8,
    #[serde(flatten)]
    pub message: Message,
}

/// IPC message variants exchanged between parent and runner.
///
/// `SpawnRequest`, `Stdin`, `CloseStdin`, `Resize`, and `Terminate` are parent->runner commands.
/// `SpawnReady`, `Output`, `Exit`, and `Error` are runner->parent events/results.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    SpawnRequest { payload: Box<SpawnRequest> },
    SpawnReady { payload: SpawnReady },
    Output { payload: OutputPayload },
    Stdin { payload: StdinPayload },
    CloseStdin { payload: EmptyPayload },
    Resize { payload: ResizePayload },
    Exit { payload: ExitPayload },
    Error { payload: ErrorPayload },
    Terminate { payload: EmptyPayload },
}

/// Spawn parameters sent from parent to runner.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpawnRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub permission_profile: PermissionProfile,
    pub workspace_roots: Vec<AbsolutePathBuf>,
    pub codex_home: PathBuf,
    pub real_codex_home: PathBuf,
    pub cap_sids: Vec<String>,
    /// Optional managed-network identity added only to the child's restricting SID set.
    #[serde(default)]
    pub network_proxy_restricting_sid: Option<String>,
    pub timeout_ms: Option<u64>,
    pub tty: bool,
    #[serde(default)]
    pub stdin_open: bool,
    /// Private desktop kept alive by the parent across command runners.
    pub private_desktop_name: Option<String>,
}

/// Ack from runner after it spawns the child process.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpawnReady {
    pub process_id: u32,
}

/// Output data sent from runner to parent.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutputPayload {
    pub data_b64: String,
    pub stream: OutputStream,
}

/// Output stream identifier for `OutputPayload`.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Stdin bytes sent from parent to runner.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StdinPayload {
    pub data_b64: String,
}

/// PTY resize request sent from parent to runner.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResizePayload {
    pub rows: u16,
    pub cols: u16,
}

/// Exit status sent from runner to parent.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExitPayload {
    pub exit_code: i32,
    pub timed_out: bool,
}

/// Error payload sent when the runner fails to spawn or stream.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorPayload {
    pub message: String,
    pub stage: ErrorStage,
    pub windows_error_code: Option<u32>,
}

/// Runner startup stage that produced an error.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStage {
    ReadSpawnRequest,
    SpawnChild,
    WriteSpawnReady,
}

/// Empty payload for control messages.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EmptyPayload {}

/// Base64-encode raw bytes for IPC payloads.
pub fn encode_bytes(data: &[u8]) -> String {
    STANDARD.encode(data)
}

/// Decode base64 payload data into raw bytes.
pub fn decode_bytes(data: &str) -> Result<Vec<u8>> {
    Ok(STANDARD.decode(data.as_bytes())?)
}

/// Write a length-prefixed JSON frame.
pub fn write_frame<W: Write>(writer: W, msg: &FramedMessage) -> Result<()> {
    crate::framed_io::write_frame(writer, msg)
}

/// Read a length-prefixed JSON frame; returns `Ok(None)` on EOF.
pub fn read_frame<R: Read>(reader: R) -> Result<Option<FramedMessage>> {
    crate::framed_io::read_frame(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn framed_round_trip() {
        let msg = FramedMessage {
            version: IPC_PROTOCOL_VERSION,
            message: Message::Output {
                payload: OutputPayload {
                    data_b64: encode_bytes(b"hello"),
                    stream: OutputStream::Stdout,
                },
            },
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).expect("write");
        let decoded = read_frame(buf.as_slice()).expect("read").expect("some");
        assert_eq!(decoded.version, IPC_PROTOCOL_VERSION);
        match decoded.message {
            Message::Output { payload } => {
                assert_eq!(payload.stream, OutputStream::Stdout);
                let data = decode_bytes(&payload.data_b64).expect("decode");
                assert_eq!(data, b"hello");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn spawn_request_serializes_permission_profile() {
        let workspace_roots = vec![
            AbsolutePathBuf::from_absolute_path(PathBuf::from(r"C:\workspace"))
                .expect("absolute workspace root"),
        ];
        let msg = FramedMessage {
            version: IPC_PROTOCOL_VERSION,
            message: Message::SpawnRequest {
                payload: Box::new(SpawnRequest {
                    command: vec!["cmd.exe".to_string(), "/c".to_string(), "ver".to_string()],
                    cwd: PathBuf::from(r"C:\workspace"),
                    env: HashMap::new(),
                    permission_profile: PermissionProfile::read_only(),
                    workspace_roots: workspace_roots.clone(),
                    codex_home: PathBuf::from(r"C:\codex"),
                    real_codex_home: PathBuf::from(r"C:\Users\codex"),
                    cap_sids: vec!["S-1-15-3-1024-1".to_string()],
                    network_proxy_restricting_sid: Some("S-1-5-21-100-200-300-400".to_string()),
                    timeout_ms: Some(1000),
                    tty: false,
                    stdin_open: false,
                    private_desktop_name: Some("CodexSandboxDesktop-1234".to_string()),
                }),
            },
        };

        let encoded = serde_json::to_value(&msg).expect("serialize");
        assert_eq!("spawn_request", encoded["type"]);
        assert_eq!("managed", encoded["payload"]["permission_profile"]["type"]);
        assert_eq!(None, encoded["payload"].get("policy_json_or_preset"));
        assert_eq!(None, encoded["payload"].get("sandbox_policy_cwd"));
        assert_eq!(None, encoded["payload"].get("permission_profile_cwd"));

        let decoded: FramedMessage = serde_json::from_value(encoded).expect("deserialize");
        let Message::SpawnRequest { payload } = decoded.message else {
            panic!("unexpected message");
        };
        assert_eq!(PermissionProfile::read_only(), payload.permission_profile);
        assert_eq!(workspace_roots, payload.workspace_roots);
        assert_eq!(
            Some("CodexSandboxDesktop-1234"),
            payload.private_desktop_name.as_deref()
        );
        assert_eq!(
            Some("S-1-5-21-100-200-300-400"),
            payload.network_proxy_restricting_sid.as_deref()
        );
    }

    #[test]
    fn error_payload_serializes_stage_and_windows_error_code() {
        let msg = FramedMessage {
            version: IPC_PROTOCOL_VERSION,
            message: Message::Error {
                payload: ErrorPayload {
                    message: "CreateProcessAsUserW failed".to_string(),
                    stage: ErrorStage::SpawnChild,
                    windows_error_code: Some(1312),
                },
            },
        };

        let encoded = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(
            serde_json::json!({
                "version": IPC_PROTOCOL_VERSION,
                "type": "error",
                "payload": {
                    "message": "CreateProcessAsUserW failed",
                    "stage": "spawn_child",
                    "windows_error_code": 1312,
                }
            }),
            encoded
        );
    }
}
