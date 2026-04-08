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
    assert!(
        installed_root
            .join("plugins/sample/.codex-plugin/plugin.json")
            .is_file()
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

    Ok(())
}
