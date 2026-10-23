//! Owner-admitted hooks bound to the exact environment that supplied them.

use std::collections::HashMap;
use std::time::Duration;

use codex_protocol::hooks::HookEventsToml;
use codex_utils_path_uri::PathUri;
use futures::future::BoxFuture;

/// An admitted requirements source. The owner must not populate this from user configuration.
pub struct EnvironmentHookSource {
    pub environment_id: String,
    /// Working directory from the same captured environment used for execution.
    pub cwd: PathUri,
    pub hooks: HookEventsToml,
}

/// Executes commands in ready environments without coupling the hooks engine to core or transports.
/// Implementations must honor the timeout, terminate timed-out processes, and bound captured output.
pub trait HookEnvironmentExecutor: Send + Sync {
    fn execute(
        &self,
        environment_id: &str,
        command: &str,
        env: &HashMap<String, String>,
        input_json: &str,
        timeout: Duration,
    ) -> BoxFuture<'_, anyhow::Result<EnvironmentHookOutput>>;
}

pub struct EnvironmentHookOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
