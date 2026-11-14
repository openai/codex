//! Keep opaque sandbox policy separate from the launcher used to apply it.
//! Both state names carry identical values during migration; neither contains
//! executable discovery data or deprecation messages.

use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::path::PathBuf;

use crate::McpConfig;
use crate::McpServerKey;

/// Servers advertise this experimental capability to receive opaque sandbox state
/// in tool-call `_meta`, along with a local launcher when one is available.
pub const MCP_SANDBOX_STATE_CAPABILITY: &str = "codex/sandbox-state";
/// Deprecated capability and metadata spelling, retained as an identical alias.
/// Deprecation guidance is outside the opaque policy object.
pub const MCP_SANDBOX_STATE_META_CAPABILITY: &str = "codex/sandbox-state-meta";
/// Tool-call metadata describing a sandbox launcher on the MCP server's host.
pub const MCP_SANDBOX_LAUNCHER_META_KEY: &str = "codex/sandbox-launcher";

/// Opaque policy forwarded unchanged to `codex sandbox --sandbox-state-json`.
///
/// Servers advertise `codex/sandbox-state` (or its deprecated `-meta` alias) to
/// receive the same object under both state keys. Consumers must preserve unknown
/// fields, not deserialize and reconstruct the policy. Launcher information lives
/// separately under `codex/sandbox-launcher`; only its documented fields may be
/// interpreted. Long-lived subprocesses retain both their state and launcher and
/// must not be reused under different authority without a lifecycle decision.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    pub permission_profile: PermissionProfile,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub sandbox_cwd: PathUri,
    #[serde(default)]
    pub use_legacy_landlock: bool,
    #[serde(default)]
    pub use_mxc: bool,
}

/// The known launcher envelope is distinct from the opaque policy. Absence never
/// authorizes searching PATH or falling back to an unsandboxed process.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SandboxLauncher {
    /// A full CLI on the MCP server's host, supporting `sandbox --sandbox-state-json`.
    /// Remote/HTTP servers and hosts without a verified, UTF-8 CLI path omit the envelope.
    codex_executable: AbsolutePathBuf,
}

/// Return a verified CLI on the same host and filesystem as a local stdio server.
pub(crate) async fn sandbox_codex_executable(
    config: &McpConfig,
    server_key: &McpServerKey,
    environment_id: &str,
) -> Option<AbsolutePathBuf> {
    let server = config.mcp_server_catalog.server(server_key)?.config();
    if environment_id != codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID
        || !server.is_local_environment()
        || !matches!(
            server.transport,
            codex_config::McpServerTransportConfig::Stdio { .. }
        )
    {
        return None;
    }

    let executable = config
        .codex_self_exe
        .as_deref()
        .filter(|path| path.is_absolute() && path.to_str().is_some())?;
    let executable = AbsolutePathBuf::from_absolute_path_checked(executable).ok()?;
    crate::sandbox_executable::supports_sandbox_state(&executable)
        .await
        .then_some(executable)
}

pub(crate) fn augment_metadata(
    meta: &mut Option<Value>,
    state: SandboxState,
    executable: Option<AbsolutePathBuf>,
) -> anyhow::Result<()> {
    let state = serde_json::to_value(state)?;
    let launcher = executable
        .map(|codex_executable| serde_json::to_value(SandboxLauncher { codex_executable }))
        .transpose()?;
    let Some(metadata) = meta
        .get_or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
    else {
        return Ok(());
    };

    metadata.insert(MCP_SANDBOX_STATE_CAPABILITY.to_owned(), state.clone());
    metadata.insert(MCP_SANDBOX_STATE_META_CAPABILITY.to_owned(), state);
    // Reserved launcher metadata must not survive from a previous/local call
    // when this server's execution environment has no verified executable.
    metadata.remove(MCP_SANDBOX_LAUNCHER_META_KEY);
    if let Some(launcher) = launcher {
        metadata.insert(MCP_SANDBOX_LAUNCHER_META_KEY.to_owned(), launcher);
    }
    Ok(())
}

#[cfg(test)]
#[path = "sandbox_metadata_tests.rs"]
mod tests;
