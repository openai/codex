use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_utils_absolute_path::canonicalize_existing_preserving_symlinks;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;

const MARKETPLACE_HEADER: &str = "MARKETPLACE";
const MARKETPLACE_LIST_HEADER: &str = "MARKETPLACE  ROOT";

fn marketplace_list_row(marketplace_name: &str, root: &Path) -> String {
    format!(
        "{marketplace_name:<width$}  {}",
        root.display(),
        width = MARKETPLACE_HEADER.len()
    )
}

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("HOME", codex_home);
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

fn write_marketplace_source_with_manifest(source: &Path, marketplace_manifest: &str) -> Result<()> {
    std::fs::create_dir_all(source.join(".agents").join("plugins"))?;
    std::fs::create_dir_all(source.join("plugins").join("sample").join(".codex-plugin"))?;
    std::fs::write(
        source
            .join(".agents")
            .join("plugins")
            .join("marketplace.json"),
        marketplace_manifest,
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

fn write_marketplace_source(source: &Path) -> Result<()> {
    write_marketplace_source_with_manifest(
        source,
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
    )
}

fn write_marketplace_source_with_explicit_empty_products(source: &Path) -> Result<()> {
    write_marketplace_source_with_manifest(
        source,
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      },
      "policy": {
        "products": []
      }
    }
  ]
}"#,
    )
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

fn setup_local_marketplace_with_explicit_empty_products() -> Result<(TempDir, TempDir)> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source_with_explicit_empty_products(source.path())?;
    let source_path = source.path().to_string_lossy().into_owned();
    record_user_marketplace(
        codex_home.path(),
        "debug",
        &configured_local_marketplace(&source_path),
    )?;
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

fn setup_configured_local_marketplace_with_missing_source() -> Result<TempDir> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[marketplaces.debug]
source_type = "local"
"#,
    )?;
    Ok(codex_home)
}

fn setup_configured_local_marketplace_with_invalid_name() -> Result<TempDir> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[marketplaces."bad/name"]
source_type = "local"
source = "/tmp/debug"
"#,
    )?;
    Ok(codex_home)
}

fn assert_configured_marketplace_snapshot_failure(
    assert: assert_cmd::assert::Assert,
    source: &Path,
    detail: &str,
) {
    assert
        .failure()
        .stderr(contains(
            "failed to load configured marketplace snapshot(s):",
        ))
        .stderr(contains("`debug`"))
        .stderr(contains(source.display().to_string()))
        .stderr(contains(detail));
}

fn assert_marketplace_failure(
    assert: assert_cmd::assert::Assert,
    marketplace_name: &str,
    source: &Path,
    detail: &str,
) {
    assert
        .failure()
        .stderr(contains("failed to load marketplace(s):"))
        .stderr(contains(format!("`{marketplace_name}`")))
        .stderr(contains(source.display().to_string()))
        .stderr(contains(detail));
}

#[tokio::test]
async fn marketplace_list_shows_configured_marketplace_names() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let expected_row = marketplace_list_row("debug", source.path());

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list"])
        .assert()
        .success()
        .stdout(contains(MARKETPLACE_LIST_HEADER))
        .stdout(contains(&expected_row))
        .stdout(contains("\t").not());

    Ok(())
}

#[tokio::test]
async fn marketplace_list_includes_home_marketplace_when_present() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    write_marketplace_source(home.path())?;
    write_plugins_enabled_config(codex_home.path())?;
    let expected_row = marketplace_list_row("debug", home.path());

    codex_command(codex_home.path())?
        .env("HOME", home.path())
        .args(["plugin", "marketplace", "list"])
        .assert()
        .success()
        .stdout(contains(MARKETPLACE_LIST_HEADER))
        .stdout(contains(&expected_row))
        .stdout(contains("\t").not());

    Ok(())
}

#[tokio::test]
async fn marketplace_list_includes_root_when_plugins_are_filtered_out() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace_with_explicit_empty_products()?;
    let expected_row = marketplace_list_row("debug", source.path());

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list"])
        .assert()
        .success()
        .stdout(contains(MARKETPLACE_LIST_HEADER))
        .stdout(contains(&expected_row));

    Ok(())
}

#[tokio::test]
async fn marketplace_list_fails_when_configured_marketplace_snapshot_is_missing() -> Result<()> {
    let (codex_home, source) = setup_configured_marketplace_without_manifest()?;

    assert_marketplace_failure(
        codex_command(codex_home.path())?
            .args(["plugin", "marketplace", "list"])
            .assert(),
        "debug",
        source.path(),
        "marketplace root does not contain a supported manifest",
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_list_fails_when_configured_marketplace_name_is_invalid() -> Result<()> {
    let codex_home = setup_configured_local_marketplace_with_invalid_name()?;

    assert_marketplace_failure(
        codex_command(codex_home.path())?
            .args(["plugin", "marketplace", "list"])
            .assert(),
        "bad/name",
        Path::new("<invalid config>"),
        "marketplace name",
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_list_fails_when_configured_local_marketplace_source_is_missing() -> Result<()>
{
    let codex_home = setup_configured_local_marketplace_with_missing_source()?;

    codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list"])
        .assert()
        .failure()
        .stderr(contains("failed to load marketplace(s):"))
        .stderr(contains("`debug`"))
        .stderr(contains("<invalid source>"))
        .stderr(contains(
            "configured local marketplace source is missing or empty",
        ));

    Ok(())
}

#[tokio::test]
async fn marketplace_list_fails_when_home_marketplace_is_malformed() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    write_plugins_enabled_config(codex_home.path())?;
    std::fs::create_dir_all(home.path().join(".agents/plugins"))?;
    let home_marketplace_path = home
        .path()
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    std::fs::write(&home_marketplace_path, "{not valid json")?;

    codex_command(codex_home.path())?
        .env("HOME", home.path())
        .args(["plugin", "marketplace", "list"])
        .assert()
        .failure()
        .stderr(contains("failed to load marketplace(s):"))
        .stderr(contains(home_marketplace_path.display().to_string()))
        .stderr(contains("key must be a string"));

    Ok(())
}

#[tokio::test]
async fn marketplace_list_fails_when_configured_marketplace_snapshot_is_malformed() -> Result<()> {
    let (codex_home, source) = setup_configured_marketplace_with_malformed_manifest()?;

    assert_marketplace_failure(
        codex_command(codex_home.path())?
            .args(["plugin", "marketplace", "list"])
            .assert(),
        "debug",
        source.path(),
        "key must be a string",
    );

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
async fn plugin_list_json_prints_available_plugins_when_requested() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let plugin_path = source.path().join("plugins").join("sample");
    let source_path = source.path().to_string_lossy().into_owned();

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "list", "--available", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "installed": [],
            "available": [
                {
                    "pluginId": "sample@debug",
                    "name": "sample",
                    "marketplaceName": "debug",
                    "version": "1.2.3",
                    "installed": false,
                    "enabled": false,
                    "source": {
                        "source": "local",
                        "path": plugin_path.display().to_string(),
                    },
                    "marketplaceSource": {
                        "sourceType": "local",
                        "source": source_path,
                    },
                    "installPolicy": "AVAILABLE",
                    "authPolicy": "ON_INSTALL",
                },
            ],
        })
    );

    Ok(())
}

#[tokio::test]
async fn plugin_list_json_includes_configured_git_marketplace_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let marketplace_root = codex_home
        .path()
        .join(".tmp")
        .join("marketplaces")
        .join("debug");
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source(&marketplace_root)?;
    let update = MarketplaceConfigUpdate {
        last_updated: "2026-06-04T08:39:49Z",
        last_revision: Some("abc123"),
        source_type: "git",
        source: "https://example.com/acme/agent-skills.git",
        ref_name: None,
        sparse_paths: &[],
    };
    record_user_marketplace(codex_home.path(), "debug", &update)?;
    let plugin_path = marketplace_root.join("plugins").join("sample");
    let normalized_plugin_path = canonicalize_existing_preserving_symlinks(&plugin_path)?;

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "list", "--available", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "installed": [],
            "available": [
                {
                    "pluginId": "sample@debug",
                    "name": "sample",
                    "marketplaceName": "debug",
                    "version": "1.2.3",
                    "installed": false,
                    "enabled": false,
                    "source": {
                        "source": "local",
                        "path": normalized_plugin_path.display().to_string(),
                    },
                    "marketplaceSource": {
                        "sourceType": "git",
                        "source": "https://example.com/acme/agent-skills.git",
                    },
                    "installPolicy": "AVAILABLE",
                    "authPolicy": "ON_INSTALL",
                },
            ],
        })
    );

    Ok(())
}

#[tokio::test]
async fn plugin_list_json_prints_installed_plugins() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let plugin_path = source.path().join("plugins").join("sample");
    let source_path = source.path().to_string_lossy().into_owned();

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "list", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "installed": [
                {
                    "pluginId": "sample@debug",
                    "name": "sample",
                    "marketplaceName": "debug",
                    "version": "1.2.3",
                    "installed": true,
                    "enabled": true,
                    "source": {
                        "source": "local",
                        "path": plugin_path.display().to_string(),
                    },
                    "marketplaceSource": {
                        "sourceType": "local",
                        "source": source_path,
                    },
                    "installPolicy": "AVAILABLE",
                    "authPolicy": "ON_INSTALL",
                },
            ],
            "available": [],
        })
    );

    Ok(())
}

#[tokio::test]
async fn plugin_list_available_requires_json() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "list", "--available"])
        .assert()
        .failure()
        .stderr(contains(
            "the following required arguments were not provided",
        ))
        .stderr(contains("--json"));

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

    assert_configured_marketplace_snapshot_failure(
        codex_command(codex_home.path())?
            .args(["plugin", "list"])
            .assert(),
        source.path(),
        "marketplace root does not contain a supported manifest",
    );

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
    let (codex_home, source) = setup_configured_marketplace_with_malformed_manifest()?;

    assert_configured_marketplace_snapshot_failure(
        codex_command(codex_home.path())?
            .args(["plugin", "add", "sample@debug"])
            .assert(),
        source.path(),
        "key must be a string",
    );

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
