use super::*;
use crate::runtime::test_support::unique_temp_dir;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn backup_moves_only_requested_runtime_db_files_to_backup_folder() -> std::io::Result<()> {
    let sqlite_home = unique_temp_dir();
    tokio::fs::create_dir_all(sqlite_home.as_path()).await?;
    let sqlite = crate::SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let runtime_paths = sqlite.runtime_db_paths();
    let mut expected_paths = Vec::new();
    for db_path in runtime_paths.iter().map(|db| db.path.as_path()) {
        for path in sqlite_paths(db_path) {
            tokio::fs::write(path.as_path(), path.display().to_string()).await?;
            expected_paths.push(path);
        }
    }
    let failed_db_path = sqlite.logs_db_path();
    let failed_paths = sqlite_paths(failed_db_path.as_path());

    let backups = backup_runtime_db_for_fresh_start(failed_db_path.as_path()).await?;

    assert_eq!(backups.len(), failed_paths.len());
    for path in &failed_paths {
        assert!(!tokio::fs::try_exists(path.as_path()).await?);
    }
    for path in expected_paths
        .iter()
        .filter(|path| !failed_paths.contains(path))
    {
        assert!(tokio::fs::try_exists(path.as_path()).await?);
    }
    for backup in backups {
        assert!(
            backup
                .backup_path
                .starts_with(sqlite_home.join(BACKUP_DIR_NAME))
        );
        assert!(tokio::fs::try_exists(backup.backup_path.as_path()).await?);
    }
    Ok(())
}

#[tokio::test]
async fn backup_replaces_blocking_sqlite_home_file() -> std::io::Result<()> {
    let temp_dir = unique_temp_dir();
    tokio::fs::create_dir_all(temp_dir.as_path()).await?;
    let sqlite_home = temp_dir.join("sqlite-home");
    tokio::fs::write(sqlite_home.as_path(), b"not-a-directory").await?;

    let backups = backup_runtime_db_for_fresh_start(
        crate::SqliteConfig::new_for_testing(sqlite_home.as_path().abs())
            .state_db_path()
            .as_path(),
    )
    .await?;

    assert_eq!(backups.len(), 1);
    assert!(tokio::fs::metadata(sqlite_home.as_path()).await?.is_dir());
    assert!(
        backups[0]
            .backup_path
            .starts_with(temp_dir.join(format!("sqlite-home.{BACKUP_DIR_NAME}")))
    );
    assert!(tokio::fs::try_exists(backups[0].backup_path.as_path()).await?);
    Ok(())
}

#[test]
fn sqlite_error_detail_classifies_corruption_and_lock_errors() {
    assert!(sqlite_error_detail_is_corruption("file is not a database"));
    assert!(sqlite_error_detail_is_corruption(
        "error returned from database: (code: 11) database disk image is malformed"
    ));
    assert!(!sqlite_error_detail_is_corruption("database is locked"));
    assert!(sqlite_error_detail_is_lock("database is locked"));
    assert!(sqlite_error_detail_is_lock("database is busy"));
}

#[tokio::test]
async fn runtime_db_path_for_corruption_error_returns_failed_database_path() -> std::io::Result<()>
{
    let sqlite_home = unique_temp_dir();
    tokio::fs::create_dir_all(sqlite_home.as_path()).await?;
    let path = crate::SqliteConfig::new_for_testing(sqlite_home.as_path().abs()).state_db_path();
    tokio::fs::write(path.as_path(), b"not sqlite").await?;

    let err = match super::super::StateRuntime::init(
        crate::SqliteConfig::new_for_testing(sqlite_home.as_path().abs()),
        "openai".to_string(),
    )
    .await
    {
        Ok(_) => panic!("malformed sqlite should fail to initialize"),
        Err(err) => err,
    };

    assert_eq!(runtime_db_path_for_corruption_error(&err), Some(path));
    Ok(())
}

#[test]
fn runtime_db_path_for_corruption_error_ignores_corrupt_word_in_path() {
    let path = PathBuf::from("/tmp/sqlite_corrupt/state_5.sqlite");
    let err = anyhow::Error::new(RuntimeDbInitError::new(
        "state DB",
        "open",
        path.as_path(),
        anyhow::anyhow!("permission denied"),
    ));

    assert_eq!(runtime_db_path_for_corruption_error(&err), None);
}
