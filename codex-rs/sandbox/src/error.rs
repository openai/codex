use crate::exec::ExecToolCallOutput;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CodexErr>;

#[derive(Error, Debug)]
pub enum SandboxErr {
    #[error(
        "sandbox denied exec error, exit code: {}, stdout: {}, stderr: {}",
        .output.exit_code, .output.stdout.text, .output.stderr.text
    )]
    Denied { output: Box<ExecToolCallOutput> },

    #[cfg(target_os = "linux")]
    #[error("seccomp setup error")]
    SeccompInstall(#[from] seccompiler::Error),

    #[cfg(target_os = "linux")]
    #[error("seccomp backend error")]
    SeccompBackend(#[from] seccompiler::BackendError),

    #[error("command timed out")]
    Timeout { output: Box<ExecToolCallOutput> },

    #[error("command was killed by a signal")]
    Signal(i32),

    #[error("Landlock was not able to fully enforce all sandbox rules")]
    LandlockRestrict,
}

#[derive(Error, Debug)]
pub enum CodexErr {
    #[error("sandbox error: {0}")]
    Sandbox(#[from] SandboxErr),

    #[error("codex-linux-sandbox was required but not provided")]
    LandlockSandboxExecutableNotProvided,

    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockRuleset(#[from] landlock::RulesetError),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockPathFd(#[from] landlock::PathFdError),
}
