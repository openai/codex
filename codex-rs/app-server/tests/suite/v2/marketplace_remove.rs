use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MarketplaceRemoveParams;
use codex_app_server_protocol::MarketplaceRemoveResponse;
use codex_app_server_protocol::RequestId;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

fn configured_marketplace_update() -> MarketplaceConfigUpdate<'static> {
    MarketplaceConfigUpdate {
        last_updated: "2026-04-13T00:00:00Z",
        last_revision: None,
        source_type: "git",
        source: "https://github.com/owner/repo.git",
        ref_name: Some("main"),
        sparse_paths: &[],
    }
}

fn write_installed_marketplace(codex_home: &std::path::Path, marketplace_name: &str) -> Result<()> {
    let root = marketplace_install_root(codex_home).join(marketplace_name);
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    std::fs::write(root.join(".agents/plugins/marketplace.json"), "{}")?;
    Ok(())
}

fn canonicalize_path_with_existing_parent(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("path {} should have a parent", path.display()))?;
    let file_name = path
        .file_name()
        .with_context(|| format!("path {} should have a file name", path.display()))?;

    Ok(parent.canonicalize()?.join(file_name))
}

#[tokio::test]
async fn marketplace_remove_deletes_config_and_installed_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    record_user_marketplace(codex_home.path(), "debug", &configured_marketplace_update())?;
    write_installed_marketplace(codex_home.path(), "debug")?;
    let installed_root = marketplace_install_root(codex_home.path()).join("debug");

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_remove_request(MarketplaceRemoveParams {
            marketplace_name: "debug".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: MarketplaceRemoveResponse = to_response(response)?;
    assert_eq!(response.marketplace_name, "debug");
    let removed_installed_root = response
        .installed_root
        .context("marketplace/remove should return removed installed root")?;
    assert_eq!(
        canonicalize_path_with_existing_parent(removed_installed_root.as_path())?,
        canonicalize_path_with_existing_parent(&installed_root)?,
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("[marketplaces.debug]"));
    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_remove_rejects_unknown_marketplace() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_remove_request(MarketplaceRemoveParams {
            marketplace_name: "debug".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert_eq!(
        err.error.message,
        "marketplace `debug` is not configured or installed",
    );
    Ok(())
}
