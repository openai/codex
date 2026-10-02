//! Exercises uninstall argument handling without performing machine-wide cleanup.

use anyhow::Result;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn uninstall_help_and_invalid_arguments_preserve_user_data() -> Result<()> {
    let home = TempDir::new()?;
    // Uninstall must work independently of configuration parsing and authentication.
    std::fs::write(home.path().join("config.toml"), "invalid config [")?;
    std::fs::write(home.path().join("auth.json"), "preserve credentials")?;

    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", home.path())
        .args(["sandbox", "uninstall", "--help"])
        .assert()
        .success();

    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", home.path())
        .args(["sandbox", "uninstall", "unexpected"])
        .assert()
        .failure()
        .stderr(contains("unexpected argument"));

    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "invalid config ["
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("auth.json"))?,
        "preserve credentials"
    );
    Ok(())
}
