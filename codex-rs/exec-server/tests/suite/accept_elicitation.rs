#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
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
use rmcp::model::object;
use serde_json::json;
use tempfile::TempDir;

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
    let transport = create_transport(codex_home.as_ref())?;

    // Create an MCP client that approves expected elicitation messages.
    let project_root = TempDir::new()?;
    let git = which::which("git")?;
    let project_root_path = project_root.path().canonicalize().unwrap();
    let expected_elicitation_message = format!(
        "Allow agent to run `{} init .` in `{}`?",
        git.display(),
        project_root_path.display()
    );
    let elicitation_requests: Arc<Mutex<Vec<CreateElicitationRequestParam>>> = Default::default();
    let client = InteractiveClient {
        elicitations_to_accept: hashset! { expected_elicitation_message.clone() },
        elicitation_requests: elicitation_requests.clone(),
    };

    // Start the MCP server and notify it about the readable sandbox.
    let service = client.serve(transport).await?;
    notify_readable_sandbox(&project_root_path, &service).await?;

    // Call the shell tool and verify that an elicitation was created and
    // auto-approved.
    let CallToolResult {
        content, is_error, ..
    } = service
        .call_tool(CallToolRequestParam {
            name: Cow::Borrowed("shell"),
            arguments: Some(object(json!(
                {
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
    assert_eq!(
        output,
        format!(
            "Initialized empty Git repository in {}/.git/\n",
            project_root_path.display()
        )
    );
    assert_eq!(exit_code, 0, "command should succeed");
    assert_eq!(is_error, Some(false), "command should succeed");

    let elicitation_messages = elicitation_requests
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.message.clone())
        .collect::<Vec<_>>();
    assert_eq!(vec![expected_elicitation_message], elicitation_messages);

    Ok(())
}
