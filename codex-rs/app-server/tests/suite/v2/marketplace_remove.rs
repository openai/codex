use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::MarketplaceRemoveParams;
use codex_app_server_protocol::MarketplaceRemoveResponse;
use codex_app_server_protocol::RequestId;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_core::config::set_project_trust_level;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

fn configured_marketplace_update() -> MarketplaceConfigUpdate<'static> {
    MarketplaceConfigUpdate {
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

#[test_case(false; "snapshot only")]
#[test_case(true; "user entry and snapshot")]
#[tokio::test]
async fn marketplace_remove_deletes_config_and_installed_root(user_entry: bool) -> Result<()> {
    let codex_home = TempDir::new()?;
    if user_entry {
        record_user_marketplace(codex_home.path(), "debug", &configured_marketplace_update())?;
    }
    write_installed_marketplace(codex_home.path(), "debug")?;
    let installed_root = marketplace_install_root(codex_home.path()).join("debug");

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let response: MarketplaceRemoveResponse = mcp
        .request(|request_id| ClientRequest::MarketplaceRemove {
            request_id,
            params: MarketplaceRemoveParams {
                marketplace_name: "debug".to_string(),
            },
        })
        .await?;
    assert_eq!(response.marketplace_name, "debug");
    let removed_installed_root = response
        .installed_root
        .context("marketplace/remove should return removed installed root")?;
    assert_eq!(
        canonicalize_path_with_existing_parent(removed_installed_root.as_path())?,
        canonicalize_path_with_existing_parent(&installed_root)?,
    );

    let config_path = codex_home.path().join("config.toml");
    if user_entry {
        let config = std::fs::read_to_string(config_path)?;
        assert!(!config.contains("[marketplaces.debug]"));
    } else {
        assert!(!config_path.exists());
    }
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

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

#[tokio::test]
async fn marketplace_remove_ignores_startup_project_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    // TestAppServer starts in CODEX_HOME, so put project config there to verify
    // that the global marketplace RPC does not use it.
    std::fs::create_dir_all(codex_home.path().join(".git"))?;
    std::fs::create_dir_all(codex_home.path().join(".codex"))?;
    let project_config_path = codex_home.path().join(".codex/config.toml");
    let project_config = "[marketplaces.debug]\nsource_type = \"git\"\nsource = \"https://github.com/owner/repo.git\"\n";
    std::fs::write(&project_config_path, project_config)?;
    set_project_trust_level(codex_home.path(), codex_home.path(), TrustLevel::Trusted)?;
    write_installed_marketplace(codex_home.path(), "debug")?;
    let snapshot_path =
        marketplace_install_root(codex_home.path()).join("debug/.agents/plugins/marketplace.json");

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let response: MarketplaceRemoveResponse = mcp
        .request(|request_id| ClientRequest::MarketplaceRemove {
            request_id,
            params: MarketplaceRemoveParams {
                marketplace_name: "debug".to_string(),
            },
        })
        .await?;

    assert_eq!(response.marketplace_name, "debug");
    assert!(response.installed_root.is_some());
    assert_eq!(
        std::fs::read_to_string(project_config_path)?,
        project_config
    );
    assert!(!snapshot_path.exists());
    Ok(())
}
