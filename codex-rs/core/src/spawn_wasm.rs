use codex_network_proxy::NetworkProxy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::collections::HashMap;
use std::path::PathBuf;

pub const CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "CODEX_SANDBOX_NETWORK_DISABLED";
pub const CODEX_SANDBOX_ENV_VAR: &str = "CODEX_SANDBOX";

#[derive(Debug, Clone, Copy)]
pub enum StdioPolicy {
    RedirectForShellTool,
    Inherit,
}

pub(crate) struct SpawnChildRequest<'a> {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub arg0: Option<&'a str>,
    pub cwd: PathBuf,
    pub network_sandbox_policy: NetworkSandboxPolicy,
    pub network: Option<&'a NetworkProxy>,
    pub stdio_policy: StdioPolicy,
    pub env: HashMap<String, String>,
}

pub(crate) async fn spawn_child_async(_request: SpawnChildRequest<'_>) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "process spawning is unavailable in wasm",
    ))
}
