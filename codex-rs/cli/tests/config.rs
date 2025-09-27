use std::fs;
use std::path::Path;

use anyhow::Result;
use assert_cmd::Command;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<Command> {
    let mut cmd = Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn config_subcommand_reports_success_for_valid_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join("config.toml");
    fs::write(config_path, "model = \"gpt-5\"\n")?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.arg("config").output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Current default config settings"));

    Ok(())
}

#[test]
fn config_subcommand_exits_with_code_three_on_validation_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join("config.toml");
    fs::write(config_path, "model = 123\n")?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.arg("config").output()?;

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("Config validation error"));

    Ok(())
}
