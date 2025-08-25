use reqwest::StatusCode;
use serde_json;
use std::io;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinError;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, CodexErr>;

#[derive(Error, Debug)]
pub enum SandboxErr {
    /// Error from sandbox execution
    #[error("sandbox denied exec error, exit code: {0}, stdout: {1}, stderr: {2}")]
    Denied(i32, String, String),

    /// Error from linux seccomp filter setup
    #[cfg(target_os = "linux")]
    #[error("seccomp setup error")]
    SeccompInstall(#[from] seccompiler::Error),

    /// Error from linux seccomp backend
    #[cfg(target_os = "linux")]
    #[error("seccomp backend error")]
    SeccompBackend(#[from] seccompiler::BackendError),

    /// Command timed out
    #[error("command timed out")]
    Timeout,

    /// Command was killed by a signal
    #[error("command was killed by a signal")]
    Signal(i32),

    /// Error from linux landlock
    #[error("Landlock was not able to fully enforce all sandbox rules")]
    LandlockRestrict,
}

#[derive(Error, Debug)]
pub enum CodexErr {
    /// Returned by ResponsesClient when the SSE stream disconnects or errors out **after** the HTTP
    /// handshake has succeeded but **before** it finished emitting `response.completed`.
    ///
    /// The Session loop treats this as a transient error and will automatically retry the turn.
    ///
    /// Optionally includes the requested delay before retrying the turn.
    #[error("stream disconnected before completion: {0}")]
    Stream(String, Option<Duration>),

    #[error("no conversation with id: {0}")]
    ConversationNotFound(Uuid),

    #[error("session configured event was not the first event in the stream")]
    SessionConfiguredNotFirstEvent,

    /// Returned by run_command_stream when the spawned child process timed out (10s).
    #[error("timeout waiting for child process to exit")]
    Timeout,

    /// Returned by run_command_stream when the child could not be spawned (its stdout/stderr pipes
    /// could not be captured). Analogous to the previous `CodexError::Spawn` variant.
    #[error("spawn failed: child stdout/stderr not captured")]
    Spawn,

    /// Returned by run_command_stream when the user pressed Ctrl‑C (SIGINT). Session uses this to
    /// surface a polite FunctionCallOutput back to the model instead of crashing the CLI.
    #[error("interrupted (Ctrl-C)")]
    Interrupted,

    /// Unexpected HTTP status code.
    #[error("unexpected status {0}: {1}")]
    UnexpectedStatus(StatusCode, String),

    #[error("{0}")]
    UsageLimitReached(UsageLimitReachedError),

    #[error(
        "To use Codex with your ChatGPT plan, upgrade to Plus: https://openai.com/chatgpt/pricing."
    )]
    UsageNotIncluded,

    #[error("We're currently experiencing high demand, which may cause temporary errors.")]
    InternalServerError,

    /// Retry limit exceeded.
    #[error("exceeded retry limit, last status: {0}")]
    RetryLimit(StatusCode),

    /// Agent loop died unexpectedly
    #[error("internal error; agent loop died unexpectedly")]
    InternalAgentDied,

    /// Sandbox error
    #[error("sandbox error: {0}")]
    Sandbox(#[from] SandboxErr),

    #[error("codex-linux-sandbox was required but not provided")]
    LandlockSandboxExecutableNotProvided,

    // -----------------------------------------------------------------
    // Automatic conversions for common external error types
    // -----------------------------------------------------------------
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockRuleset(#[from] landlock::RulesetError),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockPathFd(#[from] landlock::PathFdError),

    #[error(transparent)]
    TokioJoin(#[from] JoinError),

    #[error("{0}")]
    EnvVar(EnvVarError),
}

#[derive(Debug)]
pub struct UsageLimitReachedError {
    pub plan_type: Option<String>,
}

impl std::fmt::Display for UsageLimitReachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(plan_type) = &self.plan_type
            && plan_type == "plus"
        {
            write!(
                f,
                "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing), or wait for limits to reset (every 5h and every week.)."
            )?;
        } else {
            write!(
                f,
                "You've hit your usage limit. Limits reset every 5h and every week."
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EnvVarError {
    /// Name of the environment variable that is missing.
    pub var: String,

    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub instructions: Option<String>,
}

impl std::fmt::Display for EnvVarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing environment variable: `{}`.", self.var)?;
        if let Some(instructions) = &self.instructions {
            write!(f, " {instructions}")?;
        }
        Ok(())
    }
}

impl CodexErr {
    /// Minimal shim so that existing `e.downcast_ref::<CodexErr>()` checks continue to compile
    /// after replacing `anyhow::Error` in the return signature. This mirrors the behavior of
    /// `anyhow::Error::downcast_ref` but works directly on our concrete enum.
    pub fn downcast_ref<T: std::any::Any>(&self) -> Option<&T> {
        (self as &dyn std::any::Any).downcast_ref::<T>()
    }
}

pub fn get_error_message_ui(e: &CodexErr) -> String {
    match e {
        CodexErr::Sandbox(SandboxErr::Denied(_, _, stderr)) => stderr.to_string(),
        // Timeouts are not sandbox errors from a UX perspective; present them plainly
        CodexErr::Sandbox(SandboxErr::Timeout) => "error: command timed out".to_string(),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_limit_reached_error_formats_plus_plan() {
        let err = UsageLimitReachedError {
            plan_type: Some("plus".to_string()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing), or wait for limits to reset (every 5h and every week.)."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_when_none() {
        let err = UsageLimitReachedError { plan_type: None };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Limits reset every 5h and every week."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_for_other_plans() {
        let err = UsageLimitReachedError {
            plan_type: Some("pro".to_string()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Limits reset every 5h and every week."
        );
    }
}
