#![cfg(unix)]
use codex_core::protocol::SandboxPolicy;
use codex_core::spawn::StdioPolicy;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use tokio::process::Child;

#[cfg(target_os = "macos")]
async fn spawn_command_under_sandbox(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    use codex_core::seatbelt::spawn_command_under_seatbelt;
    spawn_command_under_seatbelt(command, sandbox_policy, cwd, stdio_policy, env).await
}

#[cfg(target_os = "linux")]
async fn spawn_command_under_sandbox(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    use codex_core::landlock::spawn_command_under_linux_sandbox;
    let codex_linux_sandbox_exe = assert_cmd::cargo::cargo_bin("codex-exec");
    spawn_command_under_linux_sandbox(
        codex_linux_sandbox_exe,
        command,
        sandbox_policy,
        cwd,
        stdio_policy,
        env,
    )
    .await
}

#[tokio::test]
async fn python_multiprocessing_lock_works_under_sandbox() {
    #[cfg(target_os = "macos")]
    let writable_roots = Vec::<PathBuf>::new();

    // From https://man7.org/linux/man-pages/man7/sem_overview.7.html
    //
    // > On Linux, named semaphores are created in a virtual filesystem,
    // > normally mounted under /dev/shm.
    #[cfg(target_os = "linux")]
    let writable_roots = vec![PathBuf::from("/dev/shm")];

    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots,
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

    let mut child = spawn_command_under_sandbox(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            python_code.to_string(),
        ],
        &policy,
        std::env::current_dir().expect("should be able to get current dir"),
        StdioPolicy::Inherit,
        HashMap::new(),
    )
    .await
    .expect("should be able to spawn python under sandbox");

    let status = child.wait().await.expect("should wait for child process");
    assert!(status.success(), "python exited with {status:?}");
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
    run_code_under_sandbox(
        "allow_unix_socketpair_recvfrom",
        &SandboxPolicy::ReadOnly,
        || async { unix_sock_body() },
    )
    .await
    .expect("should be able to reexec");
}

const IN_SANDBOX_ENV_VAR: &str = "IN_SANDBOX";

#[expect(clippy::expect_used)]
pub async fn run_code_under_sandbox<F, Fut>(
    test_selector: &str,
    policy: &SandboxPolicy,
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
        let mut child = spawn_command_under_sandbox(
            cmds,
            policy,
            std::env::current_dir().expect("should be able to get current dir"),
            stdio_policy,
            HashMap::from([("IN_SANDBOX".into(), "1".into())]),
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
