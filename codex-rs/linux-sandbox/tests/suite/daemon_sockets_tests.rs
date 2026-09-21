use super::*;
use pretty_assertions::assert_eq;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[tokio::test]
async fn private_tmp_mount_preserves_daemon_socket_isolation() {
    if should_skip_bwrap_tests().await {
        return;
    }
    // The parent owns cleanup after the child's disposable mount namespace exits.
    let private = tempfile::tempdir_in("/tmp").unwrap();
    let test_executable = std::env::current_exe().unwrap();
    // Keep both executables available after the original /tmp is hidden.
    std::fs::copy(&test_executable, private.path().join("test")).unwrap();
    std::fs::copy(codex_linux_sandbox_exe(), private.path().join("sandbox")).unwrap();
    let (_, test_module) = module_path!().split_once("::").unwrap();
    let fixture_test = format!("{test_module}::private_tmp_fixture");
    // Mount setup needs capabilities, which bubblewrap only accepts as namespace root.
    let output = tokio::process::Command::new("unshare")
        .args([
            "--user",
            "--map-root-user",
            "--mount",
            "--propagation",
            "private",
            "--",
        ])
        .arg(test_executable)
        .args([
            "--exact",
            &fixture_test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CODEX_TEST_PRIVATE_TMP", private.path())
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    if output.status.code() == Some(77)
        || String::from_utf8_lossy(&output.stderr).starts_with("unshare:")
    {
        eprintln!("skipping private tmp test: user/mount namespaces are unavailable");
        return;
    }
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("private-tmp-validated"));
}

#[test]
#[ignore = "invoked inside a disposable mount namespace"]
fn private_tmp_fixture() {
    let Some(private) = std::env::var_os("CODEX_TEST_PRIVATE_TMP") else {
        return;
    };
    let mount = std::process::Command::new("mount")
        .arg("--bind")
        .arg(private)
        .arg("/tmp")
        .output()
        .unwrap();
    if !mount.status.success() {
        std::process::exit(/*code*/ 77);
    }
    // Namespace mounts have non-path roots such as `net:[inode]`. An unrelated
    // one must not stop startup or disable the socket-isolation checks below.
    let namespace = "/tmp/network-namespace";
    std::fs::File::create(namespace).unwrap();
    let mount = std::process::Command::new("mount")
        .args(["--bind", "/proc/self/ns/net", namespace])
        .output()
        .unwrap();
    assert!(mount.status.success(), "{mount:?}");
    let root = codex_uds::prepare_shared_daemon_socket_directory().unwrap();
    let endpoint = root.join("rpc.sock");
    let _daemon = UnixListener::bind(&endpoint).unwrap();
    let _other = UnixListener::bind("/tmp/other.sock").unwrap();
    UnixStream::connect(&endpoint).expect("host can reach daemon");
    let profile = PermissionProfile::workspace_write_with(
        &[AbsolutePathBuf::from_absolute_path("/tmp").unwrap()],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ false,
    );
    let mut command = std::process::Command::new("/tmp/sandbox");
    command
        .args(["--sandbox-policy-cwd", "/tmp", "--permission-profile"])
        .arg(serde_json::to_string(&profile).unwrap())
        .arg("--")
        .current_dir("/tmp");
    let (_, test_module) = module_path!().split_once("::").unwrap();
    let client_test = format!("{test_module}::private_tmp_client");
    command
        .args([
            "/tmp/test",
            "--exact",
            &client_test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CODEX_TEST_DAEMON_SOCKET", &endpoint)
        .env("TMPDIR", "/tmp");
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("private-tmp-isolated"));

    // Devboxes expose their temporary volume through both /tmp and /build/tmp.
    // An ancestor alias must receive the same socket mask without hiding other files.
    std::fs::create_dir("/tmp/build").unwrap();
    let mount = std::process::Command::new("mount")
        .args(["--bind", "/tmp", "/tmp/build"])
        .output()
        .unwrap();
    assert!(mount.status.success(), "{mount:?}");
    let alias_endpoint = PathBuf::from("/tmp/build")
        .join(root.file_name().unwrap())
        .join("rpc.sock");
    UnixStream::connect(&alias_endpoint).expect("host can reach ancestor alias");
    command
        .env("CODEX_TEST_DAEMON_ALIAS_SOCKET", &alias_endpoint)
        .env("CODEX_TEST_OTHER_ALIAS_SOCKET", "/tmp/build/other.sock");
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("private-tmp-isolated"));

    // Rebinding the synthetic-mount registry must preserve masks for aliases
    // beneath it, even when the surrounding /tmp is writable.
    let registry_alias = PathBuf::from(format!(
        "/tmp/codex-bwrap-synthetic-mount-targets-{}/view",
        unsafe { libc::geteuid() }
    ));
    std::fs::create_dir_all(&registry_alias).unwrap();
    let mount = std::process::Command::new("mount")
        .args(["--bind", "/tmp"])
        .arg(&registry_alias)
        .output()
        .unwrap();
    assert!(mount.status.success(), "{mount:?}");
    let alias_endpoint = registry_alias
        .join(root.file_name().unwrap())
        .join("rpc.sock");
    UnixStream::connect(&alias_endpoint).expect("host can reach registry alias");
    command
        .env("CODEX_TEST_DAEMON_ALIAS_SOCKET", &alias_endpoint)
        .env(
            "CODEX_TEST_OTHER_ALIAS_SOCKET",
            registry_alias.join("other.sock"),
        );
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("private-tmp-isolated"));

    // A real alias below the private mount must still prevent startup.
    std::fs::create_dir("/tmp/alias").unwrap();
    let mount = std::process::Command::new("mount")
        .arg("--bind")
        .arg(&root)
        .arg("/tmp/alias")
        .output()
        .unwrap();
    assert!(mount.status.success(), "{mount:?}");
    UnixStream::connect("/tmp/alias/rpc.sock").expect("host can reach alias");
    let output = command.output().unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported host mount"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-tmp-client-started"));
    println!("private-tmp-validated");
}

#[test]
#[ignore = "invoked inside the private tmp sandbox"]
fn private_tmp_client() {
    let Some(endpoint) = std::env::var_os("CODEX_TEST_DAEMON_SOCKET") else {
        return;
    };
    println!("private-tmp-client-started");
    assert!(UnixStream::connect(endpoint).is_err(), "daemon reachable");
    UnixStream::connect("/tmp/other.sock").expect("unrelated socket reachable");
    if let Some(alias) = std::env::var_os("CODEX_TEST_DAEMON_ALIAS_SOCKET") {
        assert!(
            UnixStream::connect(alias).is_err(),
            "daemon alias reachable"
        );
        UnixStream::connect(std::env::var_os("CODEX_TEST_OTHER_ALIAS_SOCKET").unwrap())
            .expect("unrelated alias socket reachable");
    }
    println!("private-tmp-isolated");
}

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
