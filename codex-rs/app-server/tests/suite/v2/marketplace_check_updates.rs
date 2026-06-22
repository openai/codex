use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MarketplaceCheckUpdatesParams;
use codex_app_server_protocol::MarketplaceCheckUpdatesResponse;
use codex_app_server_protocol::MarketplaceUpdateCheckResult;
use codex_app_server_protocol::MarketplaceUpgradeParams;
use codex_app_server_protocol::RequestId;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

fn run_git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").current_dir(cwd).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn init_marketplace_repo(root: &Path, marketplace_name: &str) -> Result<String> {
    run_git(root, &["init"])?;
    run_git(root, &["config", "user.email", "codex@example.com"])?;
    run_git(root, &["config", "user.name", "Codex Tests"])?;
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    std::fs::write(
        root.join(".agents/plugins/marketplace.json"),
        format!(r#"{{"name":"{marketplace_name}","plugins":[]}}"#),
    )?;
    std::fs::write(root.join("marker.txt"), "initial")?;
    run_git(root, &["add", "."])?;
    run_git(root, &["commit", "-m", "initial marketplace"])?;
    run_git(root, &["rev-parse", "HEAD"])
}

fn commit_marketplace_update(root: &Path) -> Result<String> {
    std::fs::write(root.join("marker.txt"), "updated")?;
    run_git(root, &["add", "marker.txt"])?;
    run_git(root, &["commit", "-m", "update marketplace"])?;
    run_git(root, &["rev-parse", "HEAD"])
}

fn record_git_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
    source: &Path,
    last_revision: Option<&str>,
) -> Result<()> {
    let source = source.display().to_string();
    record_user_marketplace(
        codex_home,
        marketplace_name,
        &MarketplaceConfigUpdate {
            last_updated: "2026-04-13T00:00:00Z",
            last_revision,
            source_type: "git",
            source: &source,
            ref_name: None,
            sparse_paths: &[],
        },
    )?;
    Ok(())
}

fn record_local_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
    source: &Path,
) -> Result<()> {
    let source = source.display().to_string();
    record_user_marketplace(
        codex_home,
        marketplace_name,
        &MarketplaceConfigUpdate {
            last_updated: "2026-04-13T00:00:00Z",
            last_revision: None,
            source_type: "local",
            source: &source,
            ref_name: None,
            sparse_paths: &[],
        },
    )?;
    Ok(())
}

fn disable_plugin_startup_tasks(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!("{config}\n[features]\nplugins = false\n"),
    )?;
    Ok(())
}

async fn send_marketplace_check_updates(
    mcp: &mut TestAppServer,
    marketplace_name: Option<&str>,
) -> Result<MarketplaceCheckUpdatesResponse> {
    let request_id = mcp
        .send_marketplace_check_updates_request(MarketplaceCheckUpdatesParams {
            marketplace_name: marketplace_name.map(str::to_string),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn install_marketplace(mcp: &mut TestAppServer, marketplace_name: &str) -> Result<()> {
    let request_id = mcp
        .send_marketplace_upgrade_request(MarketplaceUpgradeParams {
            marketplace_name: Some(marketplace_name.to_string()),
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    Ok(())
}

#[tokio::test]
async fn marketplace_check_updates_reports_each_status_without_mutating_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    let current_source = TempDir::new()?;
    let changed_source = TempDir::new()?;
    let broken_source = TempDir::new()?;
    let current_revision = init_marketplace_repo(current_source.path(), "current")?;
    let changed_revision = init_marketplace_repo(changed_source.path(), "changed")?;
    commit_marketplace_update(changed_source.path())?;
    record_git_marketplace(
        codex_home.path(),
        "current",
        current_source.path(),
        Some(&current_revision),
    )?;
    record_git_marketplace(
        codex_home.path(),
        "changed",
        changed_source.path(),
        Some(&changed_revision),
    )?;
    record_git_marketplace(
        codex_home.path(),
        "broken",
        broken_source.path(),
        Some("0000000000000000000000000000000000000000"),
    )?;
    disable_plugin_startup_tasks(codex_home.path())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    install_marketplace(&mut mcp, "current").await?;
    let config_before = std::fs::read_to_string(codex_home.path().join("config.toml"))?;

    let response = send_marketplace_check_updates(&mut mcp, None).await?;

    let [broken, changed, current] = response.results.as_slice() else {
        anyhow::bail!("expected three marketplace update results");
    };
    assert!(matches!(
        broken,
        MarketplaceUpdateCheckResult::Error {
            marketplace_name,
            message,
        } if marketplace_name == "broken" && !message.is_empty()
    ));
    assert_eq!(
        [changed.clone(), current.clone()],
        [
            MarketplaceUpdateCheckResult::UpdateAvailable {
                marketplace_name: "changed".to_string(),
            },
            MarketplaceUpdateCheckResult::UpToDate {
                marketplace_name: "current".to_string(),
            },
        ]
    );
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("config.toml"))?,
        config_before
    );
    assert!(!codex_home.path().join(".tmp/marketplaces/changed").exists());
    Ok(())
}

#[tokio::test]
async fn marketplace_check_updates_reports_missing_or_damaged_snapshots() -> Result<()> {
    let codex_home = TempDir::new()?;
    let damaged_source = TempDir::new()?;
    let missing_source = TempDir::new()?;
    let damaged_revision = init_marketplace_repo(damaged_source.path(), "damaged")?;
    let missing_revision = init_marketplace_repo(missing_source.path(), "missing")?;
    record_git_marketplace(
        codex_home.path(),
        "damaged",
        damaged_source.path(),
        Some(&damaged_revision),
    )?;
    record_git_marketplace(
        codex_home.path(),
        "missing",
        missing_source.path(),
        Some(&missing_revision),
    )?;
    disable_plugin_startup_tasks(codex_home.path())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    install_marketplace(&mut mcp, "damaged").await?;
    std::fs::write(
        codex_home
            .path()
            .join(".tmp/marketplaces/damaged/.codex-marketplace-install.json"),
        "{}",
    )?;

    assert_eq!(
        send_marketplace_check_updates(&mut mcp, None).await?,
        MarketplaceCheckUpdatesResponse {
            results: vec![
                MarketplaceUpdateCheckResult::UpdateAvailable {
                    marketplace_name: "damaged".to_string(),
                },
                MarketplaceUpdateCheckResult::UpdateAvailable {
                    marketplace_name: "missing".to_string(),
                },
            ],
        }
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_check_updates_supports_named_selection_and_rejects_non_git_names() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let git_source = TempDir::new()?;
    let local_source = TempDir::new()?;
    let revision = init_marketplace_repo(git_source.path(), "git-marketplace")?;
    record_git_marketplace(
        codex_home.path(),
        "git-marketplace",
        git_source.path(),
        Some(&revision),
    )?;
    record_local_marketplace(codex_home.path(), "local-only", local_source.path())?;
    disable_plugin_startup_tasks(codex_home.path())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    assert_eq!(
        send_marketplace_check_updates(&mut mcp, Some("git-marketplace")).await?,
        MarketplaceCheckUpdatesResponse {
            results: vec![MarketplaceUpdateCheckResult::UpdateAvailable {
                marketplace_name: "git-marketplace".to_string(),
            }],
        }
    );

    for marketplace_name in ["local-only", "missing"] {
        let request_id = mcp
            .send_marketplace_check_updates_request(MarketplaceCheckUpdatesParams {
                marketplace_name: Some(marketplace_name.to_string()),
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
            format!("marketplace `{marketplace_name}` is not configured as a Git marketplace"),
        );
    }
    Ok(())
}
