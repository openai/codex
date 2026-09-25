//! End-to-end checks that only executor startup can opt into PID inheritance.

use super::*;
use codex_exec_server::FileSystemSandboxContext;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn pid_inheritance_is_startup_only_for_process_and_filesystem_helpers() -> Result<()> {
    let Some(bwrap) = codex_sandboxing::find_system_bwrap_in_path() else {
        eprintln!("skipping PID namespace test: no system bubblewrap");
        return Ok(());
    };
    let available = tokio::process::Command::new(&bwrap)
        .args(["--unshare-user", "--ro-bind", "/", "/", "--", "/bin/true"])
        .output()
        .await?;
    if !available.status.success() {
        eprintln!(
            "skipping PID namespace test: {}",
            String::from_utf8_lossy(&available.stderr)
        );
        return Ok(());
    }
    tokio::time::timeout(Duration::from_secs(30), async {
        let fixture = TempDir::new()?;
        let wrapper = fixture.path().join("bwrap");
        std::os::unix::fs::symlink(bwrap, fixture.path().join("real-bwrap"))?;
        std::fs::write(&wrapper, r#"#!/bin/sh
for arg in "$@"; do
    [ "$arg" = "--" ] && break
    if [ "$arg" = "--proc" ]; then
        echo "bwrap: Can't mount proc on /newroot/proc: Operation not permitted" >&2
        exit 1
    fi
done
exec "${0%/*}/real-bwrap" "$@"
"#)?;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))?;
        let path = format!("{}:{}", fixture.path().display(), std::env::var("PATH")?);
        // System bubblewrap discovery intentionally excludes executables under the command cwd.
        let workspace = TempDir::new()?;
        let file = workspace.path().join("readable");
        std::fs::write(&file, "ok")?;
        let file_uri = url::Url::from_file_path(&file).unwrap();
        let cwd = url::Url::from_directory_path(workspace.path()).unwrap();
        let minimal_policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special { value: FileSystemSpecialPath::Minimal },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                AbsolutePathBuf::try_from(workspace.path())?.into(), FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                AbsolutePathBuf::from_absolute_path("/proc/version")?.into(), FileSystemAccessMode::Deny,
            ),
        ]);
        let minimal_profile = PermissionProfile::from_runtime_permissions(
            &minimal_policy, NetworkSandboxPolicy::Restricted,
        );
        for (inherit, profile) in [
            (false, PermissionProfile::read_only()),
            (true, PermissionProfile::read_only()),
            (true, minimal_profile),
        ] {
            let minimal = profile.file_system_sandbox_policy().include_platform_defaults();
            let sandbox = FileSystemSandboxContext::from_permission_profile(
                profile, cwd.as_str().parse()?,
            );
            let codex_home = TempDir::new()?;
            // Neither a config key nor executor/request environment variables may opt in.
            std::fs::write(codex_home.path().join("config.toml"),
                "linux_sandbox_pid_namespace = 'inherit'\n")?;
            let mut command = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
            command.args(["exec-server", "--listen", "stdio"])
                .env("CODEX_HOME", codex_home.path())
                .env("PATH", &path)
                .env("CODEX_LINUX_SANDBOX_PID_NAMESPACE", "inherit")
                .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
                .kill_on_drop(true);
            if inherit {
                command.arg("--linux-sandbox-pid-namespace=inherit");
            }
            let mut child = command.spawn()?;
            let mut stdin = child.stdin.take().context("executor stdin")?;
            let mut stdout = BufReader::new(child.stdout.take().context("executor stdout")?);
            send_json_line(&mut stdin, &serde_json::json!({
                "id": 1, "method": "initialize",
                "params": {"clientName": "pid-namespace-test", "resumeSessionId": null}
            })).await?;
            wait_for_response(&mut stdout, /*expected_id*/ 1).await?;
            send_json_line(&mut stdin, &serde_json::json!({"method": "initialized", "params": {}})).await?;
            send_json_line(&mut stdin, &serde_json::json!({
                "id": 2, "method": "process/start", "params": {
                    "processId": "pid-test", "cwd": cwd, "sandbox": sandbox,
                    "argv": ["/bin/bash", "-lc", r#"
set -e
IFS=' ' read -r pid rest < /proc/self/stat
test "$$" = "$pid"
test "$(cat <(printf shell-ok))" = shell-ok
if [ "$MINIMAL" = true ]; then
    ! cat /proc/version
fi
"#],
                    "env": {"PATH": path, "MINIMAL": minimal.to_string(), "CODEX_LINUX_SANDBOX_PID_NAMESPACE": "inherit"},
                    "tty": false, "pipeStdin": false, "arg0": null
                }
            })).await?;
            // Notifications may precede the process/start response, so consume both.
            let mut started = false;
            let mut exit_code = None;
            while !started || exit_code.is_none() {
                let mut line = String::new();
                anyhow::ensure!(stdout.read_line(&mut line).await? > 0, "executor closed");
                let message: serde_json::Value = serde_json::from_str(&line)?;
                if message["id"] == 2 {
                    anyhow::ensure!(message.get("error").is_none(), "start failed: {message}");
                    started = true;
                }
                if message["method"] == "process/exited" {
                    exit_code = message["params"]["exitCode"].as_i64();
                }
            }
            assert_eq!(exit_code, Some(if inherit { 0 } else { 1 }));
            send_json_line(&mut stdin, &serde_json::json!({
                "id": 3, "method": "fs/readFile", "params": {"path": file_uri, "sandbox": sandbox}
            })).await?;
            loop {
                let mut line = String::new();
                anyhow::ensure!(stdout.read_line(&mut line).await? > 0, "executor closed");
                let message: serde_json::Value = serde_json::from_str(&line)?;
                if message["id"] != 3 { continue; }
                assert_eq!(message["result"], serde_json::json!({"dataBase64": "b2s="}));
                break;
            }
            drop(stdin);
            // Drain notifications while the executor shuts down.
            let mut remaining = String::new();
            stdout.read_to_string(&mut remaining).await?;
            let output = child.wait_with_output().await?;
            anyhow::ensure!(output.status.success(), "executor failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }).await?
}
