#![cfg(windows)]

use std::collections::HashMap;
use std::path::PathBuf;

use codex_core::exec::ExecParams;
use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::SandboxPolicy;
use codex_core::safety::set_windows_sandbox_enabled;

struct WindowsSandboxGuard;

impl WindowsSandboxGuard {
    fn enable() -> Self {
        set_windows_sandbox_enabled(true);
        Self
    }
}

impl Drop for WindowsSandboxGuard {
    fn drop(&mut self) {
        set_windows_sandbox_enabled(false);
    }
}

fn windows_workspace_policy(root: &PathBuf) -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![root.clone()],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

#[tokio::test]
async fn exec_tool_uses_windows_sandbox() {
    let _guard = WindowsSandboxGuard::enable();
    let temp = tempfile::tempdir().expect("tempdir");
    let cwd = temp.path().to_path_buf();
    let policy = windows_workspace_policy(&cwd);
    let params = ExecParams {
        command: vec![
            "cmd.exe".to_string(),
            "/C".to_string(),
            "set CODEX_SANDBOX".to_string(),
        ],
        cwd: cwd.clone(),
        timeout_ms: None,
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
    };

    let output = process_exec_tool_call(
        params,
        SandboxType::WindowsAppContainer,
        &policy,
        temp.path(),
        &None,
        None,
    )
    .await
    .expect("exec output");

    assert_eq!(output.exit_code, 0);
    assert!(
        output
            .aggregated_output
            .text
            .to_lowercase()
            .contains("codex_sandbox=windows_appcontainer")
    );
}
