//! Exercise migration consent, permissions, and accurate partial/no-op reporting.

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
    let mut empty = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    let empty = empty
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(
            contains("No databases found").and(contains("Migration completed successfully").not()),
        );
    insta::assert_snapshot!(
        String::from_utf8_lossy(&empty.get_output().stdout)
            .replace(home.path().to_str().unwrap(), "[HOME]"),
        @"No databases found."
    );
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

    // A later corrupt database must be reported without undoing a completed conversion.
    std::fs::write(sqlite.logs_db_path(), b"corrupt SQLite database")?;
    let mut partial = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    let partial = partial
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("y\n")
        .assert()
        .failure()
        .stdout(
            contains("Migrated database").and(contains("Migration completed successfully").not()),
        )
        .stderr(contains("migration failed for"));
    let stderr = String::from_utf8_lossy(&partial.get_output().stderr);
    insta::assert_snapshot!(
        stderr
            .lines()
            .find(|line| line.starts_with("Error: migration failed"))
            .unwrap()
            .replace(home.path().to_str().unwrap(), "[HOME]"),
        @"Error: migration failed for [HOME]/logs_2.sqlite. Keep all Codex instances stopped. Correct the error and rerun codex migrate-nfs-db"
    );
    assert_eq!(
        std::fs::read(sqlite.logs_db_path())?,
        b"corrupt SQLite database"
    );
    std::fs::remove_file(sqlite.logs_db_path())?;

    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", home.path())
        .env(codex_state::SQLITE_HOME_ENV, home.path())
        .arg("migrate-nfs-db")
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(
            contains("Already in DELETE mode").and(contains("Migration completed successfully")),
        );
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
