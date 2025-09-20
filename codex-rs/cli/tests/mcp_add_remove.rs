use std::path::Path;

use anyhow::Result;
use codex_core::config::load_global_mcp_servers;
use codex_core::config_types::McpTransport;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn add_and_remove_server_updates_global_config() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args(["mcp", "add", "docs", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(contains("Added global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path())?;
    assert_eq!(servers.len(), 1);
    let docs = servers.get("docs").expect("server should exist");
    assert_eq!(docs.transport, McpTransport::Stdio);
    assert_eq!(docs.command.as_deref(), Some("echo"));
    assert_eq!(docs.args, vec!["hello".to_string()]);
    assert!(docs.env.is_none());
    assert!(docs.url.is_none());
    assert!(docs.headers.is_none());

    let mut remove_cmd = codex_command(codex_home.path())?;
    remove_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("Removed global MCP server 'docs'."));

    let servers = load_global_mcp_servers(codex_home.path())?;
    assert!(servers.is_empty());

    let mut remove_again_cmd = codex_command(codex_home.path())?;
    remove_again_cmd
        .args(["mcp", "remove", "docs"])
        .assert()
        .success()
        .stdout(contains("No MCP server named 'docs' found."));

    let servers = load_global_mcp_servers(codex_home.path())?;
    assert!(servers.is_empty());

    Ok(())
}

#[test]
fn add_with_env_preserves_key_order_and_values() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "envy",
            "--env",
            "FOO=bar",
            "--env",
            "ALPHA=beta",
            "--",
            "python",
            "server.py",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path())?;
    let envy = servers.get("envy").expect("server should exist");
    let env = envy.env.as_ref().expect("env should be present");

    assert_eq!(env.len(), 2);
    assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    assert_eq!(env.get("ALPHA"), Some(&"beta".to_string()));

    Ok(())
}

#[test]
fn add_sse_server_records_transport_and_urls() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "sse-server",
            "--transport",
            "sse",
            "--url",
            "http://localhost:3000/sse",
            "--messages-url",
            "http://localhost:3000/messages",
            "--header",
            "Authorization=Bearer 123",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path())?;
    let cfg = servers.get("sse-server").expect("server should exist");

    assert_eq!(cfg.transport, McpTransport::Sse);
    assert_eq!(cfg.url.as_deref(), Some("http://localhost:3000/sse"));
    assert_eq!(
        cfg.messages_url.as_deref(),
        Some("http://localhost:3000/messages")
    );
    let headers = cfg.headers.as_ref().expect("headers should be present");
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer 123".to_string())
    );
    assert!(cfg.command.is_none());

    Ok(())
}

#[test]
fn add_http_server_records_headers() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut add_cmd = codex_command(codex_home.path())?;
    add_cmd
        .args([
            "mcp",
            "add",
            "http-server",
            "--transport",
            "http",
            "--url",
            "https://example.com/events",
            "--messages-url",
            "https://example.com/messages",
            "--header",
            "X-Api-Key=abc",
            "--header",
            "User-Agent=codex-test",
        ])
        .assert()
        .success();

    let servers = load_global_mcp_servers(codex_home.path())?;
    let cfg = servers.get("http-server").expect("server should exist");

    assert_eq!(cfg.transport, McpTransport::Http);
    assert_eq!(cfg.url.as_deref(), Some("https://example.com/events"));
    assert_eq!(
        cfg.messages_url.as_deref(),
        Some("https://example.com/messages")
    );
    let headers = cfg.headers.as_ref().expect("headers should be present");
    assert_eq!(headers.get("X-Api-Key"), Some(&"abc".to_string()));
    assert_eq!(headers.get("User-Agent"), Some(&"codex-test".to_string()));
    assert!(cfg.command.is_none());

    Ok(())
}
