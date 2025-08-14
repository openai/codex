// TODO(mbolin): Update this test to run on Linux, as well.
// (Should rename the test as part of that work.)
#[cfg(target_os = "macos")]
#[tokio::test]
async fn python_multiprocessing_lock_works_under_seatbelt() {
    #![expect(clippy::expect_used)]
    use codex_core::protocol::SandboxPolicy;
    use codex_core::seatbelt::spawn_command_under_seatbelt;
    use codex_core::spawn::StdioPolicy;
    use std::collections::HashMap;

    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
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

    let mut child = spawn_command_under_seatbelt(
        vec![
            "python3".to_string(),
            "-c".to_string(),
            python_code.to_string(),
        ],
        &policy,
        std::env::current_dir().expect("should be able to get current dir"),
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("should be able to spawn python under seatbelt");

    let status = child.wait().await.expect("should wait for child process");
    assert!(status.success(), "python exited with {status:?}");
}
