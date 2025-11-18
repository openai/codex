use std::collections::HashMap;
use std::os::fd::RawFd;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// 'exec-server escalate' reads this to find the inherited FD for the escalate socket.
pub(super) const ESCALATE_SOCKET_ENV_VAR: &str = "CODEX_ESCALATE_SOCKET";

/// The patched bash uses this to wrap exec() calls.
pub(super) const BASH_EXEC_WRAPPER_ENV_VAR: &str = "BASH_EXEC_WRAPPER";

// C->S on the escalate socket
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(super) enum EscalateClientMessage {
    /// The client wants to run exec() with the given arguments.
    EscalateRequest {
        /// The absolute path to the executable to run, i.e. the first arg to exec.
        file: String,
        /// The argv, including the program name (argv[0]).
        argv: Vec<String>,
        workdir: PathBuf,
        env: HashMap<String, String>,
    },
}

// C->S on the escalate socket
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(super) enum EscalateServerMessage {
    EscalateResponse(EscalateAction),
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(super) enum EscalateAction {
    RunInSandbox,
    Escalate,
}

// C->S on the super-exec socket
#[derive(Clone, Serialize, Deserialize, Debug)]
pub(super) struct SuperExecMessage {
    pub(super) fds: Vec<RawFd>,
}

// S->C on the super-exec socket
#[derive(Clone, Serialize, Deserialize, Debug)]
pub(super) struct SuperExecResult {
    pub(super) exit_code: i32,
}
