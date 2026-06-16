use std::path::Path;

use anyhow::Result;
use codex_rmcp_client::resolve_mcp_oauth_callback_url;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn callback_url_prints_configured_final_url() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"mcp_oauth_callback_url = "https://callbacks.example.com/oauth/callback"

[mcp_servers.github]
url = "https://example.com/mcp"
"#,
    )?;

    let expected = resolve_mcp_oauth_callback_url(
        "https://example.com/mcp",
        /*callback_port*/ None,
        Some("https://callbacks.example.com/oauth/callback"),
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["mcp", "callback-url", "github"]).output()?;

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, format!("{expected}\n"));
    assert_eq!(String::from_utf8(output.stderr)?, "");
    assert!(!codex_home.path().join(".credentials.json").exists());

    Ok(())
}

#[test]
fn callback_url_errors_when_default_port_is_ephemeral() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[mcp_servers.github]
url = "https://example.com/mcp"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["mcp", "callback-url", "github"]).output()?;

    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout)?, "");
    assert!(
        String::from_utf8(output.stderr)?
            .contains("set `mcp_oauth_callback_port` or `mcp_oauth_callback_url`")
    );
    assert!(!codex_home.path().join(".credentials.json").exists());

    Ok(())
}
