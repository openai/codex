mod common;

use std::path::Path;

use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_core::protocol::ReviewDecision;
use codex_mcp_server::ExecApprovalElicitRequestParams;
use codex_mcp_server::ExecApprovalResponse;
use mcp_types::ElicitRequest;
use mcp_types::ElicitRequestParamsRequestedSchema;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::common::McpProcess;
use crate::common::create_final_assistant_message_sse_response;
use crate::common::create_mock_chat_completions_server;
use crate::common::create_shell_sse_response;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Test that a shell command that is not on the "trusted" list triggers an
/// elicitation request to the MCP and that sending the approval runs the
/// command, as expected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_shell_command_approval_triggers_elicitation() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Apparently `#[tokio::test]` must return `()`, so we create a helper
    // function that returns `Result` so we can use `?` in favor of `unwrap`.
    if let Err(err) = shell_command_approval_triggers_elicitation().await {
        panic!("failure: {err}");
    }
}

async fn shell_command_approval_triggers_elicitation() -> anyhow::Result<()> {
    // We use `git init` because it will not be on the "trusted" list.
    let shell_command = vec!["git".to_string(), "init".to_string()];
    let workdir_for_shell_function_call = TempDir::new()?;

    // Configure the mock server so it makes two responses:
    // 1. The first response is a shell function call that will trigger an
    //    elicitation request.
    // 2. The second response is the final assistant message that should be
    //    returned after the elicitation is approved and the command is run.
    let server = create_mock_chat_completions_server(vec![
        create_shell_sse_response(
            shell_command.clone(),
            Some(workdir_for_shell_function_call.path()),
            Some(5_000),
            "call1234",
        )?,
        create_final_assistant_message_sse_response("Enjoy your new git repo!")?,
    ])
    .await;

    // Run `codex mcp` with a specific config.toml.
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), server.uri())?;
    let mut mcp_process = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send a "codex" tool request, which should hit the completions endpoint.
    // In turn, it should reply with a tool call, which the MCP should forward
    // as an elicitation.
    let codex_request_id = mcp_process.send_codex_tool_call("run `git init`").await?;
    let elicitation_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_request_message(),
    )
    .await??;

    // This is the first request from the server, so the id should be 0 given
    // how things are currently implemented.
    let elicitation_request_id = RequestId::Integer(0);
    let expected_elicitation_request = create_expected_elicitation_request(
        elicitation_request_id.clone(),
        shell_command.clone(),
        workdir_for_shell_function_call.path(),
        codex_request_id.to_string(),
        // Internal Codex id: empirically it is 1, but this is
        // admittedly an internal detail that could change.
        "1".to_string(),
    )?;
    assert_eq!(expected_elicitation_request, elicitation_request);

    // Accept the `git init` request by responding to the elicitation.
    mcp_process
        .send_response(
            elicitation_request_id,
            serde_json::to_value(ExecApprovalResponse {
                decision: ReviewDecision::Approved,
            })?,
        )
        .await?;

    // Verify the original `codex` tool call completes and that `git init` ran
    // successfully.
    let codex_response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(codex_request_id)),
    )
    .await??;
    assert_eq!(
        JSONRPCResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id: RequestId::Integer(codex_request_id),
            result: json!({
                "content": [
                    {
                        "text": "Enjoy your new git repo!",
                        "type": "text"
                    }
                ]
            }),
        },
        codex_response
    );

    assert!(
        workdir_for_shell_function_call.path().join(".git").is_dir(),
        ".git folder should have been created"
    );

    Ok(())
}

/// Create a Codex config that uses the mock server as the model provider.
/// It also uses `approval_policy = "untrusted"` so that we exercise the
/// elicitation code path for shell commands.
fn create_config_toml(codex_home: &Path, server_uri: String) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "untrusted"
sandbox_policy = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn create_expected_elicitation_request(
    elicitation_request_id: RequestId,
    command: Vec<String>,
    workdir: &Path,
    codex_mcp_tool_call_id: String,
    codex_event_id: String,
) -> anyhow::Result<JSONRPCRequest> {
    let expected_message = format!(
        "Allow Codex to run `{}` in `{}`?",
        shlex::try_join(command.iter().map(|s| s.as_ref()))?,
        workdir.to_string_lossy()
    );
    Ok(JSONRPCRequest {
        jsonrpc: JSONRPC_VERSION.into(),
        id: elicitation_request_id,
        method: ElicitRequest::METHOD.to_string(),
        params: Some(serde_json::to_value(&ExecApprovalElicitRequestParams {
            message: expected_message,
            requested_schema: ElicitRequestParamsRequestedSchema {
                r#type: "object".to_string(),
                properties: json!({}),
                required: None,
            },
            codex_elicitation: "exec-approval".to_string(),
            codex_mcp_tool_call_id,
            codex_event_id,
            codex_command: command,
            codex_cwd: workdir.to_path_buf(),
        })?),
    })
}
