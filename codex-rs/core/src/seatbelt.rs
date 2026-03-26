#![cfg(target_os = "macos")]

use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_network_proxy::NetworkProxy;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use codex_sandboxing::seatbelt::create_seatbelt_command_args_for_policies_with_extensions;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    network: Option<&NetworkProxy>,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args_for_policies_with_extensions(
        command,
        &FileSystemSandboxPolicy::from_legacy_sandbox_policy(sandbox_policy, sandbox_policy_cwd),
        NetworkSandboxPolicy::from(sandbox_policy),
        sandbox_policy_cwd,
        /*enforce_managed_network*/ false,
        network,
        /*extensions*/ None,
    );
    let arg0 = None;
    env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(SpawnChildRequest {
        program: PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        cwd: command_cwd,
        network_sandbox_policy: NetworkSandboxPolicy::from(sandbox_policy),
        network,
        stdio_policy,
        env,
    })
    .await
}
