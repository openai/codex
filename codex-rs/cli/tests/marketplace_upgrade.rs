use anyhow::Result;
use codex_core::plugins::marketplace_install_root;
use predicates::str::contains;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use toml::Value;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_marketplace_source(source: &Path, marketplace_name: &str, marker: &str) -> Result<()> {
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

fn init_git_repo(repo: &Path) {
    git(repo, &["init"]);
    git(repo, &["config", "user.email", "codex-test@example.com"]);
    git(repo, &["config", "user.name", "Codex Test"]);
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "initial marketplace"]);
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git should run: {err}"));
    assert!(
        output.status.success(),
        "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
        repo.display(),
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_marketplaces_config(codex_home: &Path, entries: &[(&str, &Path)]) -> Result<()> {
    let mut root = toml::map::Map::new();
    let mut features = toml::map::Map::new();
    features.insert("plugins".to_string(), Value::Boolean(true));
    root.insert("features".to_string(), Value::Table(features));

    let mut marketplaces = toml::map::Map::new();
    for (name, source) in entries {
        let mut marketplace = toml::map::Map::new();
        marketplace.insert(
            "last_updated".to_string(),
            Value::String("2026-04-10T00:00:00Z".to_string()),
        );
        marketplace.insert(
            "last_revision".to_string(),
            Value::String("old-revision".to_string()),
        );
        marketplace.insert("source_type".to_string(), Value::String("git".to_string()));
        marketplace.insert(
            "source".to_string(),
            Value::String(source.display().to_string()),
        );
        marketplaces.insert((*name).to_string(), Value::Table(marketplace));
    }
    root.insert("marketplaces".to_string(), Value::Table(marketplaces));

    std::fs::write(
        codex_home.join("config.toml"),
        toml::to_string(&Value::Table(root))?,
    )?;
    Ok(())
}

fn installed_marker(codex_home: &Path, marketplace_name: &str) -> String {
    std::fs::read_to_string(
        marketplace_install_root(codex_home)
            .join(marketplace_name)
            .join("plugins/sample/marker.txt"),
    )
    .unwrap_or_else(|err| panic!("installed marker should read: {err}"))
}

#[tokio::test]
async fn marketplace_upgrade_all_upgrades_every_configured_git_marketplace() -> Result<()> {
    let codex_home = TempDir::new()?;
    let alpha_source = TempDir::new()?;
    let beta_source = TempDir::new()?;
    write_marketplace_source(alpha_source.path(), "alpha", "alpha-new")?;
    write_marketplace_source(beta_source.path(), "beta", "beta-new")?;
    init_git_repo(alpha_source.path());
    init_git_repo(beta_source.path());
    write_marketplaces_config(
        codex_home.path(),
        &[("alpha", alpha_source.path()), ("beta", beta_source.path())],
    )?;
    write_marketplace_source(
        &marketplace_install_root(codex_home.path()).join("alpha"),
        "alpha",
        "alpha-old",
    )?;
    write_marketplace_source(
        &marketplace_install_root(codex_home.path()).join("beta"),
        "beta",
        "beta-old",
    )?;

    codex_command(codex_home.path())?
        .args(["marketplace", "upgrade"])
        .assert()
        .success()
        .stdout(contains("Upgraded 2 marketplace(s)."));

    assert_eq!(installed_marker(codex_home.path(), "alpha"), "alpha-new");
    assert_eq!(installed_marker(codex_home.path(), "beta"), "beta-new");
    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_single_marketplace_only_upgrades_requested_marketplace() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let alpha_source = TempDir::new()?;
    let beta_source = TempDir::new()?;
    write_marketplace_source(alpha_source.path(), "alpha", "alpha-new")?;
    write_marketplace_source(beta_source.path(), "beta", "beta-new")?;
    init_git_repo(alpha_source.path());
    init_git_repo(beta_source.path());
    write_marketplaces_config(
        codex_home.path(),
        &[("alpha", alpha_source.path()), ("beta", beta_source.path())],
    )?;
    write_marketplace_source(
        &marketplace_install_root(codex_home.path()).join("alpha"),
        "alpha",
        "alpha-old",
    )?;
    write_marketplace_source(
        &marketplace_install_root(codex_home.path()).join("beta"),
        "beta",
        "beta-old",
    )?;

    codex_command(codex_home.path())?
        .args(["marketplace", "upgrade", "alpha"])
        .assert()
        .success()
        .stdout(contains(
            "Upgraded marketplace `alpha` to the latest configured revision.",
        ));

    assert_eq!(installed_marker(codex_home.path(), "alpha"), "alpha-new");
    assert_eq!(installed_marker(codex_home.path(), "beta"), "beta-old");
    Ok(())
}

#[tokio::test]
async fn marketplace_upgrade_rejects_unknown_marketplace_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    let alpha_source = TempDir::new()?;
    write_marketplace_source(alpha_source.path(), "alpha", "alpha-new")?;
    init_git_repo(alpha_source.path());
    write_marketplaces_config(codex_home.path(), &[("alpha", alpha_source.path())])?;

    codex_command(codex_home.path())?
        .args(["marketplace", "upgrade", "missing"])
        .assert()
        .failure()
        .stderr(contains(
            "marketplace `missing` is not configured as a Git marketplace",
        ));

    Ok(())
}
