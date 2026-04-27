use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

const FAKE_AGENT_IDENTITY_JWT: &str = "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJhZ2VudF9ydW50aW1lX2lkIjoiYWdlbnQtcnVudGltZS1pZCIsImFnZW50X3ByaXZhdGVfa2V5IjoicHJpdmF0ZS1rZXkiLCJhY2NvdW50X2lkIjoiYWNjb3VudC0xMjMiLCJjaGF0Z3B0X3VzZXJfaWQiOiJ1c2VyLWlkIiwiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIiwicGxhbl90eXBlIjoicHJvIiwiY2hhdGdwdF9hY2NvdW50X2lzX2ZlZHJhbXAiOmZhbHNlfQ.c2ln";

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn write_file_auth_config(codex_home: &Path) -> Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        "cli_auth_credentials_store = \"file\"\n",
    )?;
    Ok(())
}

fn read_auth_json(codex_home: &Path) -> Result<Value> {
    let auth_json = std::fs::read_to_string(codex_home.join("auth.json"))?;
    Ok(serde_json::from_str(&auth_json)?)
}

#[test]
fn login_with_api_key_reads_stdin_and_writes_auth_json() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_file_auth_config(codex_home.path())?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args([
        "-c",
        "forced_login_method=\"api\"",
        "login",
        "--with-api-key",
    ])
    .write_stdin("sk-test\n")
    .assert()
    .success()
    .stderr(contains("Successfully logged in"));

    let auth = read_auth_json(codex_home.path())?;
    assert_eq!(auth["OPENAI_API_KEY"], "sk-test");
    assert!(auth.get("tokens").is_none());
    assert!(auth.get("agent_identity").is_none());

    Ok(())
}

#[test]
fn login_with_agent_identity_reads_stdin_and_writes_auth_json() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_file_auth_config(codex_home.path())?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["login", "--with-agent-identity"])
        .write_stdin(format!("{FAKE_AGENT_IDENTITY_JWT}\n"))
        .assert()
        .success()
        .stderr(contains("Successfully logged in"));

    let auth = read_auth_json(codex_home.path())?;
    assert_eq!(auth["auth_mode"], "agentIdentity");
    assert_eq!(auth["agent_identity"], FAKE_AGENT_IDENTITY_JWT);
    assert!(auth["OPENAI_API_KEY"].is_null());
    assert!(auth.get("tokens").is_none());

    Ok(())
}
