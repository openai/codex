use std::path::Path;

use anyhow::Result;
use codex_core::config::load_project_mcp_servers;
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

    // Verify the project config via helper loader
    let servers = load_project_mcp_servers(project_dir.path())?;
    let docs = servers.get("docs").expect("server should exist");
    match &docs.transport {
        codex_core::config_types::McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            assert_eq!(command, "echo");
            assert_eq!(args, &vec!["hello".to_string()]);
            assert!(env.is_none());
            assert!(env_vars.is_empty());
            assert!(cwd.is_none());
        }
        other => panic!("unexpected transport: {other:?}"),
    }
    assert!(docs.enabled);

    // Remove project-scoped server
    let mut remove_cmd = codex_command(codex_home.path())?;
    remove_cmd
        .current_dir(project_dir.path())
        .args(["mcp", "remove", "docs", "--project"])
        .assert()
        .success()
        .stdout(contains("Removed project MCP server 'docs'"));

    let servers_after = load_project_mcp_servers(project_dir.path())?;
    assert!(!servers_after.contains_key("docs"));

    Ok(())
}
