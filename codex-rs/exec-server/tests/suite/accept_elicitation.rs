#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_exec_server::ExecResult;
use exec_server_test_support::InteractiveClient;
use exec_server_test_support::create_transport;
use exec_server_test_support::notify_readable_sandbox;
use exec_server_test_support::write_default_execpolicy;
use maplit::hashset;
use pretty_assertions::assert_eq;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParam;
use rmcp::model::CallToolResult;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::EmptyResult;
use rmcp::model::ServerResult;
use rmcp::model::object;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use tempfile::TempDir;
use tokio::process::Command;

const USE_LOGIN_SHELL: bool = false;

/// Verify that when using a read-only sandbox and an execpolicy that prompts,
/// the proper elicitation is sent. Upon auto-approving the elicitation, the
/// command should be run privileged outside the sandbox.
#[tokio::test(flavor = "current_thread")]
async fn accept_elicitation_for_prompt_rule() -> Result<()> {
    // Configure a stdio transport that will launch the MCP server using
    // $CODEX_HOME with an execpolicy that prompts for `git init` commands.
    let codex_home = TempDir::new()?;
    write_default_execpolicy(
        r#"
# Create a rule with `decision = "prompt"` to exercise the elicitation flow.
prefix_rule(
  pattern = ["git", "init"],
  decision = "prompt",
  match = [
    "git init ."
  ],
)
"#,
        codex_home.as_ref(),
    )
    .await?;
    let dotslash_cache_temp_dir = TempDir::new()?;
    let dotslash_cache = dotslash_cache_temp_dir.path();
    let transport = create_transport(codex_home.as_ref(), dotslash_cache).await?;

    // Create an MCP client that approves expected elicitation messages.
    let project_root = TempDir::new()?;
    let project_root_path = project_root.path().canonicalize().unwrap();
    let git_path = resolve_git_path(USE_LOGIN_SHELL).await?;
    let expected_elicitation_message = format!(
        "Allow agent to run `{} init .` in `{}`?",
        git_path,
        project_root_path.display()
    );
    let elicitation_requests: Arc<Mutex<Vec<CreateElicitationRequestParam>>> = Default::default();
    let client = InteractiveClient {
        elicitations_to_accept: hashset! { expected_elicitation_message.clone() },
        elicitation_requests: elicitation_requests.clone(),
    };

    // Start the MCP server.
    let service: rmcp::service::RunningService<rmcp::RoleClient, InteractiveClient> =
        client.serve(transport).await?;

    // Notify the MCP server about the current sandbox state before making any
    // `shell` tool calls.
    let linux_sandbox_exe_folder = TempDir::new()?;
    let codex_linux_sandbox_exe = if cfg!(target_os = "linux") {
        let codex_linux_sandbox_exe = linux_sandbox_exe_folder.path().join("codex-linux-sandbox");
        let codex_cli = ensure_codex_cli()?;
        symlink(&codex_cli, &codex_linux_sandbox_exe)?;
        Some(codex_linux_sandbox_exe)
    } else {
        None
    };
    let response =
        notify_readable_sandbox(&project_root_path, codex_linux_sandbox_exe, &service).await?;
    let ServerResult::EmptyResult(EmptyResult {}) = response else {
        panic!("expected EmptyResult from sandbox state notification but found: {response:?}");
    };

    // Call the shell tool and verify that an elicitation was created and
    // auto-approved.
    let CallToolResult {
        content, is_error, ..
    } = service
        .call_tool(CallToolRequestParam {
            name: Cow::Borrowed("shell"),
            arguments: Some(object(json!(
                {
                    "login": USE_LOGIN_SHELL,
                    "command": "git init .",
                    "workdir": project_root_path.to_string_lossy(),
                }
            ))),
        })
        .await?;
    let tool_call_content = content
        .first()
        .expect("expected non-empty content")
        .as_text()
        .expect("expected text content");
    let ExecResult {
        exit_code, output, ..
    } = serde_json::from_str::<ExecResult>(&tool_call_content.text)?;
    let git_init_succeeded = format!(
        "Initialized empty Git repository in {}/.git/\n",
        project_root_path.display()
    );
    // Normally, this would be an exact match, but it might include extra output
    // if `git config set advice.defaultBranchName false` has not been set.
    assert!(
        output.contains(&git_init_succeeded),
        "expected output `{output}` to contain `{git_init_succeeded}`"
    );
    assert_eq!(exit_code, 0, "command should succeed");
    assert_eq!(is_error, Some(false), "command should succeed");
    assert!(
        project_root_path.join(".git").is_dir(),
        "git repo should exist"
    );

    let elicitation_messages = elicitation_requests
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.message.clone())
        .collect::<Vec<_>>();
    assert_eq!(vec![expected_elicitation_message], elicitation_messages);

    Ok(())
}

fn ensure_codex_cli() -> Result<PathBuf> {
    let codex_cli = codex_utils_cargo_bin::cargo_bin("codex")?;

    let metadata = codex_cli.metadata().with_context(|| {
        format!(
            "failed to read metadata for codex binary at {}",
            codex_cli.display()
        )
    })?;
    ensure!(
        metadata.is_file(),
        "expected codex binary at {} to be a file; run `cargo build -p codex-cli --bin codex` before this test",
        codex_cli.display()
    );

    let mode = metadata.permissions().mode();
    ensure!(
        mode & 0o111 != 0,
        "codex binary at {} is not executable (mode {mode:o}); run `cargo build -p codex-cli --bin codex` before this test",
        codex_cli.display()
    );

    Ok(codex_cli)
}

async fn resolve_git_path(use_login_shell: bool) -> Result<String> {
    let bash_flag = if use_login_shell { "-lc" } else { "-c" };
    let git = Command::new("bash")
        .arg(bash_flag)
        .arg("command -v git")
        .output()
        .await
        .context("failed to resolve git via login shell")?;
    ensure!(
        git.status.success(),
        "failed to resolve git via login shell: {}",
        String::from_utf8_lossy(&git.stderr)
    );
    let git_path = String::from_utf8(git.stdout)
        .context("git path was not valid utf8")?
        .trim()
        .to_string();
    ensure!(!git_path.is_empty(), "git path should not be empty");
    Ok(git_path)
}
