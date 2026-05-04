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
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

/// Safety cap for a single framed message payload.
///
/// This is not a protocol requirement; it simply bounds memory use and rejects
/// obviously invalid frames.
const MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

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
    pub policy_json_or_preset: String,
    pub sandbox_policy_cwd: PathBuf,
    pub codex_home: PathBuf,
    pub real_codex_home: PathBuf,
    pub cap_sids: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub tty: bool,
    #[serde(default)]
    pub stdin_open: bool,
    #[serde(default)]
    pub use_private_desktop: bool,
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
    pub code: String,
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
pub fn write_frame<W: Write>(mut writer: W, msg: &FramedMessage) -> Result<()> {
    let payload = serde_json::to_vec(msg)?;
    if payload.len() > MAX_FRAME_LEN {
        anyhow::bail!("frame too large: {}", payload.len());
    }
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Read a length-prefixed JSON frame; returns `Ok(None)` on EOF.
pub fn read_frame<R: Read>(mut reader: R) -> Result<Option<FramedMessage>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        anyhow::bail!("frame too large: {len}");
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    let msg: FramedMessage = serde_json::from_slice(&payload)?;
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_round_trip() {
        let msg = FramedMessage {
            version: 1,
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
        assert_eq!(decoded.version, 1);
        match decoded.message {
            Message::Output { payload } => {
                assert_eq!(payload.stream, OutputStream::Stdout);
                let data = decode_bytes(&payload.data_b64).expect("decode");
                assert_eq!(data, b"hello");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }
}
