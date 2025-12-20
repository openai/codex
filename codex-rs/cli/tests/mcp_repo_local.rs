use std::path::Path;

use anyhow::Result;
use codex_core::config::load_global_mcp_servers;
use predicates::str::contains;
use tempfile::TempDir;

fn codex_command(codex_home: &Path, cwd: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    cmd.current_dir(cwd);
    Ok(cmd)
}

#[tokio::test]
async fn mcp_add_writes_to_codex_home_even_in_git_repo() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo = TempDir::new()?;

    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");

    let mut cmd = codex_command(codex_home.path(), repo.path())?;
    cmd.args(["mcp", "add", "docs", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(contains("Added global MCP server 'docs'."));

    let repo_codex_dir = repo.path().join(".codex");
    let repo_servers = load_global_mcp_servers(&repo_codex_dir).await?;
    assert!(
        repo_servers.is_empty(),
        "add should no longer write repo-local config by default"
    );

    // Ensure we wrote to CODEX_HOME.
    let home_servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(home_servers.contains_key("docs"));

    Ok(())
}

#[tokio::test]
async fn mcp_add_with_global_flag_errors_and_writes_nothing() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo = TempDir::new()?;

    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");

    let mut cmd = codex_command(codex_home.path(), repo.path())?;
    cmd.args(["mcp", "add", "-g", "docs", "--", "echo", "hello"])
        .assert()
        .failure()
        .stderr(contains("unexpected argument '-g'"));

    // Ensure no config was written in error case.
    let repo_codex_dir = repo.path().join(".codex");
    let repo_servers = load_global_mcp_servers(&repo_codex_dir).await?;
    assert!(repo_servers.is_empty());

    let home_servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(home_servers.is_empty());

    Ok(())
}
