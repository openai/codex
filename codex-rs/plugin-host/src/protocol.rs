//! Narrow experimental ABI. The child supplies correlation IDs, never authority or thread IDs.

use serde::Deserialize;
use serde::Serialize;

// Leave room for worst-case JSON escaping and the packet envelope within MAX_FRAME.
pub const MAX_SOURCE: usize = 128 * 1024;
pub const MAX_RESULT: usize = 64 * 1024;
pub const MAX_METHOD: usize = 1024;
pub const MAX_PENDING_PROMISES: usize = 1024;
pub const MAX_IN_FLIGHT_TURNS: usize = 8;
pub const MAX_SCRIPT_ERROR_BYTES: usize = 16 * 1024;
const _: () = assert!(MAX_SOURCE * 6 + 1024 <= crate::framing::MAX_FRAME);

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet<T> {
    pub instance: u64,
    pub message: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum HostMessage {
    Initialize {
        source: String,
    },
    Dispatch {
        id: u64,
        export: String,
        #[serde(rename = "returnMode")]
        return_mode: ReturnMode,
    },
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum RuntimeMessage {
    Initialized,
    Pending {
        id: u64,
    },
    Reply {
        id: u64,
        value: String,
    },
    Error {
        code: ErrorCode,
        #[serde(rename = "scriptError")]
        script_error: Option<PluginScriptError>,
    },
    Closed,
}

/// Original script error text and source location. Callers must sanitize text for display.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginScriptError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub truncated: bool,
}

impl std::fmt::Debug for PluginScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PluginScriptError([redacted])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReturnMode {
    Sync,
    MaybeAsync,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidProgram,
    InvalidResult,
    ExecutionFailed,
    ExecutionTimedOut,
    QuotaExceeded,
    InstanceClosed,
}
