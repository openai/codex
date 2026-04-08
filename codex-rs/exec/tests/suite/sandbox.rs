#![cfg(unix)]
use codex_core::spawn::StdioPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::future::Future;
use std::io;
#[cfg(target_os = "linux")]
use std::net::Ipv4Addr;
#[cfg(target_os = "linux")]
use std::net::TcpListener;
#[cfg(target_os = "linux")]
use std::net::TcpStream;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use tokio::fs::create_dir_all;
use tokio::process::Child;

#[cfg(target_os = "macos")]
async fn spawn_command_under_sandbox(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    use codex_core::seatbelt::spawn_command_under_seatbelt;
    spawn_command_under_seatbelt(
        command,
        command_cwd,
        sandbox_policy,
        sandbox_cwd,
        stdio_policy,
        /*network*/ None,
        env,
    )
    .await
}

#[cfg(target_os = "linux")]
async fn spawn_command_under_sandbox(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    use codex_core::spawn_command_under_linux_sandbox;
    let codex_linux_sandbox_exe = core_test_support::find_codex_linux_sandbox_exe()
        .map_err(|err| io::Error::new(io::ErrorKind::NotFound, err))?;
    spawn_command_under_linux_sandbox(
        codex_linux_sandbox_exe,
        command,
        command_cwd,
        sandbox_policy,
        sandbox_cwd,
        /*use_legacy_landlock*/ false,
        stdio_policy,
        /*network*/ None,
        env,
    )
    .await
}

#[cfg(target_os = "linux")]
/// Determines whether Linux sandbox tests can run on this host.
///
/// These tests require an enforceable filesystem sandbox. We run a tiny command
/// under the production Landlock path and skip when enforcement is unavailable
/// (for example on kernels or container profiles where Landlock is not
/// enforced).
async fn linux_sandbox_test_env() -> Option<HashMap<String, String>> {
    let command_cwd = std::env::current_dir().ok()?;
    let sandbox_cwd = command_cwd.clone();
    let policy = SandboxPolicy::new_read_only_policy();

    if can_apply_linux_sandbox_policy(&policy, &command_cwd, sandbox_cwd.as_path(), HashMap::new())
        .await
    {
        return Some(HashMap::new());
    }

    eprintln!("Skipping test: Landlock is not enforceable on this host.");
    None
}

#[cfg(target_os = "linux")]
/// Returns whether a minimal command can run successfully with the requested
/// Linux sandbox policy applied.
///
/// This is used as a capability probe so sandbox behavior tests only run when
/// Landlock enforcement is actually active.
async fn can_apply_linux_sandbox_policy(
    policy: &SandboxPolicy,
    command_cwd: &Path,
    sandbox_cwd: &Path,
    env: HashMap<String, String>,
) -> bool {
    let spawn_result = spawn_command_under_sandbox(
        vec!["/usr/bin/true".to_string()],
        command_cwd.to_path_buf(),
        policy,
        sandbox_cwd,
        StdioPolicy::RedirectForShellTool,
        env,
    )
    .await;
    let Ok(mut child) = spawn_result else {
        return false;
    };
    child
        .wait()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn python_multiprocessing_lock_works_under_sandbox() {
    core_test_support::skip_if_sandbox!();
    #[cfg(target_os = "linux")]
    let sandbox_env = match linux_sandbox_test_env().await {
        Some(env) => env,
        // Skip on Linux hosts where Landlock cannot actually be enforced.
        None => return,
    };
    #[cfg(not(target_os = "linux"))]
    let sandbox_env = HashMap::new();
    #[cfg(target_os = "macos")]
    let writable_roots = Vec::<AbsolutePathBuf>::new();

    // From https://man7.org/linux/man-pages/man7/sem_overview.7.html
    //
    // > On Linux, named semaphores are created in a virtual filesystem,
    // > normally mounted under /dev/shm.
    #[cfg(target_os = "linux")]
    let writable_roots: Vec<AbsolutePathBuf> = vec!["/dev/shm".try_into().unwrap()];

    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots,
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    let python_code = r#"import multiprocessing
from multiprocessing import Lock, Process

def f(lock):
    with lock:
        print("Lock acquired in child process")

if __name__ == '__main__':
    lock = Lock()
    p = Process(target=f, args=(lock,))
    p.start()
    p.join()
"#;

    let command_cwd = std::env::current_dir().expect("should be able to get current dir");
    let sandbox_cwd = command_cwd.clone();
    let mut child = spawn_command_under_sandbox(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            python_code.to_string(),
        ],
        command_cwd,
        &policy,
        sandbox_cwd.as_path(),
        StdioPolicy::Inherit,
        sandbox_env,
    )
    .await
    .expect("should be able to spawn python under sandbox");

    let status = child.wait().await.expect("should wait for child process");
    assert!(status.success(), "python exited with {status:?}");
}

#[tokio::test]
async fn python_getpwuid_works_under_sandbox() {
    core_test_support::skip_if_sandbox!();
    #[cfg(target_os = "linux")]
    let sandbox_env = match linux_sandbox_test_env().await {
        Some(env) => env,
        None => return,
    };
    #[cfg(not(target_os = "linux"))]
    let sandbox_env = HashMap::new();

    if std::process::Command::new("python3")
        .arg("--version")
        .status()
        .is_err()
    {
        eprintln!("python3 not found in PATH, skipping test.");
        return;
    }

    let policy = SandboxPolicy::new_read_only_policy();
    let command_cwd = std::env::current_dir().expect("should be able to get current dir");
    let sandbox_cwd = command_cwd.clone();

    let mut child = spawn_command_under_sandbox(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            "import pwd, os; print(pwd.getpwuid(os.getuid()))".to_string(),
        ],
        command_cwd,
        &policy,
        sandbox_cwd.as_path(),
        StdioPolicy::RedirectForShellTool,
        sandbox_env,
    )
    .await
    .expect("should be able to spawn python under sandbox");

    let status = child
        .wait()
        .await
        .expect("should be able to wait for child process");
    assert!(status.success(), "python exited with {status:?}");
}

#[tokio::test]
async fn sandbox_distinguishes_command_and_policy_cwds() {
    core_test_support::skip_if_sandbox!();
    #[cfg(target_os = "linux")]
    let sandbox_env = match linux_sandbox_test_env().await {
        Some(env) => env,
        None => return,
    };
    #[cfg(not(target_os = "linux"))]
    let sandbox_env = HashMap::new();
    let temp = tempfile::tempdir().expect("should be able to create temp dir");
    let sandbox_root = temp.path().join("sandbox");
    let command_root = temp.path().join("command");
    create_dir_all(&sandbox_root).await.expect("mkdir");
    create_dir_all(&command_root).await.expect("mkdir");
    let canonical_sandbox_root = tokio::fs::canonicalize(&sandbox_root)
        .await
        .expect("canonicalize sandbox root");
    let canonical_allowed_path = canonical_sandbox_root.join("allowed.txt");

    let disallowed_path = command_root.join("forbidden.txt");

    // Note writable_roots is empty: verify that `canonical_allowed_path` is
    // writable only because it is under the sandbox policy cwd, not because it
    // is under a writable root.
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    // Attempt to write inside the command cwd, which is outside of the sandbox policy cwd.
    let mut child = spawn_command_under_sandbox(
        vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo forbidden > forbidden.txt".to_string(),
        ],
        command_root.clone(),
        &policy,
        canonical_sandbox_root.as_path(),
        StdioPolicy::Inherit,
        sandbox_env.clone(),
    )
    .await
    .expect("should spawn command writing to forbidden path");

    let status = child
        .wait()
        .await
        .expect("should wait for forbidden command");
    assert!(
        !status.success(),
        "sandbox unexpectedly allowed writing to command cwd: {status:?}"
    );
    let forbidden_exists = tokio::fs::try_exists(&disallowed_path)
        .await
        .expect("try_exists failed");
    assert!(
        !forbidden_exists,
        "forbidden path should not have been created"
    );

    // Writing to the sandbox policy cwd after changing directories into it should succeed.
    let mut child = spawn_command_under_sandbox(
        vec![
            "/usr/bin/touch".to_string(),
            canonical_allowed_path.to_string_lossy().into_owned(),
        ],
        command_root,
        &policy,
        canonical_sandbox_root.as_path(),
        StdioPolicy::Inherit,
        sandbox_env,
    )
    .await
    .expect("should spawn command writing to sandbox root");

    let status = child.wait().await.expect("should wait for allowed command");
    assert!(
        status.success(),
        "sandbox blocked allowed write: {status:?}"
    );
    let allowed_exists = tokio::fs::try_exists(&canonical_allowed_path)
        .await
        .expect("try_exists allowed failed");
    assert!(allowed_exists, "allowed path should exist");
}

#[tokio::test]
async fn sandbox_blocks_first_time_dot_codex_creation() {
    core_test_support::skip_if_sandbox!();
    #[cfg(target_os = "linux")]
    let sandbox_env = match linux_sandbox_test_env().await {
        Some(env) => env,
        None => return,
    };
    #[cfg(not(target_os = "linux"))]
    let sandbox_env = HashMap::new();

    let temp = tempfile::tempdir().expect("should be able to create temp dir");
    let repo_root = temp.path().join("repo");
    create_dir_all(&repo_root).await.expect("mkdir repo");
    let dot_codex = repo_root.join(".codex");
    let config_toml = dot_codex.join("config.toml");
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let mut child = spawn_command_under_sandbox(
        vec![
            "bash".to_string(),
            "-lc".to_string(),
            "mkdir -p .codex && echo 'sandbox_mode = \"danger-full-access\"' > .codex/config.toml"
                .to_string(),
        ],
        repo_root.clone(),
        &policy,
        repo_root.as_path(),
        StdioPolicy::RedirectForShellTool,
        sandbox_env,
    )
    .await
    .expect("should spawn command creating .codex");

    let status = child.wait().await.expect("should wait for .codex command");
    assert!(
        !status.success(),
        "sandbox unexpectedly allowed first-time .codex creation: {status:?}"
    );
    let dot_codex_metadata = tokio::fs::symlink_metadata(&dot_codex).await;
    if let Ok(metadata) = dot_codex_metadata {
        assert!(
            !metadata.is_dir(),
            "{} should not be creatable as a directory",
            dot_codex.display()
        );
    } else if let Err(err) = &dot_codex_metadata {
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "unexpected metadata error for {}: {err}",
            dot_codex.display()
        );
    }
    let config_toml_exists = match tokio::fs::try_exists(&config_toml).await {
        Ok(exists) => exists,
        Err(err) if err.kind() == io::ErrorKind::NotADirectory => false,
        Err(err) => panic!("try_exists {} failed: {err}", config_toml.display()),
    };
    assert!(
        !config_toml_exists,
        "{} should not have been created",
        config_toml.display()
    );
}

fn unix_sock_body() {
    unsafe {
        let mut fds = [0i32; 2];
        let r = libc::socketpair(libc::AF_UNIX, libc::SOCK_DGRAM, 0, fds.as_mut_ptr());
        assert_eq!(
            r,
            0,
            "socketpair(AF_UNIX, SOCK_DGRAM) failed: {}",
            io::Error::last_os_error()
        );

        let msg = b"hello_unix";
        // write() from one end (generic write is allowed)
        let sent = libc::write(fds[0], msg.as_ptr() as *const libc::c_void, msg.len());
        assert!(sent >= 0, "write() failed: {}", io::Error::last_os_error());

        // recvfrom() on the other end. We don’t need the address for socketpair,
        // so we pass null pointers for src address.
        let mut buf = [0u8; 64];
        let recvd = libc::recvfrom(
            fds[1],
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(
            recvd >= 0,
            "recvfrom() failed: {}",
            io::Error::last_os_error()
        );

        let recvd_slice = &buf[..(recvd as usize)];
        assert_eq!(
            recvd_slice,
            &msg[..],
            "payload mismatch: sent {} bytes, got {} bytes",
            msg.len(),
            recvd
        );

        let sent = libc::sendto(
            fds[0],
            msg.as_ptr() as *const libc::c_void,
            msg.len(),
            0,
            std::ptr::null(),
            0,
        );
        assert!(
            sent >= 0,
            "sendto(NULL, 0) failed: {}",
            io::Error::last_os_error()
        );
        let recvd = libc::recvfrom(
            fds[1],
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(
            recvd >= 0,
            "recvfrom() after sendto(NULL, 0) failed: {}",
            io::Error::last_os_error()
        );
        assert_eq!(&buf[..(recvd as usize)], &msg[..]);

        // Also exercise AF_UNIX stream socketpair quickly to ensure AF_UNIX in general works.
        let mut sfds = [0i32; 2];
        let sr = libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, sfds.as_mut_ptr());
        assert_eq!(
            sr,
            0,
            "socketpair(AF_UNIX, SOCK_STREAM) failed: {}",
            io::Error::last_os_error()
        );
        let snt2 = libc::write(sfds[0], msg.as_ptr() as *const libc::c_void, msg.len());
        assert!(
            snt2 >= 0,
            "write(stream) failed: {}",
            io::Error::last_os_error()
        );
        let mut b2 = [0u8; 64];
        let rcv2 = libc::recv(sfds[1], b2.as_mut_ptr() as *mut libc::c_void, b2.len(), 0);
        assert!(
            rcv2 >= 0,
            "recv(stream) failed: {}",
            io::Error::last_os_error()
        );

        // Clean up
        let _ = libc::close(sfds[0]);
        let _ = libc::close(sfds[1]);
        let _ = libc::close(fds[0]);
        let _ = libc::close(fds[1]);
    }
}

#[tokio::test]
async fn allow_unix_socketpair_recvfrom() {
    let result = run_code_under_sandbox(
        "allow_unix_socketpair_recvfrom",
        &SandboxPolicy::new_read_only_policy(),
        || async { unix_sock_body() },
    )
    .await
    .expect("should be able to reexec");
    assert_sandbox_reexec_succeeded(result);
}

const IN_SANDBOX_ENV_VAR: &str = "IN_SANDBOX";
#[cfg(target_os = "linux")]
const INHERITED_CONNECTED_SOCKET_FD_ENV_VAR: &str = "INHERITED_CONNECTED_SOCKET_FD";

#[tokio::test]
#[cfg(target_os = "linux")]
async fn inherited_connected_tcp_socket_cannot_send_after_sandbox_exec() {
    let mut sandbox_env = HashMap::new();
    let mut connected_socket_guards: Option<(TcpStream, TcpStream)> = None;

    if std::env::var(IN_SANDBOX_ENV_VAR).is_err() {
        sandbox_env = match linux_sandbox_test_env().await {
            Some(env) => env,
            None => return,
        };

        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind local TCP listener");
        let stream =
            TcpStream::connect(listener.local_addr().expect("listener address")).expect("connect");
        let inherited_fd = stream.as_raw_fd();
        clear_cloexec(inherited_fd);
        sandbox_env.insert(
            INHERITED_CONNECTED_SOCKET_FD_ENV_VAR.to_string(),
            inherited_fd.to_string(),
        );
        let accepted_stream = listener.accept().expect("accept connection").0;
        connected_socket_guards = Some((stream, accepted_stream));
    }

    let result = run_code_under_sandbox_with_env(
        "inherited_connected_tcp_socket_cannot_send_after_sandbox_exec",
        &SandboxPolicy::new_read_only_policy(),
        sandbox_env,
        || async { inherited_connected_tcp_socket_send_body() },
    )
    .await
    .expect("should be able to reexec");
    drop(connected_socket_guards);
    assert_sandbox_reexec_succeeded(result);
}

#[cfg(target_os = "linux")]
fn inherited_connected_tcp_socket_send_body() {
    let fd = std::env::var(INHERITED_CONNECTED_SOCKET_FD_ENV_VAR)
        .expect("inherited fd env var should be set")
        .parse::<libc::c_int>()
        .expect("inherited fd should parse");
    let msg = b"should_not_escape";
    let sent = unsafe {
        libc::sendto(
            fd,
            msg.as_ptr() as *const libc::c_void,
            msg.len(),
            0,
            std::ptr::null(),
            0,
        )
    };
    assert!(
        sent < 0,
        "sendto(NULL, 0) on inherited connected TCP fd unexpectedly wrote {sent} bytes"
    );
}

#[cfg(target_os = "linux")]
fn clear_cloexec(fd: libc::c_int) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0, "F_GETFD failed: {}", io::Error::last_os_error());
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
    assert!(
        result >= 0,
        "F_SETFD failed: {}",
        io::Error::last_os_error()
    );
}

pub async fn run_code_under_sandbox<F, Fut>(
    test_selector: &str,
    policy: &SandboxPolicy,
    child_body: F,
) -> io::Result<Option<ExitStatus>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    run_code_under_sandbox_with_env(test_selector, policy, HashMap::new(), child_body).await
}

#[expect(clippy::expect_used)]
pub async fn run_code_under_sandbox_with_env<F, Fut>(
    test_selector: &str,
    policy: &SandboxPolicy,
    mut env: HashMap<String, String>,
    child_body: F,
) -> io::Result<Option<ExitStatus>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    if std::env::var(IN_SANDBOX_ENV_VAR).is_err() {
        let exe = std::env::current_exe()?;
        let mut cmds = vec![exe.to_string_lossy().into_owned(), "--exact".into()];
        let mut stdio_policy = StdioPolicy::RedirectForShellTool;
        // Allow for us to pass forward --nocapture / use the right stdio policy.
        if std::env::args().any(|a| a == "--nocapture") {
            cmds.push("--nocapture".into());
            stdio_policy = StdioPolicy::Inherit;
        }
        cmds.push(test_selector.into());

        // Your existing launcher:
        let command_cwd = std::env::current_dir().expect("should be able to get current dir");
        let sandbox_cwd = command_cwd.clone();
        env.insert(IN_SANDBOX_ENV_VAR.to_string(), "1".to_string());
        let mut child = spawn_command_under_sandbox(
            cmds,
            command_cwd,
            policy,
            sandbox_cwd.as_path(),
            stdio_policy,
            env,
        )
        .await?;

        let status = child.wait().await?;
        Ok(Some(status))
    } else {
        // Child branch: run the provided body.
        child_body().await;
        Ok(None)
    }
}

fn assert_sandbox_reexec_succeeded(status: Option<ExitStatus>) {
    if let Some(status) = status {
        assert!(status.success(), "sandboxed child exited with {status:?}");
    }
}
