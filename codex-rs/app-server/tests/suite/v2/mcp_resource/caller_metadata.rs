//! Public JSON-RPC coverage for caller metadata on a thread's stdio resource read.

use super::*;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_resource_receives_caller_metadata() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.custom]\ncommand = {}",
            toml::Value::String(stdio_server_bin()?),
        ))
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams::default(),
        })
        .await?;
    let mut params = json!({
        "threadId": thread.id,
        "server": "custom",
        "uri": "test://codex/request-metadata",
    });
    let metadata = json!({
        "com.openai/codex-statusline-state": {
            "config": {"statusline_items": ["model", "context-remaining"]},
            "thread": {"id": thread.id, "name": null},
        },
        "connector_id": "custom-connector",
        "link_id": "custom-link",
        "_codex_apps": {"resource_uri": "custom://route"},
    });
    for caller_metadata in [None, Some(&metadata)] {
        if let Some(meta) = caller_metadata {
            params["_meta"] = meta.clone();
        }
        let request = mcp
            .send_raw_request("mcpServer/resource/read", Some(params.clone()))
            .await?;
        let response: McpResourceReadResponse =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request)).await??;
        let [McpResourceContent::Text { text, .. }] = response.contents.as_slice() else {
            anyhow::bail!("expected the metadata echoed by the stdio server");
        };
        let received: serde_json::Value = serde_json::from_str(text)?;
        for key in metadata.as_object().unwrap().keys() {
            assert_eq!(
                received.get(key),
                caller_metadata.and_then(|meta| meta.get(key))
            );
        }
    }
    for (invalid_meta, expected_message) in [
        (json!({"threadId": "spoofed"}), "threadId"),
        (
            json!({"x-codex-turn-metadata": {}}),
            "x-codex-turn-metadata",
        ),
        (
            json!({"codex/connectionLease": "spoofed"}),
            "codex/connectionLease",
        ),
        (json!([]), "map"),
    ] {
        params["_meta"] = invalid_meta;
        let request = mcp
            .send_raw_request("mcpServer/resource/read", Some(params.clone()))
            .await?;
        let error = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_error_message(RequestId::Integer(request)),
        )
        .await??;
        assert_eq!(error.error.code, -32600);
        assert!(
            error.error.message.contains(expected_message),
            "{}",
            error.error.message
        );
    }
    Ok(())
}
