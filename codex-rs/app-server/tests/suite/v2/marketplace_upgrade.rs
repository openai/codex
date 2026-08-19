use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::MarketplaceUpgradeParams;
use codex_app_server_protocol::MarketplaceUpgradeResponse;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListMarketplaceKind;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::RequestId;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const INSTALLED_MARKETPLACES_DIR: &str = ".tmp/marketplaces";

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

fn write_marketplace_files(root: &Path, marketplace_name: &str, marker: &str) -> Result<()> {
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    std::fs::write(
        root.join(".agents/plugins/marketplace.json"),
        format!(r#"{{"name":"{marketplace_name}","plugins":[]}}"#),
    )?;
    std::fs::write(root.join("marker.txt"), marker)?;
    Ok(())
}

fn init_marketplace_repo(root: &Path, marketplace_name: &str, marker: &str) -> Result<String> {
    run_git(root, &["init"])?;
    run_git(root, &["config", "user.email", "codex@example.com"])?;
    run_git(root, &["config", "user.name", "Codex Tests"])?;
    write_marketplace_files(root, marketplace_name, marker)?;
    run_git(root, &["add", "."])?;
    run_git(root, &["commit", "-m", "initial marketplace"])?;
    run_git(root, &["rev-parse", "HEAD"])
}

fn commit_marketplace_marker(root: &Path, marker: &str) -> Result<String> {
    std::fs::write(root.join("marker.txt"), marker)?;
    run_git(root, &["add", "marker.txt"])?;
    run_git(root, &["commit", "-m", "update marker"])?;
    run_git(root, &["rev-parse", "HEAD"])
}

fn configured_git_marketplace_update<'a>(
    source: &'a str,
    last_revision: Option<&'a str>,
    ref_name: Option<&'a str>,
) -> MarketplaceConfigUpdate<'a> {
    MarketplaceConfigUpdate {
        last_updated: "2026-04-13T00:00:00Z",
        last_revision,
        source_type: "git",
        source,
        ref_name,
        sparse_paths: &[],
    }
}

fn configured_local_marketplace_update(source: &str) -> MarketplaceConfigUpdate<'_> {
    MarketplaceConfigUpdate {
        last_updated: "2026-04-13T00:00:00Z",
        last_revision: None,
        source_type: "local",
        source,
        ref_name: None,
        sparse_paths: &[],
    }
}

fn record_git_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
    source: &Path,
    last_revision: &str,
    ref_name: Option<&str>,
) -> Result<()> {
    let source = source.display().to_string();
    record_user_marketplace(
        codex_home,
        marketplace_name,
        &configured_git_marketplace_update(&source, Some(last_revision), ref_name),
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

fn marketplace_install_root(codex_home: &Path) -> std::path::PathBuf {
    codex_home.join(INSTALLED_MARKETPLACES_DIR)
}

fn expected_installed_root(codex_home: &Path, marketplace_name: &str) -> Result<AbsolutePathBuf> {
    AbsolutePathBuf::try_from(
        marketplace_install_root(&codex_home.canonicalize()?).join(marketplace_name),
    )
    .context("expected installed root should be absolute")
}

async fn send_marketplace_upgrade(
    mcp: &mut TestAppServer,
    marketplace_name: Option<&str>,
) -> Result<MarketplaceUpgradeResponse> {
    mcp.request(|request_id| ClientRequest::MarketplaceUpgrade {
        request_id,
        params: MarketplaceUpgradeParams {
            marketplace_name: marketplace_name.map(str::to_string),
        },
    })
    .await
}

#[tokio::test]
async fn marketplace_upgrade_all_configured_git_marketplaces() -> Result<()> {
    let codex_home = TempDir::new()?;
    let debug_source = TempDir::new()?;
    let tools_source = TempDir::new()?;
    let debug_old_revision = init_marketplace_repo(debug_source.path(), "debug", "debug old")?;
    let tools_old_revision = init_marketplace_repo(tools_source.path(), "tools", "tools old")?;
    let debug_new_revision = commit_marketplace_marker(debug_source.path(), "debug new")?;
    let tools_new_revision = commit_marketplace_marker(tools_source.path(), "tools new")?;
    record_git_marketplace(
        codex_home.path(),
        "debug",
        debug_source.path(),
        &debug_old_revision,
        Some(&debug_new_revision),
    )?;
    record_git_marketplace(
        codex_home.path(),
        "tools",
        tools_source.path(),
        &tools_old_revision,
        Some(&tools_new_revision),
    )?;
    disable_plugin_startup_tasks(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let debug_root = expected_installed_root(codex_home.path(), "debug")?;
    let tools_root = expected_installed_root(codex_home.path(), "tools")?;
    let response = send_marketplace_upgrade(&mut mcp, /*marketplace_name*/ None).await?;

    assert_eq!(
        response,
        MarketplaceUpgradeResponse {
            selected_marketplaces: vec!["debug".to_string(), "tools".to_string()],
            upgraded_roots: vec![debug_root.clone(), tools_root.clone()],
            errors: Vec::new(),
        }
    );
    assert_eq!(
        std::fs::read_to_string(debug_root.as_path().join("marker.txt"))?,
        "debug new"
    );
    assert_eq!(
        std::fs::read_to_string(tools_root.as_path().join("marker.txt"))?,
        "tools new"
    );
    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains(&debug_new_revision));
    assert!(config.contains(&tools_new_revision));
    Ok(())
}

#[tokio::test]
async fn automatic_upgrade_isolates_git_while_explicit_install_preserves_configuration()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let plugin_source = TempDir::new()?;
    let old_revision = init_marketplace_repo(source.path(), "trusted", "old")?;

    init_marketplace_repo(plugin_source.path(), "plugin", "plugin")?;
    let plugin_root = plugin_source.path().join(".codex-plugin");
    std::fs::create_dir_all(&plugin_root)?;
    std::fs::write(
        plugin_root.join("plugin.json"),
        r#"{"name":"toolkit","version":"1.0.0","interface":{"displayName":"Toolkit"}}"#,
    )?;
    run_git(plugin_source.path(), &["add", "."])?;
    run_git(plugin_source.path(), &["commit", "-qm", "add plugin"])?;
    let plugin_url = url::Url::from_directory_path(plugin_source.path())
        .map_err(|()| anyhow::anyhow!("invalid plugin source path"))?
        .to_string();
    std::fs::write(
        source.path().join(".agents/plugins/marketplace.json"),
        serde_json::json!({
            "name": "trusted",
            "plugins": [{
                "name": "toolkit",
                "source": {"source": "url", "url": plugin_url},
            }],
        })
        .to_string(),
    )?;
    run_git(source.path(), &["add", ".agents/plugins/marketplace.json"])?;
    commit_marketplace_marker(source.path(), "new")?;
    record_git_marketplace(
        codex_home.path(),
        "trusted",
        source.path(),
        &old_revision,
        /*ref_name*/ None,
    )?;
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "{}\n[plugins.\"toolkit@trusted\"]\nenabled = true\n",
            std::fs::read_to_string(&config_path)?
        ),
    )?;

    run_git(codex_home.path(), &["init", "--quiet"])?;
    let source_path = source.path().display().to_string();
    let missing_path = codex_home.path().join("missing").display().to_string();
    let rewrite_key = format!("url.{missing_path}.insteadOf");
    for untrusted_source in [&source_path, &plugin_url] {
        run_git(
            codex_home.path(),
            &["config", "--add", &rewrite_key, untrusted_source],
        )?;
        assert!(run_git(codex_home.path(), &["ls-remote", untrusted_source, "HEAD"]).is_err());
    }

    let manual_plugin_alias = "https://manual.example/plugin.git";
    let manual_plugin_rewrite = format!("url.{}.insteadOf", plugin_source.path().display());
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_plugin_startup_tasks()
        .with_env_overrides(&[
            ("GIT_CONFIG_COUNT", Some("1")),
            ("GIT_CONFIG_KEY_0", Some(&manual_plugin_rewrite)),
            ("GIT_CONFIG_VALUE_0", Some(manual_plugin_alias)),
        ])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let plugin_root = codex_home
        .path()
        .join("plugins/cache/trusted/toolkit/1.0.0/.codex-plugin/plugin.json");
    timeout(DEFAULT_TIMEOUT, async {
        while !plugin_root.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;

    let marketplace_path = marketplace_install_root(codex_home.path())
        .join("trusted/.agents/plugins/marketplace.json");
    std::fs::write(
        &marketplace_path,
        serde_json::json!({
            "name": "trusted",
            "plugins": [{
                "name": "toolkit",
                "source": {"source": "url", "url": manual_plugin_alias},
            }],
        })
        .to_string(),
    )?;
    let marketplace_path = AbsolutePathBuf::try_from(marketplace_path)?;
    let _: PluginInstallResponse = server
        .request(|request_id| ClientRequest::PluginInstall {
            request_id,
            params: PluginInstallParams {
                marketplace_path: Some(marketplace_path),
                remote_marketplace_name: None,
                install_attempt_id: None,
                plugin_name: "toolkit".to_string(),
            },
        })
        .await?;

    std::fs::write(
        plugin_source.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"toolkit","version":"1.1.0","interface":{"displayName":"Toolkit"}}"#,
    )?;
    run_git(plugin_source.path(), &["add", "."])?;
    run_git(plugin_source.path(), &["commit", "-qm", "upgrade plugin"])?;
    let _: PluginListResponse = server
        .request(|request_id| ClientRequest::PluginList {
            request_id,
            params: PluginListParams {
                cwds: None,
                marketplace_kinds: Some(vec![PluginListMarketplaceKind::Local]),
                force_refetch: true,
            },
        })
        .await?;
    assert!(
        codex_home
            .path()
            .join("plugins/cache/trusted/toolkit/1.1.0/.codex-plugin/plugin.json")
            .is_file(),
        "explicit plugin/list refresh must preserve command-scoped Git configuration"
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_named_marketplace_only() -> Result<()> {
    let codex_home = TempDir::new()?;
    let debug_source = TempDir::new()?;
    let tools_source = TempDir::new()?;
    let debug_old_revision = init_marketplace_repo(debug_source.path(), "debug", "debug old")?;
    let tools_old_revision = init_marketplace_repo(tools_source.path(), "tools", "tools old")?;
    commit_marketplace_marker(debug_source.path(), "debug new")?;
    commit_marketplace_marker(tools_source.path(), "tools new")?;
    record_git_marketplace(
        codex_home.path(),
        "debug",
        debug_source.path(),
        &debug_old_revision,
        /*ref_name*/ None,
    )?;
    let tools_source_alias = "manual:tools";
    record_user_marketplace(
        codex_home.path(),
        "tools",
        &configured_git_marketplace_update(
            tools_source_alias,
            Some(&tools_old_revision),
            /*ref_name*/ None,
        ),
    )?;
    disable_plugin_startup_tasks(codex_home.path())?;
    let tools_rewrite = format!("url.{}.insteadOf", tools_source.path().display());

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[
            ("GIT_CONFIG_COUNT", Some("1")),
            ("GIT_CONFIG_KEY_0", Some(&tools_rewrite)),
            ("GIT_CONFIG_VALUE_0", Some(tools_source_alias)),
        ])
        .without_auto_env()
        .build_initialized()
        .await?;

    let tools_root = expected_installed_root(codex_home.path(), "tools")?;
    let response = send_marketplace_upgrade(&mut mcp, Some("tools")).await?;

    assert_eq!(
        response,
        MarketplaceUpgradeResponse {
            selected_marketplaces: vec!["tools".to_string()],
            upgraded_roots: vec![tools_root.clone()],
            errors: Vec::new(),
        }
    );
    assert_eq!(
        std::fs::read_to_string(tools_root.as_path().join("marker.txt"))?,
        "tools new"
    );
    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_returns_empty_roots_when_already_up_to_date() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let old_revision = init_marketplace_repo(source.path(), "debug", "debug old")?;
    commit_marketplace_marker(source.path(), "debug new")?;
    record_git_marketplace(
        codex_home.path(),
        "debug",
        source.path(),
        &old_revision,
        /*ref_name*/ None,
    )?;
    disable_plugin_startup_tasks(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let first_response = send_marketplace_upgrade(&mut mcp, Some("debug")).await?;
    assert!(first_response.errors.is_empty());

    let response = send_marketplace_upgrade(&mut mcp, Some("debug")).await?;

    assert_eq!(
        response,
        MarketplaceUpgradeResponse {
            selected_marketplaces: vec!["debug".to_string()],
            upgraded_roots: Vec::new(),
            errors: Vec::new(),
        }
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_rejects_unknown_or_non_git_marketplace() -> Result<()> {
    let codex_home = TempDir::new()?;
    let local_source = TempDir::new()?;
    record_user_marketplace(
        codex_home.path(),
        "local-only",
        &configured_local_marketplace_update(&local_source.path().display().to_string()),
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    for marketplace_name in ["missing", "local-only"] {
        let request_id = mcp
            .send_marketplace_upgrade_request(MarketplaceUpgradeParams {
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
