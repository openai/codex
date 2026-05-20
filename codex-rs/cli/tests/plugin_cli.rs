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
    std::fs::create_dir_all(source.join(".agents").join("plugins"))?;
    std::fs::create_dir_all(source.join("plugins").join("sample").join(".codex-plugin"))?;
    std::fs::write(
        source
            .join(".agents")
            .join("plugins")
            .join("marketplace.json"),
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
        source
            .join("plugins")
            .join("sample")
            .join(".codex-plugin")
            .join("plugin.json"),
        r#"{"name":"sample","version":"1.2.3","description":"Sample plugin"}"#,
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
    std::fs::create_dir_all(source.path().join(".agents").join("plugins"))?;
    std::fs::write(
        source
            .path()
            .join(".agents")
            .join("plugins")
            .join("marketplace.json"),
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

fn setup_local_marketplace_with_implicit_system_roots() -> Result<(TempDir, TempDir, TempDir)> {
    let (codex_home, source) = setup_local_marketplace()?;

    let bundled_root = codex_home
        .path()
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    std::fs::create_dir_all(&bundled_root)?;
    let bundled_source = bundled_root.display().to_string();
    record_user_marketplace(
        codex_home.path(),
        "openai-bundled",
        &configured_local_marketplace(&bundled_source),
    )?;

    let cache_home = TempDir::new()?;
    let runtime_root = cache_home
        .path()
        .join("codex-runtimes")
        .join("codex-primary-runtime")
        .join("plugins")
        .join("openai-primary-runtime");
    std::fs::create_dir_all(&runtime_root)?;
    let runtime_source = runtime_root.display().to_string();
    record_user_marketplace(
        codex_home.path(),
        "openai-primary-runtime",
        &configured_local_marketplace(&runtime_source),
    )?;

    Ok((codex_home, source, cache_home))
}

fn setup_custom_marketplace_under_implicit_system_root() -> Result<(TempDir, std::path::PathBuf)> {
    let codex_home = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;

    let custom_root = codex_home
        .path()
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("custom-marketplace");
    std::fs::create_dir_all(&custom_root)?;
    let custom_source = custom_root.display().to_string();
    record_user_marketplace(
        codex_home.path(),
        "custom-marketplace",
        &configured_local_marketplace(&custom_source),
    )?;

    Ok((codex_home, custom_root))
}

fn remove_installed_plugin_config(codex_home: &Path, plugin_key: &str) -> Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let plugin_header = format!("[plugins.\"{plugin_key}\"]");
    let config = std::fs::read_to_string(&config_path)?;
    let mut rewritten = Vec::new();
    let mut skipping = false;

    for line in config.lines() {
        if line == plugin_header {
            skipping = true;
            continue;
        }
        if skipping && line.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            rewritten.push(line);
        }
    }

    std::fs::write(config_path, format!("{}\n", rewritten.join("\n")))?;
    Ok(())
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
async fn plugin_list_prints_plugins_in_a_table() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let marketplace_manifest = source
        .path()
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    let plugin_path = source.path().join("plugins").join("sample");

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("Marketplace `debug`"))
        .stdout(contains("PLUGIN"))
        .stdout(contains("STATUS"))
        .stdout(contains("VERSION"))
        .stdout(contains("PATH"))
        .stdout(contains(marketplace_manifest.display().to_string()))
        .stdout(contains("sample@debug"))
        .stdout(contains("not installed"))
        .stdout(contains(plugin_path.display().to_string()));

    Ok(())
}

#[tokio::test]
async fn plugin_list_shows_installed_version_when_plugin_is_installed() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("sample@debug"))
        .stdout(contains("1.2.3"))
        .stdout(contains("installed, enabled"));

    Ok(())
}

#[tokio::test]
async fn plugin_list_excludes_unconfigured_repo_local_marketplaces() -> Result<()> {
    let (codex_home, source) = setup_unconfigured_local_marketplace()?;

    codex_command_in(codex_home.path(), source.path())?
        .args(["plugin", "list", "--marketplace", "debug"])
        .assert()
        .success()
        .stdout(contains("No plugins found in marketplace `debug`."))
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
async fn plugin_list_ignores_implicit_system_marketplace_roots_without_manifests() -> Result<()> {
    let (codex_home, source, cache_home) = setup_local_marketplace_with_implicit_system_roots()?;

    codex_command(codex_home.path())?
        .env("XDG_CACHE_HOME", cache_home.path())
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("Marketplace `debug`"))
        .stdout(contains(
            source
                .path()
                .join(".agents")
                .join("plugins")
                .join("marketplace.json")
                .display()
                .to_string(),
        ))
        .stderr(
            predicates::str::contains("failed to load configured marketplace snapshot(s):").not(),
        );

    Ok(())
}

#[tokio::test]
async fn plugin_list_fails_for_custom_marketplace_under_system_root() -> Result<()> {
    let (codex_home, custom_root) = setup_custom_marketplace_under_implicit_system_root()?;

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .failure()
        .stderr(contains(
            "failed to load configured marketplace snapshot(s):",
        ))
        .stderr(contains("`custom-marketplace`"))
        .stderr(contains(custom_root.display().to_string()))
        .stderr(contains(
            "marketplace root does not contain a supported manifest",
        ));

    Ok(())
}

#[tokio::test]
async fn plugin_list_hides_version_for_cached_but_unconfigured_plugin() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    remove_installed_plugin_config(codex_home.path(), "sample@debug")?;

    codex_command(codex_home.path())?
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(contains("sample@debug"))
        .stdout(contains("not installed"))
        .stdout(predicates::str::contains("1.2.3").not());

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
            .join("plugins/cache/debug/sample/1.2.3/.codex-plugin/plugin.json")
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
            .join("plugins/cache/debug/sample/1.2.3/.codex-plugin/plugin.json")
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
