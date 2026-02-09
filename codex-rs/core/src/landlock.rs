use crate::protocol::SandboxPolicy;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_network_proxy::PROXY_URL_ENV_KEYS;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

/// Spawn a shell tool command under the Linux sandbox helper
/// (codex-linux-sandbox), which currently uses bubblewrap for filesystem
/// isolation plus seccomp for network restrictions.
///
/// Unlike macOS Seatbelt where we directly embed the policy text, the Linux
/// helper accepts a list of `--sandbox-permission`/`-s` flags mirroring the
/// public CLI. We convert the internal [`SandboxPolicy`] representation into
/// the equivalent CLI options.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_command_under_linux_sandbox<P>(
    codex_linux_sandbox_exe: P,
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    use_bwrap_sandbox: bool,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child>
where
    P: AsRef<Path>,
{
    let args = create_linux_sandbox_command_args(
        command,
        sandbox_policy,
        sandbox_policy_cwd,
        use_bwrap_sandbox,
        !sandbox_policy.has_full_network_access() && has_proxy_env_vars(&env),
    );
    let arg0 = Some("codex-linux-sandbox");
    spawn_child_async(SpawnChildRequest {
        program: codex_linux_sandbox_exe.as_ref().to_path_buf(),
        args,
        arg0,
        cwd: command_cwd,
        sandbox_policy,
        network: None,
        stdio_policy,
        env,
    })
    .await
}

/// Converts the sandbox policy into the CLI invocation for `codex-linux-sandbox`.
///
/// The helper performs the actual sandboxing (bubblewrap + seccomp) after
/// parsing these arguments. See `docs/linux_sandbox.md` for the Linux semantics.
pub(crate) fn create_linux_sandbox_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    use_bwrap_sandbox: bool,
    allow_network_for_proxy: bool,
) -> Vec<String> {
    #[expect(clippy::expect_used)]
    let sandbox_policy_cwd = sandbox_policy_cwd
        .to_str()
        .expect("cwd must be valid UTF-8")
        .to_string();

    #[expect(clippy::expect_used)]
    let sandbox_policy_json =
        serde_json::to_string(sandbox_policy).expect("Failed to serialize SandboxPolicy to JSON");

    let mut linux_cmd: Vec<String> = vec![
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd,
        "--sandbox-policy".to_string(),
        sandbox_policy_json,
    ];
    if use_bwrap_sandbox {
        linux_cmd.push("--use-bwrap-sandbox".to_string());
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

fn has_proxy_env_vars(env: &HashMap<String, String>) -> bool {
    PROXY_URL_ENV_KEYS
        .iter()
        .any(|key| env.get(*key).is_some_and(|value| !value.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bwrap_flags_are_feature_gated() {
        let command = vec!["/bin/true".to_string()];
        let cwd = Path::new("/tmp");
        let policy = SandboxPolicy::ReadOnly;

        let with_bwrap =
            create_linux_sandbox_command_args(command.clone(), &policy, cwd, true, false);
        assert_eq!(
            with_bwrap.contains(&"--use-bwrap-sandbox".to_string()),
            true
        );

        let without_bwrap = create_linux_sandbox_command_args(command, &policy, cwd, false, false);
        assert_eq!(
            without_bwrap.contains(&"--use-bwrap-sandbox".to_string()),
            false
        );
    }

    #[test]
    fn proxy_flag_is_included_when_requested() {
        let command = vec!["/bin/true".to_string()];
        let cwd = Path::new("/tmp");
        let policy = SandboxPolicy::ReadOnly;

        let args = create_linux_sandbox_command_args(command, &policy, cwd, true, true);
        assert_eq!(
            args.contains(&"--allow-network-for-proxy".to_string()),
            true
        );
    }
}
