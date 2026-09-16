use super::*;
use pretty_assertions::assert_eq;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;

#[tokio::test]
async fn daemon_socket_bind_mount_alias_rejects_sandbox_startup() {
    if should_skip_bwrap_tests().await {
        return;
    }
    let root = codex_uds::prepare_shared_daemon_socket_directory().unwrap();
    let private = tempfile::tempdir_in(&root).unwrap();
    let alias = tempfile::tempdir().unwrap();
    let _listener = UnixListener::bind(private.path().join("rpc.sock")).unwrap();
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::read_only(),
        NetworkSandboxPolicy::Enabled,
    );
    let script = r#"
import pathlib, socket, subprocess, sys
root, alias, endpoint, helper, profile = sys.argv[1:]
mount = subprocess.run(['mount', '--bind', root, alias], capture_output=True)
if mount.returncode:
    sys.exit(77)
try:
    socket.socket(socket.AF_UNIX).connect(str(pathlib.Path(alias) / endpoint))
    result = subprocess.run([helper, '--sandbox-policy-cwd', '/', '--permission-profile', profile,
        '--', 'sh', '-c', 'echo command-started'], capture_output=True)
    assert result.returncode != 0, result
    assert b'unsupported host mount' in result.stderr, result
    assert b'command-started' not in result.stdout, result
finally:
    subprocess.run(['umount', alias], check=True)
"#;
    let output = tokio::process::Command::new("unshare")
        .args([
            "--user",
            "--map-current-user",
            "--keep-caps",
            "--mount",
            "--propagation",
            "private",
            "--",
        ])
        .args(["python3", "-c", script])
        .arg(&root)
        .arg(alias.path())
        .arg(private.path().strip_prefix(&root).unwrap().join("rpc.sock"))
        .arg(codex_linux_sandbox_exe())
        .arg(serde_json::to_string(&profile).unwrap())
        .kill_on_drop(true)
        .output()
        .await;
    let Ok(output) = output else {
        eprintln!("skipping bind-mount test: unshare is unavailable");
        return;
    };
    if output.status.code() == Some(77)
        || String::from_utf8_lossy(&output.stderr).starts_with("unshare:")
    {
        eprintln!("skipping bind-mount test: user/mount namespaces are unavailable");
        return;
    }
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[tokio::test]
async fn daemon_sockets_are_hidden_with_network_and_tmp_write_grants() {
    if should_skip_bwrap_tests().await {
        return;
    }
    let root = codex_uds::prepare_shared_daemon_socket_directory().unwrap();
    let private = tempfile::tempdir_in(&root).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let socket = private.path().join("rpc.sock");
    let alias = workspace.path().join("alias.sock");
    let unrelated = workspace.path().join("other.sock");
    let directory_fd = std::fs::File::open(private.path()).unwrap();
    let _daemon = UnixListener::bind(&socket).unwrap();
    let _other = UnixListener::bind(&unrelated).unwrap();
    std::os::unix::fs::symlink(&socket, &alias).unwrap();
    UnixStream::connect(&alias).expect("host can reach daemon");
    let script = r#"
import os, socket, sys
for path in [sys.argv[1], sys.argv[2], '/proc/' + sys.argv[5] + '/root' + sys.argv[1]]:
    try:
        socket.socket(socket.AF_UNIX).connect(path)
    except OSError:
        pass
    else:
        raise AssertionError('daemon reachable: ' + path)
try:
    os.link(sys.argv[1], sys.argv[4] + '/hardlink')
except OSError:
    pass
else:
    raise AssertionError('daemon hardlink created')
socket.socket(socket.AF_UNIX).connect(sys.argv[3])
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[4] + '/local.sock')
s.listen(1)
c = socket.socket(socket.AF_UNIX)
c.connect(sys.argv[4] + '/local.sock')
p, _ = s.accept()
c.sendall(b'ok')
assert p.recv(2) == b'ok'
os.unlink(sys.argv[4] + '/local.sock')
# Main already hides procfs when isolating WSL interop.
if sys.argv[6] == 'host-proc' and not os.path.isdir('/run/WSL'):
    assert os.path.isfile('/proc/self/status')
    assert os.path.isdir('/proc/' + sys.argv[5])
    for name in ['root', 'cwd', 'fd/' + sys.argv[7]]:
        try:
            os.readlink('/proc/' + sys.argv[5] + '/' + name)
        except PermissionError:
            pass
        else:
            raise AssertionError('host process link accessible: ' + name)
"#;
    let profile = PermissionProfile::workspace_write_with(
        &[AbsolutePathBuf::from_absolute_path("/tmp").unwrap()],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ false,
    );
    let mut env = create_env_from_core_vars();
    env.insert(
        "CODEX_HOME".to_string(),
        workspace.path().display().to_string(),
    );
    env.insert("TMPDIR".to_string(), workspace.path().display().to_string());
    // Inherited procfs remains useful in containers that prohibit a fresh mount.
    // Its host-process magic links must still respect the user namespace boundary.
    for proc_mode in ["fresh-proc", "host-proc"] {
        let mut command = tokio::process::Command::new(codex_linux_sandbox_exe());
        command
            .arg("--sandbox-policy-cwd")
            .arg(workspace.path())
            .arg("--permission-profile")
            .arg(serde_json::to_string(&profile).unwrap())
            .envs(&env)
            .current_dir(workspace.path());
        if proc_mode == "host-proc" {
            command.arg("--no-proc");
        }
        let output = command
            .args([
                "--",
                "python3",
                "-c",
                script,
                socket.to_str().unwrap(),
                alias.to_str().unwrap(),
                unrelated.to_str().unwrap(),
                workspace.path().to_str().unwrap(),
                &std::process::id().to_string(),
                proc_mode,
                &directory_fd.as_raw_fd().to_string(),
            ])
            .kill_on_drop(true)
            .output()
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
}
