use crate::exec::ExecToolCallOutput;
use crate::token_data::KnownPlan;
use crate::token_data::PlanType;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::protocol::RateLimitSnapshot;
use reqwest::StatusCode;
use serde_json;
use std::io;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinError;

pub type Result<T> = std::result::Result<T, CodexErr>;

#[derive(Error, Debug)]
pub enum SandboxErr {
    /// Error from sandbox execution
    #[error(
        "sandbox denied exec error, exit code: {}, stdout: {}, stderr: {}",
        .output.exit_code, .output.stdout.text, .output.stderr.text
    )]
    Denied { output: Box<ExecToolCallOutput> },

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
    Timeout { output: Box<ExecToolCallOutput> },

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
    ConversationNotFound(ConversationId),

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

    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),

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
    pub(crate) plan_type: Option<PlanType>,
    pub(crate) resets_in_seconds: Option<u64>,
    pub(crate) rate_limits: Option<RateLimitSnapshot>,
}

impl std::fmt::Display for UsageLimitReachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self.plan_type.as_ref() {
            Some(PlanType::Known(KnownPlan::Plus)) => format!(
                "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing){}",
                retry_suffix_after_or(self.resets_in_seconds)
            ),
            Some(PlanType::Known(KnownPlan::Team)) | Some(PlanType::Known(KnownPlan::Business)) => {
                format!(
                    "You've hit your usage limit. To get more access now, send a request to your admin{}",
                    retry_suffix_after_or(self.resets_in_seconds)
                )
            }
            Some(PlanType::Known(KnownPlan::Free)) => {
                "You've hit your usage limit. Upgrade to Plus to continue using Codex (https://openai.com/chatgpt/pricing)."
                    .to_string()
            }
            Some(PlanType::Known(KnownPlan::Pro))
            | Some(PlanType::Known(KnownPlan::Enterprise))
            | Some(PlanType::Known(KnownPlan::Edu)) => format!(
                "You've hit your usage limit.{}",
                retry_suffix(self.resets_in_seconds)
            ),
            Some(PlanType::Unknown(_)) | None => format!(
                "You've hit your usage limit.{}",
                retry_suffix(self.resets_in_seconds)
            ),
        };

        write!(f, "{message}")
    }
}

fn retry_suffix(resets_in_seconds: Option<u64>) -> String {
    if let Some(secs) = resets_in_seconds {
        let reset_duration = format_reset_duration(secs);
        format!(" Try again in {reset_duration}.")
    } else {
        " Try again later.".to_string()
    }
}

fn retry_suffix_after_or(resets_in_seconds: Option<u64>) -> String {
    if let Some(secs) = resets_in_seconds {
        let reset_duration = format_reset_duration(secs);
        format!(" or try again in {reset_duration}.")
    } else {
        " or try again later.".to_string()
    }
}

fn format_reset_duration(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;

    let mut parts: Vec<String> = Vec::new();
    if days > 0 {
        let unit = if days == 1 { "day" } else { "days" };
        parts.push(format!("{days} {unit}"));
    }
    if hours > 0 {
        let unit = if hours == 1 { "hour" } else { "hours" };
        parts.push(format!("{hours} {unit}"));
    }
    if minutes > 0 {
        let unit = if minutes == 1 { "minute" } else { "minutes" };
        parts.push(format!("{minutes} {unit}"));
    }

    if parts.is_empty() {
        return "less than a minute".to_string();
    }

    match parts.len() {
        1 => parts[0].clone(),
        2 => format!("{} {}", parts[0], parts[1]),
        _ => format!("{} {} {}", parts[0], parts[1], parts[2]),
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
        CodexErr::Sandbox(SandboxErr::Denied { output }) => output.stderr.text.clone(),
        // Timeouts are not sandbox errors from a UX perspective; present them plainly
        CodexErr::Sandbox(SandboxErr::Timeout { output }) => format!(
            "error: command timed out after {} ms",
            output.duration.as_millis()
        ),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::RateLimitWindow;

    fn rate_limit_snapshot() -> RateLimitSnapshot {
        RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 50.0,
                window_minutes: Some(60),
                resets_in_seconds: Some(3600),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 30.0,
                window_minutes: Some(120),
                resets_in_seconds: Some(7200),
            }),
        }
    }

    #[test]
    fn usage_limit_reached_error_formats_plus_plan() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Plus)),
            resets_in_seconds: None,
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again later."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_free_plan() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Free)),
            resets_in_seconds: Some(3600),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Plus to continue using Codex (https://openai.com/chatgpt/pricing)."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_when_none() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: None,
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again later."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_team_plan() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Team)),
            resets_in_seconds: Some(3600),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. To get more access now, send a request to your admin or try again in 1 hour."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_business_plan_without_reset() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Business)),
            resets_in_seconds: None,
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. To get more access now, send a request to your admin or try again later."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_for_other_plans() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Pro)),
            resets_in_seconds: None,
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again later."
        );
    }

    #[test]
    fn usage_limit_reached_includes_minutes_when_available() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(5 * 60),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in 5 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_includes_hours_and_minutes() {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Plus)),
            resets_in_seconds: Some(3 * 3600 + 32 * 60),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again in 3 hours 32 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_includes_days_hours_minutes() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(2 * 86_400 + 3 * 3600 + 5 * 60),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in 2 days 3 hours 5 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_less_than_minute() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(30),
            rate_limits: Some(rate_limit_snapshot()),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in less than a minute."
        );
    }
}
