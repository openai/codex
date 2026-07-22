use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::MarketplaceAddParams;
use codex_app_server_protocol::MarketplaceAddResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn marketplace_add_local_directory_source() -> Result<()> {
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
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_marketplace_add_request(MarketplaceAddParams {
            source: "./marketplace".to_string(),
            ref_name: None,
            sparse_paths: None,
        })
        .await?;

    let MarketplaceAddResponse {
        marketplace_name,
        installed_root,
        already_added,
    } = timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let expected_root = AbsolutePathBuf::from_absolute_path(source.canonicalize()?)?;

    assert_eq!(marketplace_name, "debug");
    assert_eq!(installed_root, expected_root);
    assert!(!already_added);
    assert_eq!(
        std::fs::read_to_string(installed_root.as_path().join("plugins/sample/marker.txt"))?,
        "local ref"
    );
    Ok(())
}
