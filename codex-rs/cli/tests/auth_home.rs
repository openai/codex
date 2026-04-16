use std::path::Path;

use anyhow::Result;
use predicates::str::contains;
use tempfile::TempDir;

fn codex_command(codex_home: &Path, auth_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    cmd.env("CODEX_AUTH_HOME", auth_home);
    Ok(cmd)
}

#[tokio::test]
async fn login_uses_codex_auth_home_without_writing_codex_home_auth() -> Result<()> {
    let codex_home = TempDir::new()?;
    let auth_root = TempDir::new()?;
    let auth_home = auth_root.path().join("auth-home");

    let mut login = codex_command(codex_home.path(), &auth_home)?;
    login
        .args(["login", "--with-api-key"])
        .write_stdin("sk-proj-1234567890ABCDE\n")
        .assert()
        .success()
        .stderr(contains("Successfully logged in"));

    assert!(!codex_home.path().join("auth.json").exists());
    assert!(auth_home.join("auth.json").exists());

    let mut status = codex_command(codex_home.path(), &auth_home)?;
    status
        .args(["login", "status"])
        .assert()
        .success()
        .stderr(contains("Logged in using an API key - sk-proj-***ABCDE"));

    Ok(())
}
