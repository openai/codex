use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn add_and_remove_project_scoped_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    let project_dir = TempDir::new()?;

    // Add project-scoped server (writes into <project>/.codex/config.toml)
    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .current_dir(project_dir.path())
        .args(["mcp", "add", "docs", "--project", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(contains("Added project MCP server 'docs'"));

    // Verify the project file contents
    let project_toml =
        std::fs::read_to_string(project_dir.path().join(".codex").join("config.toml"))?;
    let parsed: toml::Value = toml::from_str(&project_toml)?;
    let mcp = parsed
        .get("mcp_servers")
        .and_then(|v| v.as_table())
        .expect("mcp_servers table");
    let docs = mcp
        .get("docs")
        .and_then(|v| v.as_table())
        .expect("docs entry");
    assert_eq!(docs.get("command").and_then(|v| v.as_str()), Some("echo"));
    let args = docs
        .get("args")
        .and_then(|v| v.as_array())
        .expect("args array");
    assert_eq!(
        args.iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>(),
        vec!["hello"]
    );

    // Remove project-scoped server
    let mut remove_cmd = codex_command(codex_home.path())?;
    remove_cmd
        .current_dir(project_dir.path())
        .args(["mcp", "remove", "docs", "--project"])
        .assert()
        .success()
        .stdout(contains("Removed project MCP server 'docs'"));

    let project_toml =
        std::fs::read_to_string(project_dir.path().join(".codex").join("config.toml"))?;
    let parsed_after: toml::Value = toml::from_str(&project_toml)?;
    let mcp_after = parsed_after
        .get("mcp_servers")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    assert!(!mcp_after.contains_key("docs"));

    Ok(())
}
