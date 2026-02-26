use std::collections::HashMap;
use std::os::fd::RawFd;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

/// Exec wrappers read this to find the inherited FD for the escalation socket.
pub const ESCALATE_SOCKET_ENV_VAR: &str = "CODEX_ESCALATE_SOCKET";

/// Patched shells use this to wrap exec() calls.
pub const EXEC_WRAPPER_ENV_VAR: &str = "EXEC_WRAPPER";

/// Compatibility alias for older patched bash builds.
pub const LEGACY_BASH_EXEC_WRAPPER_ENV_VAR: &str = "BASH_EXEC_WRAPPER";

/// The client sends this to the server to request an exec() call.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct EscalateRequest {
    /// The executable path from the intercepted exec call.
    ///
    /// This may be relative, in which case it should be resolved against
    /// `workdir`.
    pub file: PathBuf,
    /// The argv, including the program name (argv[0]).
    pub argv: Vec<String>,
    pub workdir: AbsolutePathBuf,
    pub env: HashMap<String, String>,
}

/// The server sends this to the client to respond to an exec() request.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct EscalateResponse {
    pub action: EscalateAction,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum EscalateAction {
    /// The command should be run directly by the client.
    Run,
    /// The command should be escalated to the server for execution.
    Escalate,
    /// The command should not be executed.
    Deny { reason: Option<String> },
}

/// The client sends this to the server to forward its open FDs.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SuperExecMessage {
    pub fds: Vec<RawFd>,
}

/// The server responds when the exec()'d command has exited.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SuperExecResult {
    pub exit_code: i32,
}
