use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_utils_cargo_bin::CargoBinError;
use futures::FutureExt as _;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::InitializeRequestParams;
use serde_json::json;

fn rmcp_test_server_bin() -> Result<PathBuf, CargoBinError> {
    codex_utils_cargo_bin::cargo_bin("rmcp_test_server")
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities {
            experimental: None,
            roots: None,
            sampling: None,
            elicitation: Some(json!({})),
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex tool_list_changed test".into()),
            user_agent: None,
        },
        protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_string(),
    }
}

/// Verify that when the MCP server sends `notifications/tools/list_changed`,
/// the client's `OnToolListChanged` callback is invoked and the tool list
/// is updated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_list_changed_callback_is_invoked() -> anyhow::Result<()> {
    let callback_count = Arc::new(AtomicUsize::new(0));

    let client = RmcpClient::new_stdio_client(
        OsString::from(rmcp_test_server_bin()?),
        Vec::<OsString>::new(),
        None,
        &[],
        None,
    )
    .await?;

    let count_clone = Arc::clone(&callback_count);
    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: None,
                    })
                }
                .boxed()
            }),
            Box::new(move || {
                let count = Arc::clone(&count_clone);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                }
                .boxed()
            }),
        )
        .await?;

    // Confirm initial tool list has 2 tools (echo + trigger_list_changed).
    let tools_before = client
        .list_tools(None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(tools_before.tools.len(), 2, "expected 2 initial tools");
    assert_eq!(callback_count.load(Ordering::SeqCst), 0);

    // Call the trigger tool — server adds new_tool and sends the notification.
    client
        .call_tool(
            "trigger_list_changed".to_string(),
            Some(serde_json::json!({})),
            Some(Duration::from_secs(5)),
        )
        .await?;

    // Give the notification a moment to be processed.
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        callback_count.load(Ordering::SeqCst),
        1,
        "OnToolListChanged callback should have been invoked once"
    );

    // Verify the client sees the updated tool list.
    let tools_after = client
        .list_tools(None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(
        tools_after.tools.len(),
        3,
        "tool list should have grown to 3 after notification"
    );
    assert!(
        tools_after.tools.iter().any(|t| t.name == "new_tool"),
        "new_tool should be present after list_changed"
    );

    Ok(())
}
