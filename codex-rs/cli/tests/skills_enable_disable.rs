use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::CONFIG_TOML_FILE;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use toml::Value as TomlValue;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn parse_skills_config_entries(contents: &str) -> Result<Vec<toml::value::Table>> {
    let value: TomlValue = toml::from_str(contents).context("config should parse as TOML")?;
    let skills = value
        .get("skills")
        .and_then(TomlValue::as_table)
        .context("expected skills table")?;
    let config = skills
        .get("config")
        .and_then(TomlValue::as_array)
        .context("expected skills.config array")?;
    config
        .iter()
        .map(|entry| {
            entry
                .as_table()
                .cloned()
                .context("skills.config entries must be tables")
        })
        .collect::<Result<Vec<_>>>()
}

fn toml_basic_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[test]
fn disable_writes_skill_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let skill_dir = TempDir::new()?;
    let skill_path = skill_dir.path().join("SKILL.md");
    fs::write(&skill_path, "# Skill\n")?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args([
        "skills",
        "disable",
        skill_path.to_str().context("skill path must be UTF-8")?,
    ])
    .assert()
    .success()
    .stdout(contains(format!(
        "Disabled skill at {path}.",
        path = skill_path.display()
    )));

    let contents = fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    let canonical_path = fs::canonicalize(&skill_path).unwrap_or_else(|_| skill_path.clone());
    let entries = parse_skills_config_entries(&contents)?;
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    let path = entry
        .get("path")
        .and_then(TomlValue::as_str)
        .context("skills.config.path should be a string")?;
    let enabled = entry
        .get("enabled")
        .and_then(TomlValue::as_bool)
        .context("skills.config.enabled should be a bool")?;
    assert!(!enabled);
    let normalized =
        fs::canonicalize(Path::new(path)).unwrap_or_else(|_| Path::new(path).to_path_buf());
    assert_eq!(normalized, canonical_path);

    Ok(())
}

#[test]
fn enable_removes_skill_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let skill_dir = TempDir::new()?;
    let skill_path = skill_dir.path().join("SKILL.md");
    fs::write(&skill_path, "# Skill\n")?;

    let canonical_path = fs::canonicalize(&skill_path).unwrap_or_else(|_| skill_path.clone());
    let disabled_entry = format!(
        "[[skills.config]]\npath = {path}\nenabled = false\n",
        path = toml_basic_string(&canonical_path.to_string_lossy())
    );
    fs::write(codex_home.path().join(CONFIG_TOML_FILE), disabled_entry)?;

    let mut cmd = codex_command(codex_home.path())?;
    cmd.args([
        "skills",
        "enable",
        skill_path.to_str().context("skill path must be UTF-8")?,
    ])
    .assert()
    .success()
    .stdout(contains(format!(
        "Enabled skill at {path}.",
        path = skill_path.display()
    )));

    let contents = fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
    assert_eq!(contents.trim(), "");

    Ok(())
}
