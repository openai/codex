use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn codex_command_in(codex_home: &Path, current_dir: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = codex_command(codex_home)?;
    cmd.current_dir(current_dir);
    Ok(cmd)
}

fn configured_local_marketplace(source: &str) -> MarketplaceConfigUpdate<'_> {
    MarketplaceConfigUpdate {
        last_updated: "2026-05-06T00:00:00Z",
        last_revision: None,
        source_type: "local",
        source,
        ref_name: None,
        sparse_paths: &[],
    }
}

fn write_plugins_enabled_config(codex_home: &Path) -> Result<()> {
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    )?;
    Ok(())
}

fn write_marketplace_source(source: &Path) -> Result<()> {
    std::fs::create_dir_all(source.join(".agents/plugins"))?;
    std::fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
    std::fs::write(
        source.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        source.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"Sample plugin"}"#,
    )?;
    Ok(())
}

fn setup_local_marketplace() -> Result<(TempDir, TempDir)> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source(source.path())?;
    let source_path = source.path().to_string_lossy().into_owned();
    record_user_marketplace(
        codex_home.path(),
        "debug",
        &configured_local_marketplace(&source_path),
    )?;
    Ok((codex_home, source))
}

fn setup_unconfigured_local_marketplace() -> Result<(TempDir, TempDir)> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source(source.path())?;
    Ok((codex_home, source))
}

fn setup_configured_marketplace_without_manifest() -> Result<(TempDir, TempDir)> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    let source_path = source.path().to_string_lossy().into_owned();
    record_user_marketplace(
        codex_home.path(),
        "debug",
        &configured_local_marketplace(&source_path),
    )?;
    Ok((codex_home, source))
}

fn setup_configured_marketplace_with_malformed_manifest() -> Result<(TempDir, TempDir)> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    std::fs::create_dir_all(source.path().join(".agents/plugins"))?;
    std::fs::write(
        source.path().join(".agents/plugins/marketplace.json"),
        "{not valid json",
    )?;
    let source_path = source.path().to_string_lossy().into_owned();
    record_user_marketplace(
        codex_home.path(),
        "debug",
        &configured_local_marketplace(&source_path),
    )?;
    Ok((codex_home, source))
}

#[tokio::test]
async fn marketplace_list_shows_configured_marketplace_names() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list"])
        .assert()
        .success()
        .stdout(contains("debug"))
        .stdout(contains(source.path().display().to_string()));

    Ok(())
}

#[tokio::test]
async fn plugin_list_shows_plugins_grouped_by_marketplace() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("Marketplace `debug`"))
        .stdout(contains("sample@debug (not installed)"));

    Ok(())
}

#[tokio::test]
async fn plugin_list_excludes_unconfigured_repo_local_marketplaces() -> Result<()> {
    let (codex_home, source) = setup_unconfigured_local_marketplace()?;

    codex_command_in(codex_home.path(), source.path())?
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("No marketplace plugins found."))
        .stdout(predicates::str::is_match("sample@debug").unwrap().not());

    Ok(())
}

#[tokio::test]
async fn plugin_list_fails_when_configured_marketplace_snapshot_is_missing() -> Result<()> {
    let (codex_home, source) = setup_configured_marketplace_without_manifest()?;

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .failure()
        .stderr(contains(
            "failed to load configured marketplace snapshot(s):",
        ))
        .stderr(contains("`debug`"))
        .stderr(contains(source.path().display().to_string()))
        .stderr(contains(
            "marketplace root does not contain a supported manifest",
        ));

    Ok(())
}

#[tokio::test]
async fn plugin_add_and_remove_updates_installed_plugin_config() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success()
        .stdout(contains("Added plugin `sample` from marketplace `debug`."));

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    assert!(config.contains("[plugins.\"sample@debug\"]"));

    codex_command(codex_home.path())?
        .args(["plugin", "remove", "sample", "--marketplace", "debug"])
        .assert()
        .success()
        .stdout(contains(
            "Removed plugin `sample` from marketplace `debug`.",
        ));

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    assert!(!config.contains("[plugins.\"sample@debug\"]"));

    Ok(())
}

#[tokio::test]
async fn plugin_add_rejects_unconfigured_repo_local_marketplaces() -> Result<()> {
    let (codex_home, source) = setup_unconfigured_local_marketplace()?;

    codex_command_in(codex_home.path(), source.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .failure()
        .stderr(contains(
            "plugin `sample` was not found in marketplace `debug`",
        ));

    Ok(())
}

#[tokio::test]
async fn plugin_add_fails_when_configured_marketplace_snapshot_is_malformed() -> Result<()> {
    let (codex_home, _source) = setup_configured_marketplace_with_malformed_manifest()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .failure()
        .stderr(contains(
            "failed to load configured marketplace snapshot(s):",
        ))
        .stderr(contains("`debug`"))
        .stderr(contains("invalid marketplace file"))
        .stderr(contains("key must be a string"));

    Ok(())
}

#[tokio::test]
async fn plugin_add_reinstalls_from_configured_marketplace_snapshot() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success()
        .stdout(contains("Added plugin `sample` from marketplace `debug`."));

    assert!(
        codex_home
            .path()
            .join("plugins/cache/debug/sample/local/.codex-plugin/plugin.json")
            .is_file()
    );

    Ok(())
}

#[tokio::test]
async fn plugin_remove_works_after_marketplace_is_removed() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample", "--marketplace", "debug"])
        .assert()
        .success();

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug"])
        .assert()
        .success();

    codex_command(codex_home.path())?
        .args(["plugin", "remove", "sample@debug"])
        .assert()
        .success()
        .stdout(contains(
            "Removed plugin `sample` from marketplace `debug`.",
        ));

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    assert!(!config.contains("[plugins.\"sample@debug\"]"));

    Ok(())
}

#[tokio::test]
async fn plugin_add_rejects_cached_plugins_without_authorizing_marketplace_snapshot() -> Result<()>
{
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "remove", "debug"])
        .assert()
        .success();

    assert!(
        codex_home
            .path()
            .join("plugins/cache/debug/sample/local/.codex-plugin/plugin.json")
            .is_file()
    );

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .failure()
        .stderr(contains(
            "plugin `sample` was not found in marketplace `debug`",
        ));

    Ok(())
}
