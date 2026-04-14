use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MarketplaceAddParams;
use codex_app_server_protocol::MarketplaceAddResponse;
use codex_app_server_protocol::RequestId;
use codex_core::plugins::marketplace_install_root;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn marketplace_add_supports_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = codex_home.path().join("marketplace");
    std::fs::create_dir_all(source.join(".agents/plugins"))?;
    std::fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
    std::fs::write(
        source.join(".agents/plugins/marketplace.json"),
        r#"{"name":"debug","plugins":[]}"#,
    )?;
    std::fs::write(
        source.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )?;
    std::fs::write(source.join("plugins/sample/marker.txt"), "local ref")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_add_request(MarketplaceAddParams {
            source: "./marketplace".to_string(),
            ref_name: None,
            sparse_paths: None,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let MarketplaceAddResponse {
        marketplace_name,
        installed_root,
        already_added,
    } = to_response(response)?;
    let expected_root = marketplace_install_root(codex_home.path())
        .join("debug")
        .canonicalize()?;

    assert_eq!(marketplace_name, "debug");
    assert_eq!(installed_root.as_path(), expected_root.as_path());
    assert!(!already_added);
    assert_eq!(
        std::fs::read_to_string(installed_root.as_path().join("plugins/sample/marker.txt"))?,
        "local ref"
    );
    Ok(())
}
