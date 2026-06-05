use super::*;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell_snapshot::ShellSnapshot;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;
use tokio::sync::watch;

fn shell_with_snapshot(
    shell_type: ShellType,
    shell_path: &str,
    snapshot_path: AbsolutePathBuf,
    snapshot_cwd: AbsolutePathBuf,
) -> Shell {
    let (_tx, shell_snapshot) = watch::channel(Some(Arc::new(ShellSnapshot {
        path: snapshot_path,
        cwd: snapshot_cwd,
    })));
    Shell {
        shell_type,
        shell_path: PathBuf::from(shell_path),
        shell_snapshot,
    }
}

#[test]
fn user_shell_snapshot_preserves_package_path_prepend() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport PATH='/snapshot/bin'\n",
    )
    .expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path.abs(),
        dir.path().abs(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let package_path_dir = dir.path().join("codex-path");
    let mut env = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let rewritten = prepare_user_shell_exec_command_with_path_prepend(
        &command,
        &session_shell,
        &dir.path().abs(),
        &HashMap::new(),
        &mut env,
        |env, runtime_path_prepends| {
            runtime_path_prepends.prepend(env, package_path_dir.as_path());
        },
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("PATH", env.get("PATH").expect("PATH should be set"))
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}:/snapshot/bin", package_path_dir.display())
    );
}
