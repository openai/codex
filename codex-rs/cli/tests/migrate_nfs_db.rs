//! Exercise consent and file permissions through the migration CLI.

#![cfg(unix)]

use anyhow::Result;
use codex_state::SqliteConfig;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn migration_requires_consent_and_preserves_shared_database_mode() -> Result<()> {
    let home = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(home.path().to_path_buf().try_into()?);
    let path = sqlite.state_db_path();
    let pool = sqlite.open_read_write_pool(&path).await?;
    sqlx::raw_sql("CREATE TABLE entries (id INTEGER); INSERT INTO entries VALUES (42)")
        .execute(&pool)
        .await?;
    pool.close().await;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(/*mode*/ 0o600))?;
    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    let declined = command
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("n\n")
        .assert()
        .failure()
        .stderr(contains("permissions other than 0600").not())
        .stderr(contains("Continue? [y/N]"));
    let stderr = String::from_utf8_lossy(&declined.get_output().stderr);
    let disclosure = stderr
        .find("the original owner, group, and extended ACLs are not copied")
        .unwrap();
    let consent = stderr.find("Continue? [y/N]").unwrap();
    assert!(
        disclosure < consent,
        "ACL loss must be disclosed before consent, including for 0600 files"
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(/*mode*/ 0o644))?;
    let before = std::fs::read(&path)?;

    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("n\n")
        .assert()
        .failure()
        .stderr(contains("permissions other than 0600"))
        .stderr(contains("No databases were changed"));
    assert_eq!(std::fs::read(&path)?, before);

    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(contains("Migration completed successfully"));
    let pool = sqlite
        .open_read_only_pool(&path, /*busy_timeout*/ None)
        .await?;
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await?;
    let rows: Vec<i64> = sqlx::query_scalar("SELECT id FROM entries")
        .fetch_all(&pool)
        .await?;
    assert_eq!(
        (
            mode,
            rows,
            std::fs::metadata(&path)?.permissions().mode() & 0o777
        ),
        ("delete".to_string(), vec![42], 0o644)
    );
    pool.close().await;
    Ok(())
}
