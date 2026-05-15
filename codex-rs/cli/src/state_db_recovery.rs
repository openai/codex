//! CLI recovery for local state database startup failures.
//!
//! This keeps user-facing repair and lock-contention handling out of the main
//! CLI dispatch path while preserving the TUI startup error as the boundary type.

use codex_tui::LocalStateDbStartupError;
use std::path::PathBuf;

pub(crate) fn startup_error(err: &std::io::Error) -> Option<&LocalStateDbStartupError> {
    err.get_ref()
        .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
}

pub(crate) fn is_locked(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("database is locked") || detail.contains("database is busy")
}

pub(crate) fn confirm_repair(startup_error: &LocalStateDbStartupError) -> std::io::Result<bool> {
    eprintln!("Codex couldn't start because its local database appears to be damaged.");
    eprintln!("Codex can try a safe repair by backing up those files and rebuilding them.");
    print_technical_details(startup_error);
    crate::confirm("Repair Codex local data now? [y/N]: ")
}

pub(crate) async fn repair_files(
    startup_error: &LocalStateDbStartupError,
) -> std::io::Result<Vec<PathBuf>> {
    let state_db_path = startup_error.state_db_path();
    let sqlite_home = state_db_path.parent().ok_or_else(|| {
        std::io::Error::other("state database path does not have a parent directory")
    })?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let repair_suffix = format!("codex-repair-{timestamp}");
    let mut backups = Vec::new();

    match tokio::fs::metadata(sqlite_home).await {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            backups.push(backup_path(sqlite_home, &repair_suffix).await?);
            tokio::fs::create_dir_all(sqlite_home).await?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir_all(sqlite_home).await?;
        }
        Err(err) => return Err(err),
    }

    let logs_db_path = codex_state::logs_db_path(sqlite_home);
    for path in sqlite_paths(state_db_path)
        .into_iter()
        .chain(sqlite_paths(logs_db_path.as_path()))
    {
        if tokio::fs::try_exists(path.as_path()).await? {
            backups.push(backup_path(path.as_path(), &repair_suffix).await?);
        }
    }

    if backups.is_empty() {
        return Err(std::io::Error::other(
            "no repairable Codex local data files were found",
        ));
    }

    Ok(backups)
}

pub(crate) fn print_repair_backups(backups: &[PathBuf]) {
    eprintln!("Backed up Codex local data before repair:");
    for backup in backups {
        eprintln!("  {}", backup.display());
    }
    eprintln!("Retrying startup with rebuilt local data...");
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

fn sqlite_paths(db_path: &std::path::Path) -> Vec<PathBuf> {
    let mut wal_path = db_path.as_os_str().to_os_string();
    wal_path.push("-wal");
    let mut shm_path = db_path.as_os_str().to_os_string();
    shm_path.push("-shm");
    vec![
        db_path.to_path_buf(),
        PathBuf::from(wal_path),
        PathBuf::from(shm_path),
    ]
}

async fn backup_path(path: &std::path::Path, repair_suffix: &str) -> std::io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::other(format!(
            "cannot create a repair backup name for {}",
            path.display()
        ))
    })?;
    let mut sequence = 0;
    loop {
        let mut backup_name = file_name.to_os_string();
        backup_name.push(format!(".{repair_suffix}.{sequence}.bak"));
        let backup_path = path.with_file_name(backup_name);
        if !tokio::fs::try_exists(backup_path.as_path()).await? {
            tokio::fs::rename(path, backup_path.as_path()).await?;
            return Ok(backup_path);
        }
        sequence += 1;
    }
}

fn print_technical_details(startup_error: &LocalStateDbStartupError) {
    eprintln!("Technical details:");
    eprintln!("  Location: {}", startup_error.state_db_path().display());
    eprintln!("  Cause: {}", startup_error.detail());
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn repair_backs_up_owned_database_files() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let state_path = codex_state::state_db_path(temp_dir.path());
        let logs_path = codex_state::logs_db_path(temp_dir.path());
        let state_sidecars = sqlite_paths(state_path.as_path());
        tokio::fs::write(state_path.as_path(), b"state").await?;
        tokio::fs::write(state_sidecars[1].as_path(), b"state-wal").await?;
        tokio::fs::write(logs_path.as_path(), b"logs").await?;

        let startup_error =
            LocalStateDbStartupError::new(state_path.clone(), "corrupt".to_string());
        let backups = repair_files(&startup_error).await?;

        assert_eq!(backups.len(), 3);
        assert!(!tokio::fs::try_exists(state_path.as_path()).await?);
        assert!(!tokio::fs::try_exists(state_sidecars[1].as_path()).await?);
        assert!(!tokio::fs::try_exists(logs_path.as_path()).await?);
        for backup in backups {
            assert!(tokio::fs::try_exists(backup.as_path()).await?);
        }
        Ok(())
    }

    #[tokio::test]
    async fn repair_replaces_blocking_sqlite_home_file() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let sqlite_home = temp_dir.path().join("sqlite-home");
        tokio::fs::write(sqlite_home.as_path(), b"not-a-directory").await?;
        let startup_error = LocalStateDbStartupError::new(
            codex_state::state_db_path(sqlite_home.as_path()),
            "File exists".to_string(),
        );

        let backups = repair_files(&startup_error).await?;

        assert_eq!(backups.len(), 1);
        assert!(tokio::fs::metadata(sqlite_home.as_path()).await?.is_dir());
        assert!(tokio::fs::try_exists(backups[0].as_path()).await?);
        Ok(())
    }

    #[test]
    fn lock_failures_skip_repair() {
        assert!(is_locked("database is locked"));
        assert!(is_locked("database is busy"));
        assert!(!is_locked("database disk image is malformed"));
    }
}
