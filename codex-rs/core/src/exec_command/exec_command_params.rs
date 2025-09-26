use serde::Deserialize;
use serde::Serialize;

use crate::exec_command::session_id::SessionId;

#[derive(Debug, Clone, Deserialize)]
pub struct ExecCommandParams {
    pub(crate) cmd: String,

    #[serde(default = "default_yield_time")]
    pub(crate) yield_time_ms: u64,

    #[serde(default = "max_output_tokens")]
    pub(crate) max_output_tokens: u64,

    #[serde(default = "default_shell")]
    pub(crate) shell: String,

    #[serde(default = "default_login")]
    pub(crate) login: bool,

    #[serde(default)]
    pub(crate) idle_timeout_ms: Option<u64>,

    #[serde(default)]
    pub(crate) hard_timeout_ms: Option<u64>,

    #[serde(default = "default_grace_period_ms")]
    pub(crate) grace_period_ms: u64,

    #[serde(default = "default_log_threshold_bytes")]
    pub(crate) log_threshold_bytes: u64,
}

fn default_yield_time() -> u64 {
    10_000
}

fn max_output_tokens() -> u64 {
    10_000
}

fn default_login() -> bool {
    true
}

fn default_shell() -> String {
    "/bin/bash".to_string()
}

fn default_grace_period_ms() -> u64 {
    5_000
}

fn default_log_threshold_bytes() -> u64 {
    4 * 1024
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteStdinParams {
    pub(crate) session_id: SessionId,
    pub(crate) chars: String,

    #[serde(default = "write_stdin_default_yield_time_ms")]
    pub(crate) yield_time_ms: u64,

    #[serde(default = "write_stdin_default_max_output_tokens")]
    pub(crate) max_output_tokens: u64,
}

fn write_stdin_default_yield_time_ms() -> u64 {
    250
}

fn write_stdin_default_max_output_tokens() -> u64 {
    10_000
}
