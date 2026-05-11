use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use ts_rs::TS;

/// PTY size in character cells for `process/spawn` PTY sessions.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessTerminalSize {
    /// Terminal height in character cells.
    pub rows: u16,
    /// Terminal width in character cells.
    pub cols: u16,
}

/// Spawn a standalone process (argv vector) without a Codex sandbox on the host
/// where the app server is running.
///
/// `process/spawn` returns after the process has started and the connection-scoped
/// `processHandle` has been registered. Process output and exit are reported via
/// `process/outputDelta` and `process/exited` notifications.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessSpawnParams {
    /// Command argv vector. Empty arrays are rejected.
    pub command: Vec<String>,
    /// Client-supplied, connection-scoped process handle.
    ///
    /// Duplicate active handles are rejected on the same connection. The same
    /// handle can be reused after the prior process exits.
    pub process_handle: String,
    /// Absolute working directory for the process.
    pub cwd: AbsolutePathBuf,
    /// Enable PTY mode.
    ///
    /// This implies `streamStdin` and `streamStdoutStderr`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tty: bool,
    /// Allow follow-up `process/writeStdin` requests to write stdin bytes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream_stdin: bool,
    /// Stream stdout/stderr via `process/outputDelta` notifications.
    ///
    /// Streamed bytes are not duplicated into the `process/exited` notification.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream_stdout_stderr: bool,
    /// Optional per-stream stdout/stderr capture cap in bytes.
    ///
    /// When omitted, the server default applies. Set to `null` to disable the
    /// cap.
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub output_bytes_cap: Option<Option<usize>>,
    /// Optional timeout in milliseconds.
    ///
    /// When omitted, the server default applies. Set to `null` to disable the
    /// timeout.
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub timeout_ms: Option<Option<i64>>,
    /// Optional environment overrides merged into the app-server process
    /// environment.
    ///
    /// Matching names override inherited values. Set a key to `null` to unset
    /// an inherited variable.
    #[ts(optional = nullable)]
    pub env: Option<HashMap<String, Option<String>>>,
    /// Optional initial PTY size in character cells. Only valid when `tty` is
    /// true.
    #[ts(optional = nullable)]
    pub size: Option<ProcessTerminalSize>,
}

/// Successful response for `process/spawn`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessSpawnResponse {}

/// Write stdin bytes to a running `process/spawn` session, close stdin, or
/// both.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessWriteStdinParams {
    /// Client-supplied, connection-scoped `processHandle` from `process/spawn`.
    pub process_handle: String,
    /// Optional base64-encoded stdin bytes to write.
    #[ts(optional = nullable)]
    pub delta_base64: Option<String>,
    /// Close stdin after writing `deltaBase64`, if present.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub close_stdin: bool,
}

/// Empty success response for `process/writeStdin`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessWriteStdinResponse {}

/// Terminate a running `process/spawn` session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessKillParams {
    /// Client-supplied, connection-scoped `processHandle` from `process/spawn`.
    pub process_handle: String,
}

/// Empty success response for `process/kill`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessKillResponse {}

/// Resize a running PTY-backed `process/spawn` session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessResizePtyParams {
    /// Client-supplied, connection-scoped `processHandle` from `process/spawn`.
    pub process_handle: String,
    /// New PTY size in character cells.
    pub size: ProcessTerminalSize,
}

/// Empty success response for `process/resizePty`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessResizePtyResponse {}

/// Stream label for `process/outputDelta` notifications.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ProcessOutputStream {
    /// stdout stream. PTY mode multiplexes terminal output here.
    Stdout,
    /// stderr stream.
    Stderr,
}

/// Base64-encoded output chunk emitted for a streaming `process/spawn` request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessOutputDeltaNotification {
    /// Client-supplied, connection-scoped `processHandle` from `process/spawn`.
    pub process_handle: String,
    /// Output stream this chunk belongs to.
    pub stream: ProcessOutputStream,
    /// Base64-encoded output bytes.
    pub delta_base64: String,
    /// True on the final streamed chunk for this stream when output was
    /// truncated by `outputBytesCap`.
    pub cap_reached: bool,
}

/// Final process exit notification for `process/spawn`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ProcessExitedNotification {
    /// Client-supplied, connection-scoped `processHandle` from `process/spawn`.
    pub process_handle: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Buffered stdout capture.
    ///
    /// Empty when stdout was streamed via `process/outputDelta`.
    pub stdout: String,
    /// Whether stdout reached `outputBytesCap`.
    ///
    /// In streaming mode, stdout is empty and cap state is also reported on the
    /// final stdout `process/outputDelta` notification.
    pub stdout_cap_reached: bool,
    /// Buffered stderr capture.
    ///
    /// Empty when stderr was streamed via `process/outputDelta`.
    pub stderr: String,
    /// Whether stderr reached `outputBytesCap`.
    ///
    /// In streaming mode, stderr is empty and cap state is also reported on the
    /// final stderr `process/outputDelta` notification.
    pub stderr_cap_reached: bool,
}
