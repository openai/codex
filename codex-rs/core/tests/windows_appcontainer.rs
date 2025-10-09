#![cfg(all(
    windows,
    feature = "windows_appcontainer_command_ext",
    feature = "windows_appcontainer_command_ext_raw_attribute",
    feature = "windows_appcontainer_raw_attribute_api",
))]

use codex_core::protocol::SandboxPolicy;
use codex_core::spawn::StdioPolicy;
use codex_core::windows_appcontainer::spawn_command_under_windows_appcontainer;
use std::collections::HashMap;
use tokio::io::AsyncReadExt;

fn windows_workspace_policy() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

#[tokio::test]
async fn windows_appcontainer_writes_to_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cwd = temp.path().to_path_buf();
    let policy_cwd = cwd.clone();
    let mut child = spawn_command_under_windows_appcontainer(
        vec![
            "cmd.exe".to_string(),
            "/C".to_string(),
            "echo hello>out.txt".to_string(),
        ],
        cwd.clone(),
        &windows_workspace_policy(),
        &policy_cwd,
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("spawn cmd");

    let status = child.wait().await.expect("wait");
    assert!(status.success(), "cmd.exe failed: {status:?}");

    let contents = tokio::fs::read_to_string(temp.path().join("out.txt"))
        .await
        .expect("read redirected output");
    assert!(contents.to_lowercase().contains("hello"));
}

#[tokio::test]
async fn windows_appcontainer_sets_env_flags() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cwd = temp.path().to_path_buf();
    let policy_cwd = cwd.clone();
    let mut child = spawn_command_under_windows_appcontainer(
        vec![
            "cmd.exe".to_string(),
            "/C".to_string(),
            "set CODEX_SANDBOX".to_string(),
        ],
        cwd,
        &windows_workspace_policy(),
        &policy_cwd,
        StdioPolicy::RedirectForShellTool,
        HashMap::new(),
    )
    .await
    .expect("spawn cmd");

    let mut stdout = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        out.read_to_end(&mut stdout).await.expect("stdout");
    }
    let status = child.wait().await.expect("wait");
    assert!(status.success(), "cmd.exe env probe failed: {status:?}");
    let stdout_text = String::from_utf8_lossy(&stdout).to_lowercase();
    assert!(stdout_text.contains("codex_sandbox=windows_appcontainer"));
    assert!(stdout_text.contains("codex_sandbox_network_disabled=1"));
}
