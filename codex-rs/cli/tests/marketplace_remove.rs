use anyhow::Result;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_utils_absolute_path::canonicalize_existing_preserving_symlinks;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

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

fn write_installed_marketplace(codex_home: &Path, marketplace_name: &str) -> Result<()> {
    let root = marketplace_install_root(codex_home).join(marketplace_name);
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    std::fs::write(root.join(".agents/plugins/marketplace.json"), "{}")?;
    std::fs::write(root.join("marker.txt"), "installed")?;
    Ok(())
}

#[tokio::test]
async fn marketplace_remove_deletes_config_and_installed_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    record_user_marketplace(codex_home.path(), "debug", &configured_marketplace_update())?;
    write_installed_marketplace(codex_home.path(), "debug")?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug"])
        .assert()
        .success()
        .stdout(contains("Removed marketplace `debug`."));

    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(config_path)?;
    assert!(!config.contains("[marketplaces.debug]"));
    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_remove_json_prints_remove_outcome() -> Result<()> {
    let codex_home = TempDir::new()?;
    record_user_marketplace(codex_home.path(), "debug", &configured_marketplace_update())?;
    write_installed_marketplace(codex_home.path(), "debug")?;
    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    let normalized_installed_root = canonicalize_existing_preserving_symlinks(&installed_root)?;

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "marketplaceName": "debug",
            "installedRoot": normalized_installed_root.display().to_string(),
        })
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_remove_rejects_unknown_marketplace() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug"])
        .assert()
        .failure()
        .stderr(contains(
            "marketplace `debug` is not configured or installed",
        ));

    Ok(())
}
