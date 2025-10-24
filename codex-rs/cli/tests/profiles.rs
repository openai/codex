use std::fs;
use std::path::Path;

use anyhow::Result;
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<Command> {
    let mut cmd = Command::cargo_bin("codex")?;
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn profiles_list_outputs_sorted_names() -> Result<()> {
    let codex_home = TempDir::new()?;
    fs::write(
        codex_home.path().join("config.toml"),
        r#"
[profiles.zeta]
model = "gpt-5"

[profiles.alpha]
model = "gpt-5"

[profiles.mid]
model = "gpt-5"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["profiles", "list"]).output()?;
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines, vec!["alpha", "mid", "zeta"]);

    Ok(())
}

#[test]
fn profiles_list_respects_invalid_flag() -> Result<()> {
    let codex_home = TempDir::new()?;
    fs::write(
        codex_home.path().join("config.toml"),
        r#"
[profiles.alpha]
model = "gpt-5"
"#,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args(["--profile", "missing", "profiles", "list"])
        .assert()
        .failure()
        .stderr(contains("config profile `missing` not found"));

    Ok(())
}

#[test]
fn fish_completion_uses_profiles_helper() -> Result<()> {
    let output = Command::cargo_bin("codex")?
        .args(["completion", "fish"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("function __fish_codex_profile_list"));
    assert!(stdout.contains("codex profiles list"));
    Ok(())
}

#[test]
fn bash_completion_uses_profiles_helper() -> Result<()> {
    let output = Command::cargo_bin("codex")?
        .args(["completion", "bash"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("__codex_bash_complete_profiles()"));
    assert!(stdout.contains("__codex_bash_complete_profiles \"${cur}\""));
    Ok(())
}

#[test]
fn zsh_completion_uses_profiles_helper() -> Result<()> {
    let output = Command::cargo_bin("codex")?
        .args(["completion", "zsh"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("__codex_zsh_complete_profiles()"));
    assert!(stdout.contains(":CONFIG_PROFILE:__codex_zsh_complete_profiles"));
    Ok(())
}
