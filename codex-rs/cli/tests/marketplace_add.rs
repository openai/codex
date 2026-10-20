use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_utils_absolute_path::AbsolutePathBuf;
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
async fn marketplace_add_git_source_cleans_staging_after_failures() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "git source")?;
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Codex Tests",
            "-c",
            "user.email=codex@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "marketplace",
        ],
    ] {
        let output = std::process::Command::new("git")
            .current_dir(source.path())
            .args(args)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Exercise real Git clones without using the network or changing the process environment.
    let source_url = "https://example.com/marketplace.git";
    let rewrite_key = format!("url.{}.insteadOf", source.path().display());
    let install_root = marketplace_install_root(codex_home.path());
    let staging_root = install_root.join(".staging");
    let abandoned = staging_root.join(format!("marketplace-{}-abandoned", std::env::consts::OS));
    std::fs::create_dir_all(abandoned.join("checkout"))?;
    std::fs::write(abandoned.join(".lock"), "")?;
    std::fs::write(abandoned.join("checkout/payload"), "abandoned clone")?;
    for (args, error) in [
        (["--ref", "missing-ref"], "git checkout missing-ref failed"),
        (
            ["--sparse", "plugins/sample"],
            "marketplace root does not contain a supported manifest",
        ),
    ] {
        for _ in 0..2 {
            codex_command(codex_home.path())?
                .current_dir(codex_home.path())
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", &rewrite_key)
                .env("GIT_CONFIG_VALUE_0", source_url)
                .args(["plugin", "marketplace", "add", source_url])
                .args(args)
                .assert()
                .failure()
                .stderr(contains(error));

            assert_eq!(std::fs::read_dir(&staging_root)?.count(), 0);
            assert!(!install_root.join("debug").exists());
            assert!(!codex_home.path().join(CONFIG_TOML_FILE).exists());
        }
    }

    codex_command(codex_home.path())?
        .current_dir(codex_home.path())
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", &rewrite_key)
        .env("GIT_CONFIG_VALUE_0", source_url)
        .args(["plugin", "marketplace", "add", source_url])
        .assert()
        .success();
    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;

    // A different ref bypasses the already-added shortcut and conflicts after cloning.
    codex_command(codex_home.path())?
        .current_dir(codex_home.path())
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", &rewrite_key)
        .env("GIT_CONFIG_VALUE_0", source_url)
        .args(["plugin", "marketplace", "add", source_url, "--ref", "HEAD"])
        .assert()
        .failure()
        .stderr(contains("already added from a different source"));

    assert_eq!(std::fs::read_dir(&staging_root)?.count(), 0);
    assert_eq!(
        std::fs::read_to_string(install_root.join("debug/plugins/sample/marker.txt"))?,
        "git source"
    );
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?,
        config
    );
    Ok(())
}

#[tokio::test]
async fn marketplace_add_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;
    let source_parent = source.path().parent().unwrap();
    let source_arg = format!("./{}", source.path().file_name().unwrap().to_string_lossy());

    codex_command(codex_home.path())?
        .current_dir(source_parent)
        .args(["plugin", "marketplace", "add", source_arg.as_str()])
        .assert()
        .success();

    let installed_root = marketplace_install_root(codex_home.path()).join("debug");
    assert!(!installed_root.exists());

    let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    let config: toml::Value = toml::from_str(&config)?;
    let expected_source = source.path().canonicalize()?.display().to_string();
    assert_eq!(
        config["marketplaces"]["debug"]["source_type"].as_str(),
        Some("local")
    );
    assert_eq!(
        config["marketplaces"]["debug"]["source"].as_str(),
        Some(expected_source.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_json_prints_add_outcome() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;
    let source_parent = source.path().parent().unwrap();
    let source_arg = format!("./{}", source.path().file_name().unwrap().to_string_lossy());

    let assert = codex_command(codex_home.path())?
        .current_dir(source_parent)
        .args([
            "plugin",
            "marketplace",
            "add",
            source_arg.as_str(),
            "--json",
        ])
        .assert()
        .success();
    let stdout = assert.get_output().stdout.as_slice();
    let actual: serde_json::Value = serde_json::from_slice(stdout)?;
    let expected_installed_root = AbsolutePathBuf::try_from(source.path().canonicalize()?)?;

    assert_eq!(
        actual,
        json!({
            "marketplaceName": "debug",
            "installedRoot": expected_installed_root.as_path().display().to_string(),
            "alreadyAdded": false,
        })
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_local_manifest_file_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;
    let manifest_path = source.path().join(".agents/plugins/marketplace.json");

    codex_command(codex_home.path())?
        .args([
            "plugin",
            "marketplace",
            "add",
            manifest_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains(
            "local marketplace source must be a directory, not a file",
        ));

    Ok(())
}

#[tokio::test]
async fn marketplace_add_rejects_sparse_for_local_directory_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let source = TempDir::new()?;
    write_marketplace_source(source.path(), "local ref")?;

    codex_command(codex_home.path())?
        .args([
            "plugin",
            "marketplace",
            "add",
            "--sparse",
            ".agents",
            source.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains(
            "--sparse is only supported for git marketplace sources",
        ));

    Ok(())
}
