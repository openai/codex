use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::CONFIG_TOML_FILE;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::canonicalize_existing_preserving_symlinks;
use flate2::Compression;
use flate2::write::GzEncoder;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

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
        .join(".cache")
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
async fn marketplace_list_json_prints_configured_marketplaces() -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let source_path = source.path().display().to_string();

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "marketplaces": [
                {
                    "name": "debug",
                    "root": source_path,
                    "marketplaceSource": {
                        "sourceType": "local",
                        "source": source_path,
                    },
                },
            ],
        })
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_list_json_includes_configured_git_marketplace_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let marketplace_root = codex_home
        .path()
        .join(".tmp")
        .join("marketplaces")
        .join("debug");
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source(&marketplace_root)?;
    let update = MarketplaceConfigUpdate {
        source_type: "git",
        source: "https://example.com/acme/agent-skills.git",
        ref_name: None,
        sparse_paths: &[],
    };
    record_user_marketplace(codex_home.path(), "debug", &update)?;
    let normalized_root = canonicalize_existing_preserving_symlinks(&marketplace_root)?;

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "marketplace", "list", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "marketplaces": [
                {
                    "name": "debug",
                    "root": normalized_root.display().to_string(),
                    "marketplaceSource": {
                        "sourceType": "git",
                        "source": "https://example.com/acme/agent-skills.git",
                    },
                },
            ],
        })
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_list_json_keys_configured_source_by_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    let marketplace_root = codex_home
        .path()
        .join(".tmp")
        .join("marketplaces")
        .join("debug");
    write_plugins_enabled_config(codex_home.path())?;
    write_marketplace_source(home.path())?;
    write_marketplace_source(&marketplace_root)?;
    let update = MarketplaceConfigUpdate {
        source_type: "git",
        source: "https://example.com/acme/agent-skills.git",
        ref_name: None,
        sparse_paths: &[],
    };
    record_user_marketplace(codex_home.path(), "debug", &update)?;
    let normalized_root = canonicalize_existing_preserving_symlinks(&marketplace_root)?;

    let assert = codex_command(codex_home.path())?
        .env("HOME", home.path())
        .args(["plugin", "marketplace", "list", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "marketplaces": [
                {
                    "name": "debug",
                    "root": home.path().display().to_string(),
                },
                {
                    "name": "debug",
                    "root": normalized_root.display().to_string(),
                    "marketplaceSource": {
                        "sourceType": "git",
                        "source": "https://example.com/acme/agent-skills.git",
                    },
                },
            ],
        })
    );

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
        .stdout(contains("SOURCE"))
        .stdout(contains(marketplace_manifest.display().to_string()))
        .stdout(contains("sample@debug"))
        .stdout(contains("not installed"))
        .stdout(contains(plugin_path.display().to_string()));

    Ok(())
}

#[tokio::test]
async fn plugin_list_succeeds_when_stdout_closes_early() -> Result<()> {
    assert_plugin_list_succeeds_when_stdout_closes_early(&[]).await
}

#[tokio::test]
async fn plugin_list_json_succeeds_when_stdout_closes_early() -> Result<()> {
    assert_plugin_list_succeeds_when_stdout_closes_early(&["--json", "--available"]).await
}

async fn assert_plugin_list_succeeds_when_stdout_closes_early(args: &[&str]) -> Result<()> {
    let (codex_home, source) = setup_local_marketplace()?;
    let plugins = (0..32_768)
        .map(|index| {
            json!({
                "name": format!("sample-{index:05}"),
                "source": {"source": "npm", "package": "sample"},
            })
        })
        .collect::<Vec<_>>();
    write_marketplace_source_with_manifest(
        source.path(),
        &json!({"name": "debug", "plugins": plugins}).to_string(),
    )?;

    // The producer must not fit its entire output in the pipe before we close it.
    let complete = codex_command(codex_home.path())?
        .args(["plugin", "list", "--marketplace", "debug"])
        .args(args)
        .assert()
        .success();
    assert!(complete.get_output().stdout.len() > 1024 * 1024);

    let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .env("HOME", codex_home.path())
        .current_dir(codex_home.path())
        .args(["plugin", "list", "--marketplace", "debug"])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().context("piped stdout")?;
    let mut prefix = [0; 1];
    stdout.read_exact(&mut prefix).await?;
    drop(stdout);

    let output = child.wait_with_output().await?;
    assert!(
        output.status.success(),
        "plugin list {args:?} exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(!String::from_utf8(output.stderr)?.contains("panicked"));
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
        .env("HOME", cache_home.path())
        .env("USERPROFILE", cache_home.path())
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
async fn plugin_add_json_prints_install_outcome() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    let assert = codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug", "--json"])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;
    let installed_path = codex_home.path().join("plugins/cache/debug/sample/1.2.3");
    let normalized_installed_path = canonicalize_existing_preserving_symlinks(&installed_path)?;

    assert_eq!(
        actual,
        json!({
            "pluginId": "sample@debug",
            "name": "sample",
            "marketplaceName": "debug",
            "version": "1.2.3",
            "installedPath": normalized_installed_path.display().to_string(),
            "authPolicy": "ON_INSTALL",
        })
    );

    Ok(())
}

#[tokio::test]
async fn plugin_remove_json_prints_remove_outcome() -> Result<()> {
    let (codex_home, _source) = setup_local_marketplace()?;

    codex_command(codex_home.path())?
        .args(["plugin", "add", "sample@debug"])
        .assert()
        .success();

    let assert = codex_command(codex_home.path())?
        .args([
            "plugin",
            "remove",
            "sample",
            "--marketplace",
            "debug",
            "--json",
        ])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;

    assert_eq!(
        actual,
        json!({
            "pluginId": "sample@debug",
            "name": "sample",
            "marketplaceName": "debug",
        })
    );

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

const REMOTE_ID: &str = "b1234567-89ab-4cde-8f01-234567890abc";
const MARKETPLACE: &str = "openai-curated-remote";
const PLUGIN_KEY: &str = "sample@openai-curated-remote";

fn sample_remote_plugin_bundle() -> Result<Vec<u8>> {
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (path, contents) in [
        (
            ".codex-plugin/plugin.json",
            r#"{"name":"sample","version":"1.2.3"}"#,
        ),
        (
            "skills/sample/SKILL.md",
            "---\nname: sample\ndescription: Sample remote skill\n---\nSample instructions.\n",
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(/*mode*/ 0o644);
        header.set_cksum();
        archive.append_data(&mut header, path, contents.as_bytes())?;
    }
    Ok(archive.into_inner()?.finish()?)
}

struct RemoteMarketplaceFixture {
    home: TempDir,
    server: MockServer,
    plugin: Value,
}

impl RemoteMarketplaceFixture {
    async fn new() -> Result<Self> {
        let home = TempDir::new()?;
        let server = MockServer::start().await;
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                "cli_auth_credentials_store = 'file'\nchatgpt_base_url = '{}/backend-api'\n[features]\nplugins = true\nremote_plugin = true\n",
                server.uri()
            ),
        )?;
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new("chatgpt-token")
                .account_id("account-123")
                .chatgpt_account_id("account-123"),
            AuthCredentialsStoreMode::File,
        )?;
        let plugin = json!({
            "id": REMOTE_ID, "name": "sample", "scope": "GLOBAL",
            "installation_policy": "AVAILABLE", "authentication_policy": "ON_USE",
            "release": {"version": "1.2.3", "display_name": "Sample", "description": "Remote sample",
                "interface": {}, "bundle_download_url": format!("{}/bundle.tar.gz", server.uri())}
        });
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/list"))
            .and(query_param("scope", "GLOBAL"))
            .and(header("authorization", "Bearer chatgpt-token"))
            .and(header("chatgpt-account-id", "account-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"plugins": [plugin.clone()], "pagination": {"next_page_token": null}}),
            ))
            .mount(&server)
            .await;
        Ok(Self {
            home,
            server,
            plugin,
        })
    }

    fn write_local_curated_marketplace(&self) -> Result<Value> {
        let source = canonicalize_existing_preserving_symlinks(self.home.path())?
            .join(".tmp")
            .join("plugins");
        let mut manifest = json!({
            "name": "openai-curated",
            "plugins": [{
                "name": "sample",
                "source": {"source": "local", "path": "./plugins/sample"}
            }]
        });
        write_marketplace_source_with_manifest(&source, &manifest.to_string())?;
        std::fs::write(
            self.home.path().join(".tmp").join("plugins.sha"),
            "local-curated",
        )?;
        manifest["name"] = json!("openai-api-curated");
        std::fs::write(
            source
                .join(".agents")
                .join("plugins")
                .join("api_marketplace.json"),
            manifest.to_string(),
        )?;
        Ok(json!({
            "pluginId": "sample@openai-curated", "name": "sample",
            "marketplaceName": "openai-curated", "version": "1.2.3",
            "installed": false, "enabled": false,
            "source": {"source": "local", "path": source.join("plugins").join("sample")},
            "installPolicy": "AVAILABLE", "authPolicy": "ON_INSTALL"
        }))
    }

    async fn run(&self, args: &[&str]) -> Result<Output> {
        Ok(Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .current_dir(self.home.path())
            .env("CODEX_HOME", self.home.path())
            .env("HOME", self.home.path())
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .env("CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS", "1")
            .args(args)
            .output()
            .await?)
    }

    async fn success(&self, args: &[&str]) -> Result<String> {
        let output = self.run(args).await?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?)
    }
}

#[tokio::test]
async fn remote_plugin_listing_replaces_local_curated_catalog() -> Result<()> {
    let fixture = RemoteMarketplaceFixture::new().await?;
    let mut local_plugin = fixture.write_local_curated_marketplace()?;
    fixture
        .success(&["plugin", "add", "sample@openai-curated"])
        .await?;
    local_plugin["installed"] = json!(true);
    local_plugin["enabled"] = json!(true);
    local_plugin["version"] = json!("local-curated");
    let mut installed_plugin = fixture.plugin.clone();
    installed_plugin["enabled"] = json!(true);
    Mock::given(path("/backend-api/ps/plugins/installed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [installed_plugin], "pagination": {"next_page_token": null}
        })))
        .mount(&fixture.server)
        .await;

    let listed = fixture
        .success(&["plugin", "list", "--available", "--json"])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [{
            "pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE,
            "version": "1.2.3", "installed": true, "enabled": true,
            "source": {"source": "remote", "id": REMOTE_ID},
            "installPolicy": "AVAILABLE", "authPolicy": "ON_USE"
        }], "available": []})
    );
    let table = fixture.success(&["plugin", "list"]).await?;
    insta::assert_snapshot!(table, @r"
    Marketplace `openai-curated-remote`
    Remote catalog

    PLUGIN                        STATUS              VERSION  SOURCE
    sample@openai-curated-remote  installed, enabled  1.2.3    b1234567-89ab-4cde-8f01-234567890abc
    ");

    let config_path = fixture.home.path().join(CONFIG_TOML_FILE);
    let mut config = std::fs::read_to_string(&config_path)?;
    config.push_str(&format!("\n[plugins.\"{PLUGIN_KEY}\"]\nenabled = false\n"));
    std::fs::write(config_path, config)?;
    let mut expected: Value = serde_json::from_str(&listed)?;
    expected["installed"][0]["enabled"] = json!(false);
    let disabled = fixture
        .success(&["plugin", "list", "--available", "--json"])
        .await?;
    assert_eq!(serde_json::from_str::<Value>(&disabled)?, expected);
    let table = fixture.success(&["plugin", "list"]).await?;
    assert!(table.contains("installed, disabled"), "{table}");

    fixture.server.reset().await;
    let listed = fixture
        .success(&["plugin", "list", "-m", "openai-curated", "--json"])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [local_plugin], "available": []})
    );
    assert!(fixture.server.received_requests().await.unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_plugin_listing_preserves_local_curated_only_when_fetch_fails() -> Result<()> {
    for status in [200, 403] {
        let fixture = RemoteMarketplaceFixture::new().await?;
        fixture.server.reset().await;
        let local_plugin = fixture.write_local_curated_marketplace()?;
        let response = ResponseTemplate::new(status).set_body_json(json!({
            "plugins": [], "pagination": {"next_page_token": null}
        }));
        let _catalog = Mock::given(path("/backend-api/ps/plugins/list"))
            .respond_with(response)
            .mount_as_scoped(&fixture.server)
            .await;
        Mock::given(path("/backend-api/ps/plugins/installed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [], "pagination": {"next_page_token": null}
            })))
            .mount(&fixture.server)
            .await;

        let listed = fixture
            .success(&["plugin", "list", "--available", "--json"])
            .await?;
        let available = if status == 200 {
            Vec::new()
        } else {
            vec![local_plugin]
        };
        assert_eq!(
            serde_json::from_str::<Value>(&listed)?,
            json!({"installed": [], "available": available}),
            "catalog response status: {status}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn remote_plugin_add_list_and_remove() -> Result<()> {
    let fixture = RemoteMarketplaceFixture::new().await?;
    let installed = Arc::new(AtomicBool::new(false));
    let installed_for_list = Arc::clone(&installed);
    let mut installed_plugin = fixture.plugin.clone();
    installed_plugin["enabled"] = json!(true);
    Mock::given(path("/backend-api/ps/plugins/installed"))
        .respond_with(move |_: &wiremock::Request| {
            let plugins = if installed_for_list.load(Ordering::SeqCst) {
                vec![installed_plugin.clone()]
            } else {
                Vec::new()
            };
            ResponseTemplate::new(200)
                .set_body_json(json!({"plugins": plugins, "pagination": {"next_page_token": null}}))
        })
        .mount(&fixture.server)
        .await;
    Mock::given(path(format!("/backend-api/ps/plugins/{REMOTE_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fixture.plugin))
        .mount(&fixture.server)
        .await;
    Mock::given(path("/bundle.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(sample_remote_plugin_bundle()?))
        .expect(1)
        .mount(&fixture.server)
        .await;
    let installed_root = canonicalize_existing_preserving_symlinks(fixture.home.path())?
        .join("plugins")
        .join("cache")
        .join(MARKETPLACE)
        .join("sample")
        .join("1.2.3");
    let manifest = installed_root.join(".codex-plugin/plugin.json");
    let installed_for_add = Arc::clone(&installed);
    Mock::given(method("POST"))
        .and(path(format!("/backend-api/ps/plugins/{REMOTE_ID}/install")))
        .and(query_param("includeAppsNeedingAuth", "true"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .respond_with(move |_: &wiremock::Request| {
            assert!(
                manifest.is_file(),
                "cache must exist before backend install"
            );
            installed_for_add.store(true, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(json!({"id": REMOTE_ID, "enabled": true}))
        })
        .expect(1)
        .mount(&fixture.server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/backend-api/ps/plugins/{REMOTE_ID}/uninstall"
        )))
        .and(header("authorization", "Bearer chatgpt-token"))
        .respond_with(move |_: &wiremock::Request| {
            installed.store(false, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(json!({"id": REMOTE_ID, "enabled": false}))
        })
        .expect(1)
        .mount(&fixture.server)
        .await;

    let available = json!({
        "pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE,
        "version": "1.2.3", "installed": false, "enabled": false,
        "source": {"source": "remote", "id": REMOTE_ID}, "installPolicy": "AVAILABLE", "authPolicy": "ON_USE"
    });
    let listed = fixture
        .success(&["plugin", "list", "--available", "--json"])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [], "available": [available.clone()]})
    );
    let table = fixture
        .success(&["plugin", "list", "-m", MARKETPLACE])
        .await?;
    insta::assert_snapshot!(table, @r"
    Marketplace `openai-curated-remote`
    Remote catalog

    PLUGIN                        STATUS         VERSION  SOURCE
    sample@openai-curated-remote  not installed  1.2.3    b1234567-89ab-4cde-8f01-234567890abc
    ");
    let added = fixture
        .success(&["plugin", "add", PLUGIN_KEY, "--json"])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&added)?,
        json!({
            "pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE,
            "version": "1.2.3", "installedPath": installed_root, "authPolicy": "ON_USE"
        })
    );
    assert_eq!(
        fixture
            .server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path() == "/backend-api/ps/plugins/list")
            .count(),
        1,
        "installing a cached name must not refetch the catalog"
    );
    assert!(installed_root.join("skills/sample/SKILL.md").is_file());
    let mut expected = available;
    expected["installed"] = json!(true);
    expected["enabled"] = json!(true);
    let listed = fixture
        .success(&["plugin", "list", "-m", MARKETPLACE, "--json"])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [expected], "available": []})
    );
    let _delisted = Mock::given(path("/backend-api/ps/plugins/list"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"plugins": [], "pagination": {"next_page_token": null}})),
        )
        .mount_as_scoped(&fixture.server)
        .await;
    let removed = fixture
        .success(&[
            "-c",
            "features.remote_plugin=false",
            "plugin",
            "remove",
            "sample",
            "-m",
            MARKETPLACE,
            "--json",
        ])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&removed)?,
        json!({"pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE})
    );
    assert!(!installed_root.exists());
    let listed = fixture.success(&["plugin", "list", "--json"]).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [], "available": []})
    );
    Ok(())
}

#[tokio::test]
async fn remote_plugin_add_refreshes_cached_catalog_on_name_miss() -> Result<()> {
    for (remote_plugin_config, collection) in [
        ("features.remote_plugin=true", None),
        ("features.remote_plugin=false", Some("vertical")),
    ] {
        let fixture = RemoteMarketplaceFixture::new().await?;
        fixture.server.reset().await;
        let published = Arc::new(AtomicBool::new(false));
        let published_for_list = Arc::clone(&published);
        let plugin = fixture.plugin.clone();
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/list"))
            .and(query_param("scope", "GLOBAL"))
            .respond_with(move |_: &wiremock::Request| {
                let plugins = if published_for_list.load(Ordering::SeqCst) {
                    vec![plugin.clone()]
                } else {
                    Vec::new()
                };
                ResponseTemplate::new(200).set_body_json(json!({
                    "plugins": plugins, "pagination": {"next_page_token": null}
                }))
            })
            .expect(2)
            .mount(&fixture.server)
            .await;
        Mock::given(path("/backend-api/ps/plugins/installed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [], "pagination": {"next_page_token": null}
            })))
            .mount(&fixture.server)
            .await;
        Mock::given(path(format!("/backend-api/ps/plugins/{REMOTE_ID}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(&fixture.plugin))
            .expect(1)
            .mount(&fixture.server)
            .await;
        Mock::given(path("/bundle.tar.gz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(sample_remote_plugin_bundle()?))
            .expect(1)
            .mount(&fixture.server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/backend-api/ps/plugins/{REMOTE_ID}/install")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"id": REMOTE_ID, "enabled": true})),
            )
            .expect(1)
            .mount(&fixture.server)
            .await;

        for is_published in [false, true] {
            published.store(is_published, Ordering::SeqCst);
            let listed = fixture
                .success(&[
                    "-c",
                    remote_plugin_config,
                    "plugin",
                    "list",
                    "-m",
                    MARKETPLACE,
                    "--available",
                    "--json",
                ])
                .await?;
            assert_eq!(
                serde_json::from_str::<Value>(&listed)?,
                json!({"installed": [], "available": []}),
                "normal listing must keep using the fresh catalog cache"
            );
        }
        let added = fixture
            .success(&[
                "-c",
                remote_plugin_config,
                "plugin",
                "add",
                PLUGIN_KEY,
                "--json",
            ])
            .await?;
        let installed_root = canonicalize_existing_preserving_symlinks(fixture.home.path())?
            .join("plugins")
            .join("cache")
            .join(MARKETPLACE)
            .join("sample")
            .join("1.2.3");
        assert_eq!(
            serde_json::from_str::<Value>(&added)?,
            json!({
                "pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE,
                "version": "1.2.3", "installedPath": installed_root, "authPolicy": "ON_USE"
            })
        );
        assert!(
            installed_root
                .join("skills")
                .join("sample")
                .join("SKILL.md")
                .is_file()
        );
        let requests = fixture.server.received_requests().await.unwrap();
        let catalog_collections = requests
            .iter()
            .filter(|request| request.url.path() == "/backend-api/ps/plugins/list")
            .map(|request| {
                request
                    .url
                    .query_pairs()
                    .find_map(|(key, value)| (key == "collection").then(|| value.into_owned()))
            })
            .collect::<Vec<_>>();
        assert_eq!(catalog_collections, vec![collection.map(str::to_owned); 2]);
    }
    Ok(())
}

#[tokio::test]
async fn remote_plugin_add_limits_catalog_refresh_without_mutation() -> Result<()> {
    enum CatalogCache {
        Missing,
        Fresh,
        Expired,
    }

    for (remote_plugin_config, collection) in [
        ("features.remote_plugin=true", None),
        ("features.remote_plugin=false", Some("vertical")),
    ] {
        for (cache, plugin_count, status, catalog_fetches, error) in [
            (
                CatalogCache::Missing,
                0,
                200,
                1,
                "was not found in remote marketplace",
            ),
            (
                CatalogCache::Fresh,
                0,
                200,
                2,
                "was not found in remote marketplace",
            ),
            (
                CatalogCache::Expired,
                0,
                200,
                2,
                "was not found in remote marketplace",
            ),
            (
                CatalogCache::Missing,
                2,
                200,
                1,
                "matched multiple remote plugins",
            ),
            (
                CatalogCache::Missing,
                0,
                403,
                1,
                "failed to list remote marketplace plugins",
            ),
        ] {
            let fixture = RemoteMarketplaceFixture::new().await?;
            fixture.server.reset().await;
            let mut duplicate = fixture.plugin.clone();
            duplicate["id"] = json!("c1234567-89ab-4cde-8f01-234567890abc");
            let plugins = [fixture.plugin.clone(), duplicate]
                .into_iter()
                .take(plugin_count)
                .collect::<Vec<_>>();
            Mock::given(path("/backend-api/ps/plugins/list"))
                .and(query_param("scope", "GLOBAL"))
                .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                    "plugins": plugins, "pagination": {"next_page_token": null}
                })))
                .expect(catalog_fetches)
                .mount(&fixture.server)
                .await;
            Mock::given(path("/backend-api/ps/plugins/installed"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "plugins": [], "pagination": {"next_page_token": null}
                })))
                .mount(&fixture.server)
                .await;
            if !matches!(cache, CatalogCache::Missing) {
                let listed = fixture
                    .success(&[
                        "-c",
                        remote_plugin_config,
                        "plugin",
                        "list",
                        "-m",
                        MARKETPLACE,
                        "--available",
                        "--json",
                    ])
                    .await?;
                assert_eq!(
                    serde_json::from_str::<Value>(&listed)?,
                    json!({"installed": [], "available": []})
                );
            }
            if matches!(cache, CatalogCache::Expired) {
                let cache_paths = std::fs::read_dir(
                    fixture
                        .home
                        .path()
                        .join("cache")
                        .join("remote_plugin_catalog"),
                )?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
                assert_eq!(cache_paths.len(), 1);
                let cache_path = &cache_paths[0];
                let mut cached_catalog: Value =
                    serde_json::from_slice(&std::fs::read(cache_path)?)?;
                cached_catalog["fetched_at"] = json!("2000-01-01T00:00:00Z");
                std::fs::write(cache_path, serde_json::to_vec(&cached_catalog)?)?;
            }
            let output = fixture
                .run(&["-c", remote_plugin_config, "plugin", "add", PLUGIN_KEY])
                .await?;
            assert!(!output.status.success());
            let stderr = String::from_utf8(output.stderr)?;
            assert!(stderr.contains(error), "{stderr}");
            let requests = fixture.server.received_requests().await.unwrap();
            let catalog_collections = requests
                .iter()
                .filter(|request| request.url.path() == "/backend-api/ps/plugins/list")
                .map(|request| {
                    request
                        .url
                        .query_pairs()
                        .find_map(|(key, value)| (key == "collection").then(|| value.into_owned()))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                catalog_collections,
                vec![collection.map(str::to_owned); catalog_fetches as usize]
            );
            assert!(requests.iter().all(|request| {
                request.method == "GET"
                    && matches!(
                        request.url.path(),
                        "/backend-api/ps/plugins/list" | "/backend-api/ps/plugins/installed"
                    )
            }));
            assert!(
                !fixture
                    .home
                    .path()
                    .join("plugins")
                    .join("cache")
                    .join(MARKETPLACE)
                    .exists()
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn remote_plugin_install_checks_policy_and_bundle_before_mutation() -> Result<()> {
    let fixture = RemoteMarketplaceFixture::new().await?;
    Mock::given(path("/backend-api/ps/plugins/installed"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"plugins": [], "pagination": {"next_page_token": null}})),
        )
        .mount(&fixture.server)
        .await;
    for (field, value, error) in [
        ("status", "DISABLED_BY_ADMIN", "disabled by admin"),
        (
            "installation_policy",
            "NOT_AVAILABLE",
            "not available for install",
        ),
        ("status", "ENABLED", "failed to read plugin bundle tar"),
    ] {
        let mut plugin = fixture.plugin.clone();
        plugin[field] = json!(value);
        let _detail = Mock::given(path(format!("/backend-api/ps/plugins/{REMOTE_ID}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(plugin))
            .mount_as_scoped(&fixture.server)
            .await;
        let _bundle = Mock::given(path("/bundle.tar.gz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bad gzip"))
            .mount_as_scoped(&fixture.server)
            .await;
        let output = fixture
            .run(&["plugin", "add", "sample", "-m", MARKETPLACE])
            .await?;
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains(error), "{stderr}");
    }
    let requests = fixture.server.received_requests().await.unwrap();
    assert!(requests.iter().all(|request| request.method != "POST"));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/backend-api/ps/plugins/list")
            .count(),
        1,
        "installation policy and bundle failures must not refetch the catalog"
    );
    Ok(())
}

#[tokio::test]
async fn remote_plugin_listing_uses_collection_when_remote_catalog_is_disabled() -> Result<()> {
    let fixture = RemoteMarketplaceFixture::new().await?;
    let mut local_plugin = fixture.write_local_curated_marketplace()?;
    Mock::given(path("/backend-api/ps/plugins/installed"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"plugins": [], "pagination": {"next_page_token": null}})),
        )
        .mount(&fixture.server)
        .await;
    let listed = fixture
        .success(&[
            "-c",
            "features.remote_plugin=false",
            "plugin",
            "list",
            "--available",
            "--json",
        ])
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [], "available": [local_plugin.clone(), {
            "pluginId": PLUGIN_KEY, "name": "sample", "marketplaceName": MARKETPLACE,
            "version": "1.2.3", "installed": false, "enabled": false,
            "source": {"source": "remote", "id": REMOTE_ID},
            "installPolicy": "AVAILABLE", "authPolicy": "ON_USE"
        }]})
    );
    let requests = fixture.server.received_requests().await.unwrap();
    let listing = requests
        .iter()
        .find(|request| request.url.path().ends_with("/list"))
        .unwrap();
    assert!(
        listing
            .url
            .query_pairs()
            .any(|(key, value)| key == "collection" && value == "vertical")
    );
    fixture.server.reset().await;
    fixture
        .success(&["-c", "features.plugins=false", "plugin", "list"])
        .await?;
    fixture
        .success(&["plugin", "list", "-m", "local-only"])
        .await?;
    assert!(fixture.server.received_requests().await.unwrap().is_empty());
    std::fs::remove_file(fixture.home.path().join("auth.json"))?;
    let output = fixture.run(&["plugin", "list", "-m", MARKETPLACE]).await?;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("chatgpt authentication required"));
    let listed = fixture
        .success(&["plugin", "list", "--available", "--json"])
        .await?;
    local_plugin["pluginId"] = json!("sample@openai-api-curated");
    local_plugin["marketplaceName"] = json!("openai-api-curated");
    assert_eq!(
        serde_json::from_str::<Value>(&listed)?,
        json!({"installed": [], "available": [local_plugin]})
    );
    Ok(())
}
