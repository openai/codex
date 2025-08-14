#![cfg(unix)]
#![expect(clippy::expect_used)]

use codex_core::protocol::SandboxPolicy;
use codex_core::spawn::StdioPolicy;
use std::collections::HashMap;
use std::path::PathBuf;
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
