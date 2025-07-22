mod common;

use std::path::Path;

use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_mcp_server::CodexToolCallParam;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::common::McpProcess;
use crate::common::create_final_assistant_message_sse_response;
use crate::common::create_mock_chat_completions_server;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_codex_tool_passes_base_instructions() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Apparently `#[tokio::test]` must return `()`, so we create a helper
    // function that returns `Result` so we can use `?` in favor of `unwrap`.
    if let Err(err) = codex_tool_passes_base_instructions().await {
        panic!("failure: {err}");
    }
}
async fn codex_tool_passes_base_instructions() -> anyhow::Result<()> {
    #![allow(clippy::unwrap_used)]

    let server =
        create_mock_chat_completions_server(vec![create_final_assistant_message_sse_response(
            "Enjoy!",
        )?])
        .await;

    // Run `codex mcp` with a specific config.toml.
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), server.uri())?;
    let mut mcp_process = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize()).await??;

    // Send a "codex" tool request, which should hit the completions endpoint.
    let codex_request_id = mcp_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "How are you?".to_string(),
            base_instructions: Some("You are a helpful assistant.".to_string()),
            ..Default::default()
        })
        .await?;

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
                        "text": "Enjoy!",
                        "type": "text"
                    }
                ]
            }),
        },
        codex_response
    );

    let requests = server.received_requests().await.unwrap();
    let request = requests[0].body_json::<serde_json::Value>().unwrap();
    let instructions = request["messages"][0]["content"].as_str().unwrap();
    assert!(instructions.starts_with("You are a helpful assistant."));

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
