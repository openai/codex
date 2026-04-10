use anyhow::Context;
use anyhow::Result;
use codex_core::plugins::marketplace_install_root;
use codex_core::plugins::validate_marketplace_root;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_marketplace_source(source: &Path, marker: &str) -> Result<()> {
    write_marketplace_source_with_name(source, "debug", marker)
}

fn write_marketplace_source_with_name(
    source: &Path,
    marketplace_name: &str,
    marker: &str,
) -> Result<()> {
    std::fs::create_dir_all(source.join(".agents/plugins"))?;
    std::fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
    std::fs::write(
        source.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "sample",
      "source": {{
        "source": "local",
        "path": "./plugins/sample"
      }}
    }}
  ]
}}"#
        ),
    )?;
    std::fs::write(
        source.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )?;
    std::fs::write(source.join("plugins/sample/marker.txt"), marker)?;
    Ok(())
}

#[tokio::test]
async fn marketplace_add_local_directory_installs_valid_marketplace_root() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "first install")?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["marketplace", "add", source.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(validate_marketplace_root(&installed_root)?, "debug");
    assert_marketplace_config(codex_home.path(), "debug", &source.path().canonicalize()?)?;
    assert!(
        installed_root
            .join("plugins/sample/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(!installed_root.join(".codex-marketplace-source").exists());
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_invalid_marketplace_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source_with_name(source.path(), "debug.market", "invalid marketplace")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", source.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(
            "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed",
        ));

    assert!(
        !marketplace_install_root(codex_home.path())
            .join("debug.market")
            .exists()
    );
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_same_source_is_idempotent() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "first install")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", source.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    std::fs::write(
        source.path().join("plugins/sample/marker.txt"),
        "source changed after add",
    )?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", source.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Marketplace `debug` is already added"));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(
        std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt"))?,
        "first install"
    );
    assert_marketplace_config(codex_home.path(), "debug", &source.path().canonicalize()?)?;
    assert!(!installed_root.join(".codex-marketplace-source").exists());
    assert!(
        !codex_home
            .path()
            .join(".tmp/known_marketplaces.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_same_name_from_different_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let first_source = TempDir::new()?;
    let second_source = TempDir::new()?;
    write_marketplace_source(first_source.path(), "first install")?;
    write_marketplace_source(second_source.path(), "replacement install")?;

    codex_command(codex_home.path())?
        .args(["marketplace", "add", first_source.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Added marketplace `debug`"));

    codex_command(codex_home.path())?
        .args(["marketplace", "add", second_source.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(
            "marketplace `debug` is already added from a different source",
        ));

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert_eq!(
        std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt"))?,
        "first install"
    );

    Ok(())
}

fn assert_marketplace_config(
    codex_home: &Path,
    marketplace_name: &str,
    source: &Path,
) -> Result<()> {
    let config = std::fs::read_to_string(codex_home.join("config.toml"))?;
    let config: toml::Value = toml::from_str(&config)?;
    let marketplace = config
        .get("marketplaces")
        .and_then(|marketplaces| marketplaces.get(marketplace_name))
        .context("marketplace config should be written")?;
    let expected_source = source.to_string_lossy().to_string();

    assert!(
        marketplace
            .get("last_updated")
            .and_then(toml::Value::as_str)
            .is_some_and(|last_updated| {
                last_updated.len() == "2026-04-10T12:34:56Z".len() && last_updated.ends_with('Z')
            }),
        "last_updated should be an RFC3339-like UTC timestamp"
    );
    assert_eq!(
        marketplace.get("source_type").and_then(toml::Value::as_str),
        Some("directory")
    );
    assert_eq!(
        marketplace.get("source").and_then(toml::Value::as_str),
        Some(expected_source.as_str())
    );
    assert_eq!(marketplace.get("ref").and_then(toml::Value::as_str), None);
    assert!(marketplace.get("sparse_paths").is_none());
    assert!(marketplace.get("source_id").is_none());
    assert!(marketplace.get("install_root").is_none());
    assert!(marketplace.get("install_location").is_none());

    Ok(())
}

#[tokio::test]
async fn marketplace_add_sparse_flag_parses_before_and_after_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    let source = source.path().to_str().unwrap();
    let sparse_requires_git = "--sparse can only be used with git marketplace sources";

    codex_command(codex_home.path())?
        .args(["marketplace", "add", "--sparse", "plugins/foo", source])
        .assert()
        .failure()
        .stderr(contains(sparse_requires_git));

    codex_command(codex_home.path())?
        .args(["marketplace", "add", source, "--sparse", "plugins/foo"])
        .assert()
        .failure()
        .stderr(contains(sparse_requires_git));

    codex_command(codex_home.path())?
        .args([
            "marketplace",
            "add",
            "--sparse",
            "plugins/foo",
            "--sparse",
            "skills/bar",
            source,
        ])
        .assert()
        .failure()
        .stderr(contains(sparse_requires_git));

    Ok(())
}
