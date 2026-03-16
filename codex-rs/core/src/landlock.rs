use crate::protocol::SandboxPolicy;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_network_proxy::NetworkProxy;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

/// Spawn a shell tool command under the Linux sandbox helper
/// (codex-linux-sandbox), which defaults to bubblewrap for filesystem
/// isolation plus seccomp for network restrictions.
///
/// Unlike macOS Seatbelt where we directly embed the policy text, the Linux
/// helper is a separate executable. We pass the legacy [`SandboxPolicy`] plus
/// split filesystem/network policies as JSON so the helper can migrate
/// incrementally without breaking older call sites.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_command_under_linux_sandbox<P>(
    codex_linux_sandbox_exe: P,
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    use_legacy_landlock: bool,
    stdio_policy: StdioPolicy,
    network: Option<&NetworkProxy>,
    env: HashMap<String, String>,
) -> std::io::Result<Child>
where
    P: AsRef<Path>,
{
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::from_legacy_sandbox_policy(sandbox_policy, sandbox_policy_cwd);
    let network_sandbox_policy = NetworkSandboxPolicy::from(sandbox_policy);
    let args = create_linux_sandbox_command_args_for_policies(
        command,
        command_cwd.as_path(),
        sandbox_policy,
        &file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_policy_cwd,
        use_legacy_landlock,
        allow_network_for_proxy(false),
    );
    let arg0 = Some("codex-linux-sandbox");
    spawn_child_async(SpawnChildRequest {
        program: codex_linux_sandbox_exe.as_ref().to_path_buf(),
        args,
        arg0,
        cwd: command_cwd,
        network_sandbox_policy,
        network,
        stdio_policy,
        env,
    })
    .await
}

pub(crate) fn allow_network_for_proxy(enforce_managed_network: bool) -> bool {
    // When managed network requirements are active, request proxy-only
    // networking from the Linux sandbox helper. Without managed requirements,
    // preserve existing behavior.
    enforce_managed_network
}

/// Converts the sandbox policies into the CLI invocation for
/// `codex-linux-sandbox`.
///
/// The helper performs the actual sandboxing (bubblewrap by default + seccomp) after
/// parsing these arguments. Policy JSON flags are emitted before helper feature
/// flags so the argv order matches the helper's CLI shape. See
/// `docs/linux_sandbox.md` for the Linux semantics.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_linux_sandbox_command_args_for_policies(
    command: Vec<String>,
    command_cwd: &Path,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_policy_cwd: &Path,
    use_legacy_landlock: bool,
    allow_network_for_proxy: bool,
) -> Vec<String> {
    let sandbox_policy_json = serde_json::to_string(sandbox_policy)
        .unwrap_or_else(|err| panic!("failed to serialize sandbox policy: {err}"));
    let file_system_policy_json = serde_json::to_string(file_system_sandbox_policy)
        .unwrap_or_else(|err| panic!("failed to serialize filesystem sandbox policy: {err}"));
    let network_policy_json = serde_json::to_string(&network_sandbox_policy)
        .unwrap_or_else(|err| panic!("failed to serialize network sandbox policy: {err}"));
    let sandbox_policy_cwd = sandbox_policy_cwd
        .to_str()
        .unwrap_or_else(|| panic!("cwd must be valid UTF-8"))
        .to_string();
    let command_cwd = command_cwd
        .to_str()
        .unwrap_or_else(|| panic!("command cwd must be valid UTF-8"))
        .to_string();

    let mut linux_cmd: Vec<String> = vec![
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd,
        "--command-cwd".to_string(),
        command_cwd,
        "--sandbox-policy".to_string(),
        sandbox_policy_json,
        "--file-system-sandbox-policy".to_string(),
        file_system_policy_json,
        "--network-sandbox-policy".to_string(),
        network_policy_json,
    ];
    if use_legacy_landlock {
        linux_cmd.push("--use-legacy-landlock".to_string());
    }
    if allow_network_for_proxy {
        linux_cmd.push("--allow-network-for-proxy".to_string());
    }
    linux_cmd.push("--".to_string());
    linux_cmd.extend(command);
    linux_cmd
}

/// Converts the sandbox cwd and execution options into the CLI invocation for
/// `codex-linux-sandbox`.
#[cfg(test)]
pub(crate) fn create_linux_sandbox_command_args(
    command: Vec<String>,
    command_cwd: &Path,
    sandbox_policy_cwd: &Path,
    use_legacy_landlock: bool,
    allow_network_for_proxy: bool,
) -> Vec<String> {
    let command_cwd = command_cwd
        .to_str()
        .unwrap_or_else(|| panic!("command cwd must be valid UTF-8"))
        .to_string();
    let sandbox_policy_cwd = sandbox_policy_cwd
        .to_str()
        .unwrap_or_else(|| panic!("cwd must be valid UTF-8"))
        .to_string();

    let mut linux_cmd: Vec<String> = vec![
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd,
        "--command-cwd".to_string(),
        command_cwd,
    ];
    if use_legacy_landlock {
        linux_cmd.push("--use-legacy-landlock".to_string());
    }
    if allow_network_for_proxy {
        linux_cmd.push("--allow-network-for-proxy".to_string());
    }

    // Separator so that command arguments starting with `-` are not parsed as
    // options of the helper itself.
    linux_cmd.push("--".to_string());

    // Append the original tool command.
    linux_cmd.extend(command);

    linux_cmd
}

#[cfg(test)]
#[path = "landlock_tests.rs"]
mod tests;
