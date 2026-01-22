use crate::features::Feature;
use crate::features::Features;
use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::process::Child;
use tracing::warn;

static BIND_MOUNT_PROBE_RESULT: OnceLock<bool> = OnceLock::new();

/// Spawn a shell tool command under the Linux Landlock+seccomp sandbox helper
/// (codex-linux-sandbox).
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
    use_linux_sandbox_bind_mounts: bool,
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
        use_linux_sandbox_bind_mounts,
    );
    let arg0 = Some("codex-linux-sandbox");
    spawn_child_async(
        codex_linux_sandbox_exe.as_ref().to_path_buf(),
        args,
        arg0,
        command_cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

/// Returns whether bind-mount protections should be enabled on this host.
/// The probe is cached so we only run it once per process.
pub fn resolve_use_linux_sandbox_bind_mounts(
    features: &Features,
    codex_linux_sandbox_exe: Option<&PathBuf>,
) -> bool {
    if !features.enabled(Feature::LinuxSandboxBindMounts) {
        return false;
    }
    if !cfg!(target_os = "linux") {
        return false;
    }
    let Some(exe) = codex_linux_sandbox_exe else {
        return false;
    };
    *BIND_MOUNT_PROBE_RESULT.get_or_init(|| run_bind_mount_probe(exe.as_path()))
}

fn run_bind_mount_probe(exe: &Path) -> bool {
    let status = Command::new(exe)
        .arg("--probe-bind-mounts")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(status) => status.success(),
        Err(err) => {
            warn!("failed to run codex-linux-sandbox --probe-bind-mounts: {err}");
            false
        }
    }
}

/// Converts the sandbox policy into the CLI invocation for `codex-linux-sandbox`.
pub(crate) fn create_linux_sandbox_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    use_linux_sandbox_bind_mounts: bool,
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

    if use_linux_sandbox_bind_mounts {
        linux_cmd.push("--enable-bind-mounts".to_string());
    }

    // Separator so that command arguments starting with `-` are not parsed as
    // options of the helper itself.
    linux_cmd.push("--".to_string());

    // Append the original tool command.
    linux_cmd.extend(command);

    linux_cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[cfg(target_os = "linux")]
    #[test]
    fn run_bind_mount_probe_reports_success_and_failure() {
        let (_dir_ok, exe_ok) = make_probe_script(0);
        assert!(run_bind_mount_probe(&exe_ok));

        let (_dir_fail, exe_fail) = make_probe_script(1);
        assert!(!run_bind_mount_probe(&exe_fail));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolve_use_linux_sandbox_bind_mounts_caches_probe() {
        let mut features = Features::with_defaults();
        features.enable(Feature::LinuxSandboxBindMounts);

        let (_dir_ok, exe_ok) = make_probe_script(0);
        assert!(resolve_use_linux_sandbox_bind_mounts(
            &features,
            Some(&exe_ok)
        ));

        let (_dir_fail, exe_fail) = make_probe_script(1);
        assert_eq!(
            resolve_use_linux_sandbox_bind_mounts(&features, Some(&exe_fail)),
            true
        );
    }

    #[cfg(target_os = "linux")]
    fn make_probe_script(exit_code: i32) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("probe.sh");
        std::fs::write(&path, format!("#!/bin/sh\nexit {exit_code}\n"))
            .expect("write probe script");
        let mut perms = std::fs::metadata(&path)
            .expect("probe metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("set probe permissions");
        (dir, path)
    }
}
