//! CLI handling for local state database startup failures.
//!
//! This keeps user-facing backup and lock-contention handling out of the main
//! CLI dispatch path while preserving the TUI startup error as the boundary type.

use codex_state::RuntimeDbBackup;
use codex_tui::LocalStateDbStartupError;
use std::io::IsTerminal;
use std::path::Path;

pub(crate) fn startup_error(err: &std::io::Error) -> Option<&LocalStateDbStartupError> {
    err.get_ref()
        .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
}

pub(crate) fn is_locked(detail: &str) -> bool {
    codex_state::sqlite_error_detail_is_lock(detail)
}

pub(crate) fn is_corruption(detail: &str) -> bool {
    codex_state::sqlite_error_detail_is_corruption(detail)
}

pub(crate) fn print_auto_backup_start(startup_error: &LocalStateDbStartupError) {
    eprintln!("Codex couldn't start because its local database appears to be damaged.");
    eprintln!("Moving the damaged local database aside so Codex can rebuild it from saved data.");
    print_technical_details(startup_error);
}

pub(crate) async fn backup_files_for_fresh_start(
    startup_error: &LocalStateDbStartupError,
) -> std::io::Result<Vec<RuntimeDbBackup>> {
    let state_db_path = startup_error.state_db_path();
    let sqlite_home = state_db_path.parent().ok_or_else(|| {
        std::io::Error::other("state database path does not have a parent directory")
    })?;
    codex_state::backup_runtime_dbs_for_fresh_start(sqlite_home).await
}

pub(crate) fn confirm_fresh_start_rebuild(
    startup_error: &LocalStateDbStartupError,
    backups: &[RuntimeDbBackup],
) -> std::io::Result<()> {
    eprintln!("Codex rebuilt its local database.");
    eprintln!(
        "Codex detected a damaged local database, moved it into a backup folder, and will continue startup with a fresh database."
    );
    eprintln!("Database path: {}", startup_error.state_db_path().display());
    if let Some(backup_folder) = backup_folder(backups) {
        eprintln!("Backup folder: {}", backup_folder.display());
    } else {
        eprintln!("Backup folder: unavailable");
    }

    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        eprintln!("Press Enter to continue.");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    } else {
        eprintln!("Continuing startup with a fresh local database...");
    }
    Ok(())
}

pub(crate) fn print_diagnostic_guidance(startup_error: &LocalStateDbStartupError) {
    eprintln!("Codex couldn't start because its local database appears to be damaged.");
    eprintln!("Run `codex doctor` to check your setup and get next-step guidance.");
    eprintln!("If this keeps happening, share the technical details below when asking for help.");
    print_technical_details(startup_error);
}

pub(crate) fn print_locked_guidance(startup_error: &LocalStateDbStartupError) {
    eprintln!("Codex couldn't start because another Codex process is using its local data.");
    eprintln!("Quit any other copies of Codex that may still be running, then try again.");
    print_technical_details(startup_error);
}

fn print_technical_details(startup_error: &LocalStateDbStartupError) {
    eprintln!("Technical details:");
    eprintln!("  Location: {}", startup_error.state_db_path().display());
    eprintln!("  Cause: {}", startup_error.detail());
}

fn backup_folder(backups: &[RuntimeDbBackup]) -> Option<&Path> {
    backups.first()?.backup_path.parent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn backup_backs_up_owned_database_files() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let state_path = codex_state::state_db_path(temp_dir.path());
        let logs_path = codex_state::logs_db_path(temp_dir.path());
        let goals_path = codex_state::goals_db_path(temp_dir.path());
        let state_wal_path = state_path.with_file_name("state_5.sqlite-wal");
        tokio::fs::write(state_path.as_path(), b"state").await?;
        tokio::fs::write(state_wal_path.as_path(), b"state-wal").await?;
        tokio::fs::write(logs_path.as_path(), b"logs").await?;
        tokio::fs::write(goals_path.as_path(), b"goals").await?;

        let startup_error =
            LocalStateDbStartupError::new(state_path.clone(), "corrupt".to_string());
        let backups = backup_files_for_fresh_start(&startup_error).await?;

        assert_eq!(backups.len(), 4);
        assert!(!tokio::fs::try_exists(state_path.as_path()).await?);
        assert!(!tokio::fs::try_exists(state_wal_path.as_path()).await?);
        assert!(!tokio::fs::try_exists(logs_path.as_path()).await?);
        assert!(!tokio::fs::try_exists(goals_path.as_path()).await?);
        for backup in backups {
            assert!(tokio::fs::try_exists(backup.backup_path.as_path()).await?);
        }
        Ok(())
    }

    #[tokio::test]
    async fn backup_replaces_blocking_sqlite_home_file() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let sqlite_home = temp_dir.path().join("sqlite-home");
        tokio::fs::write(sqlite_home.as_path(), b"not-a-directory").await?;
        let startup_error = LocalStateDbStartupError::new(
            codex_state::state_db_path(sqlite_home.as_path()),
            "File exists".to_string(),
        );

        let backups = backup_files_for_fresh_start(&startup_error).await?;

        assert_eq!(backups.len(), 1);
        assert!(tokio::fs::metadata(sqlite_home.as_path()).await?.is_dir());
        assert!(tokio::fs::try_exists(backups[0].backup_path.as_path()).await?);
        Ok(())
    }

    #[test]
    fn classifies_lock_and_corruption_errors() {
        assert!(is_locked("database is locked"));
        assert!(is_locked("database is busy"));
        assert!(!is_locked("database disk image is malformed"));
        assert!(is_corruption("database disk image is malformed"));
        assert!(!is_corruption("database is locked"));
    }

    #[test]
    fn backup_folder_uses_parent_of_first_backup_path() {
        let backups = vec![RuntimeDbBackup {
            original_path: PathBuf::from("/tmp/state_5.sqlite"),
            backup_path: PathBuf::from("/tmp/db-backups/sqlite-1-0/state_5.sqlite"),
        }];

        assert_eq!(
            backup_folder(&backups),
            Some(Path::new("/tmp/db-backups/sqlite-1-0"))
        );
    }
}
