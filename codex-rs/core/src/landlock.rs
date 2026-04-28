use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_network_proxy::NetworkProxy;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::compatibility_sandbox_policy_for_permission_profile;
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_sandboxing::landlock::allow_network_for_proxy;
use codex_sandboxing::landlock::create_linux_sandbox_command_args_for_policies;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Child;

/// Spawn a shell tool command under the Linux sandbox helper
/// (codex-linux-sandbox), which defaults to bubblewrap for filesystem
/// isolation plus seccomp for network restrictions.
///
/// Unlike macOS Seatbelt where we directly embed the policy text, the Linux
/// helper is a separate executable. We pass both the canonical split
/// filesystem/network policies and a compatibility legacy projection as JSON
/// until the helper protocol no longer needs the legacy field.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_command_under_linux_sandbox<P>(
    codex_linux_sandbox_exe: P,
    command: Vec<String>,
    command_cwd: AbsolutePathBuf,
    permission_profile: &PermissionProfile,
    sandbox_policy_cwd: &AbsolutePathBuf,
    use_legacy_landlock: bool,
    stdio_policy: StdioPolicy,
    network: Option<&NetworkProxy>,
    env: HashMap<String, String>,
) -> std::io::Result<Child>
where
    P: AsRef<Path>,
{
    let (file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();
    let sandbox_policy = compatibility_sandbox_policy_for_permission_profile(
        permission_profile,
        &file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_policy_cwd.as_path(),
    );
    let args = create_linux_sandbox_command_args_for_policies(
        command,
        command_cwd.as_path(),
        &sandbox_policy,
        &file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_policy_cwd,
        use_legacy_landlock,
        allow_network_for_proxy(/*enforce_managed_network*/ false),
    );
    let codex_linux_sandbox_exe = codex_linux_sandbox_exe.as_ref();
    // Preserve the helper alias when we already have it; otherwise force argv0
    // so arg0 dispatch still reaches the Linux sandbox path.
    let arg0 = if codex_linux_sandbox_exe
        .file_name()
        .and_then(|name| name.to_str())
        == Some(CODEX_LINUX_SANDBOX_ARG0)
    {
        // Old bubblewrap builds without `--argv0` need a real helper path whose
        // basename still dispatches to the Linux sandbox entrypoint.
        codex_linux_sandbox_exe.to_string_lossy().into_owned()
    } else {
        CODEX_LINUX_SANDBOX_ARG0.to_string()
    };
    spawn_child_async(SpawnChildRequest {
        program: codex_linux_sandbox_exe.to_path_buf(),
        args,
        arg0: Some(&arg0),
        cwd: command_cwd,
        network_sandbox_policy,
        network,
        stdio_policy,
        env,
    })
    .await
}
