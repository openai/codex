use super::refresh_ssh_auth_sock_from_path;
use super::stable_ssh_auth_sock_path;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixListener;
use tempfile::TempDir;

#[test]
fn refresh_retargets_stable_agent_symlink() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let control_socket_path = temp_dir.path().join("app-server.sock");
    let first_agent_path = temp_dir.path().join("agent-1.sock");
    let second_agent_path = temp_dir.path().join("agent-2.sock");
    let _first_agent = UnixListener::bind(&first_agent_path).expect("bind first agent socket");
    let _second_agent = UnixListener::bind(&second_agent_path).expect("bind second agent socket");

    let stable_path =
        refresh_ssh_auth_sock_from_path(&control_socket_path, first_agent_path.as_os_str())
            .expect("refresh first agent")
            .expect("first agent should be valid");
    assert_eq!(stable_path, stable_ssh_auth_sock_path(&control_socket_path));
    assert_eq!(
        fs::read_link(&stable_path).expect("read first agent symlink"),
        first_agent_path
    );
    assert!(
        fs::metadata(&stable_path)
            .expect("read stable agent metadata")
            .file_type()
            .is_socket()
    );

    let refreshed_path =
        refresh_ssh_auth_sock_from_path(&control_socket_path, second_agent_path.as_os_str())
            .expect("refresh second agent")
            .expect("second agent should be valid");
    assert_eq!(refreshed_path, stable_path);
    assert_eq!(
        fs::read_link(&stable_path).expect("read second agent symlink"),
        second_agent_path
    );
}

#[test]
fn refresh_ignores_non_socket_agent_path() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let control_socket_path = temp_dir.path().join("app-server.sock");
    let regular_file_path = temp_dir.path().join("not-an-agent");
    fs::write(&regular_file_path, "not a socket").expect("write regular file");

    assert_eq!(
        refresh_ssh_auth_sock_from_path(&control_socket_path, regular_file_path.as_os_str())
            .expect("ignore regular file"),
        None
    );
    assert!(fs::symlink_metadata(stable_ssh_auth_sock_path(&control_socket_path)).is_err());
}

#[test]
fn refresh_refuses_to_replace_non_symlink_path() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let control_socket_path = temp_dir.path().join("app-server.sock");
    let agent_path = temp_dir.path().join("agent.sock");
    let _agent = UnixListener::bind(&agent_path).expect("bind agent socket");
    let stable_path = stable_ssh_auth_sock_path(&control_socket_path);
    fs::write(&stable_path, "do not replace").expect("write stable path");

    let err = refresh_ssh_auth_sock_from_path(&control_socket_path, agent_path.as_os_str())
        .expect_err("non-symlink path should be preserved");

    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(stable_path).expect("read preserved file"),
        "do not replace"
    );
}

#[test]
fn refresh_preserves_inherited_stable_path_while_agent_is_disconnected() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let control_socket_path = temp_dir.path().join("app-server.sock");
    let stable_path = stable_ssh_auth_sock_path(&control_socket_path);

    assert_eq!(
        refresh_ssh_auth_sock_from_path(&control_socket_path, stable_path.as_os_str())
            .expect("preserve stable path"),
        Some(stable_path)
    );
}

#[test]
fn refresh_rejects_inherited_stable_path_when_it_is_not_a_symlink() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let control_socket_path = temp_dir.path().join("app-server.sock");
    let stable_path = stable_ssh_auth_sock_path(&control_socket_path);
    fs::write(&stable_path, "not a symlink").expect("write stable path");

    let err = refresh_ssh_auth_sock_from_path(&control_socket_path, stable_path.as_os_str())
        .expect_err("non-symlink stable path should be rejected");

    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
}
