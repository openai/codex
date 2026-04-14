use anyhow::Result;
use app_test_support::McpProcess;
use codex_app_server_protocol::MarketplaceAddParams;
use codex_app_server_protocol::RequestId;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn marketplace_add_rejects_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_add_request(MarketplaceAddParams {
            source: "./marketplace".to_string(),
            ref_name: None,
            sparse_paths: None,
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error.message.contains(
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo"
        ),
        "unexpected error: {}",
        err.error.message
    );
    Ok(())
}
