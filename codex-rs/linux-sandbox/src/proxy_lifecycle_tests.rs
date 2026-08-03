use super::PROXY_SOCKET_DIR_PREFIX;
use super::cleanup_proxy_socket_dir;
use super::cleanup_stale_proxy_socket_dirs_in;
use super::is_pid_alive_in_proc_root;
use super::is_pid_alive_raw;
use super::parse_proxy_socket_dir_owner_pid;
use pretty_assertions::assert_eq;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

#[test]
fn cleanup_proxy_socket_dir_removes_bridge_artifacts() {
    let root = tempfile::tempdir().expect("tempdir should create");
    let socket_dir = root.path().join("codex-linux-sandbox-proxy-test");
    std::fs::create_dir(&socket_dir).expect("socket dir should create");
    let marker = socket_dir.join("bridge.sock");
    std::fs::write(&marker, b"test").expect("marker should write");

    cleanup_proxy_socket_dir(socket_dir.as_path()).expect("cleanup should succeed");

    assert_eq!(socket_dir.exists(), false);
}

#[test]
fn parse_proxy_socket_dir_owner_pid_reads_owner_pid() {
    assert_eq!(
        parse_proxy_socket_dir_owner_pid("codex-linux-sandbox-proxy-1234-0"),
        Some(1234)
    );
    assert_eq!(
        parse_proxy_socket_dir_owner_pid("codex-linux-sandbox-proxy-1234-1000-0"),
        Some(1234)
    );
    assert_eq!(
        parse_proxy_socket_dir_owner_pid("codex-linux-sandbox-proxy-x"),
        None
    );
    assert_eq!(parse_proxy_socket_dir_owner_pid("not-a-proxy-dir"), None);
}

#[test]
fn cleanup_stale_proxy_socket_dirs_removes_dead_pid_directories() {
    let root = tempfile::tempdir().expect("tempdir should create");
    let dead_dir = root
        .path()
        .join(format!("{PROXY_SOCKET_DIR_PREFIX}{}-0", u32::MAX));
    std::fs::create_dir(&dead_dir).expect("dead dir should create");

    let alive_dir = root
        .path()
        .join(format!("{PROXY_SOCKET_DIR_PREFIX}{}-1", std::process::id()));
    std::fs::create_dir(&alive_dir).expect("alive dir should create");

    let unrelated_dir = root.path().join("unrelated-proxy-dir");
    std::fs::create_dir(&unrelated_dir).expect("unrelated dir should create");

    cleanup_stale_proxy_socket_dirs_in(root.path()).expect("stale cleanup should succeed");

    assert_eq!(dead_dir.exists(), false);
    assert_eq!(alive_dir.exists(), true);
    assert_eq!(unrelated_dir.exists(), true);
}

#[test]
fn missing_procfs_keeps_live_process_alive() {
    let root = tempfile::tempdir().expect("tempdir should create");
    let missing_proc_root = root.path().join("missing-proc");
    let pid = libc::pid_t::try_from(std::process::id()).expect("current pid should fit");

    assert_eq!(is_pid_alive_in_proc_root(pid, &missing_proc_root), true);
}

#[test]
fn zombie_process_is_not_alive() {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .spawn()
        .expect("child should spawn");
    let pid = libc::pid_t::try_from(child.id()).expect("child pid should fit");
    let deadline = Instant::now() + Duration::from_secs(5);

    while is_pid_alive_raw(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let zombie_was_recognized = !is_pid_alive_raw(pid);
    child.wait().expect("child should be reaped");

    assert_eq!(zombie_was_recognized, true);
}
