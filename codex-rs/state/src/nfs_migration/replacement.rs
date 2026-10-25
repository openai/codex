//! Move the WAL files into a backup directory, then install the validated DELETE copy.
//! Resume an interrupted replacement from the files that remain on disk.
//! All clients must remain stopped. The caller must hold the migration lock.
//! Sync both directories after each step before changing more files.

use anyhow::Context;
use anyhow::ensure;
use sqlx::ConnectOptions;
use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteConnectOptions;
use std::fs;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const MARKER: &[u8] = b"codex-nfs-migration-v1\n";
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

pub(super) enum Recovery {
    // The caller must check and lock the source before starting a new replacement.
    InspectSource,
    // Recovery installed the saved copy. No new replacement is necessary.
    Installed,
}

pub(super) struct Replacement {
    source: PathBuf,
    pub(super) backup_directory: PathBuf,
    pub(super) backup_database: PathBuf,
}

impl Replacement {
    pub(super) fn new(source: &Path) -> anyhow::Result<Self> {
        let name = source.file_name().context("database needs a file name")?;
        let mut directory = source.as_os_str().to_owned();
        directory.push("_wal_migration_backup");
        let directory = PathBuf::from(directory);
        Ok(Self {
            source: source.to_path_buf(),
            backup_database: directory.join(name),
            backup_directory: directory,
        })
    }

    pub(super) fn replacement_path(&self) -> PathBuf {
        self.backup_directory.join("delete.sqlite")
    }

    // Call only while the original database is locked. Rebuild an unused copy on every attempt.
    pub(super) fn prepare_backup_directory(&self) -> anyhow::Result<()> {
        if self.backup_directory.try_exists()? {
            ensure!(
                !regular_file_exists(&self.backup_database)?,
                "WAL backup already exists"
            );
            if regular_file_exists(&self.replacement_path())? {
                fs::remove_file(self.replacement_path())?;
            }
        } else {
            let builder = fs::DirBuilder::new();
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = builder;
                builder.mode(0o700);
                builder
            };
            builder.create(&self.backup_directory)?;
            let mut marker = File::create_new(self.backup_directory.join("migration-v1"))?;
            marker.write_all(MARKER)?;
            marker.sync_all()?;
        }
        self.sync_directories()
    }

    pub(super) fn move_original_to_backup(&self) -> anyhow::Result<()> {
        ensure!(
            !regular_file_exists(&self.backup_database)?,
            "WAL backup already exists"
        );
        // Move the main file first. An incomplete switch must not look like an intact source.
        fs::rename(&self.source, &self.backup_database)?;
        self.sync_directories()?;
        self.move_sidecars_to_backup()
    }

    fn move_sidecars_to_backup(&self) -> anyhow::Result<()> {
        for suffix in SIDECARS {
            let source = with_suffix(&self.source, suffix);
            let destination = with_suffix(&self.backup_database, suffix);
            if regular_file_exists(&source)? {
                ensure!(
                    !regular_file_exists(&destination)?,
                    "backup sidecar already exists: {}",
                    destination.display()
                );
                fs::rename(source, destination)?;
            }
        }
        self.sync_directories()
    }

    pub(super) fn install(&self) -> anyhow::Result<()> {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            ensure!(
                !regular_file_exists(&with_suffix(&self.source, suffix))?,
                "database or sidecar already exists at the installation path"
            );
        }
        // A hard link installs the file atomically without replacing an unexpected database.
        fs::hard_link(self.replacement_path(), &self.source)?;
        self.finish_installation()
    }

    fn finish_installation(&self) -> anyhow::Result<()> {
        self.sync_directories()?;
        fs::remove_file(self.replacement_path())?;
        self.sync_directories()
    }

    fn sync_directories(&self) -> anyhow::Result<()> {
        File::open(&self.backup_directory)?.sync_all()?;
        File::open(
            self.source
                .parent()
                .context("database needs a parent directory")?,
        )?
        .sync_all()?;
        Ok(())
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "Inspect the prepared copy without runtime initialization"
    )]
    pub(super) async fn resume(&self) -> anyhow::Result<Recovery> {
        match fs::symlink_metadata(&self.backup_directory) {
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(Recovery::InspectSource);
            }
            Err(error) => return Err(error.into()),
            Ok(metadata) => ensure!(
                metadata.is_dir(),
                "backup path already exists and is not a directory"
            ),
        }
        let marker = self.backup_directory.join("migration-v1");
        ensure!(
            regular_file_exists(&marker)? && fs::read(&marker)? == MARKER,
            "backup directory already exists without a valid migration marker: {}",
            self.backup_directory.display()
        );
        let backup_exists = regular_file_exists(&self.backup_database)?;
        let replacement_exists = regular_file_exists(&self.replacement_path())?;
        let source_exists = regular_file_exists(&self.source)?;
        // No original files moved yet. The caller must lock the source and rebuild the copy.
        if !backup_exists {
            for suffix in SIDECARS {
                ensure!(
                    !regular_file_exists(&with_suffix(&self.backup_database, suffix))?,
                    "backup has a SQLite sidecar without its original database"
                );
            }
            ensure!(
                source_exists,
                "original database is missing from both source and backup"
            );
            return Ok(Recovery::InspectSource);
        }
        // Installation finished, or the replacement is missing. The caller must check the source.
        if !replacement_exists {
            ensure!(
                source_exists,
                "replacement is missing. Preserve the WAL backup and restore it before retrying"
            );
            return Ok(Recovery::InspectSource);
        }
        // An existing source must be the link installed before the interruption.
        if source_exists {
            let same_file = {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let source = fs::metadata(&self.source)?;
                    let prepared = fs::metadata(self.replacement_path())?;
                    (source.dev(), source.ino()) == (prepared.dev(), prepared.ino())
                }
                #[cfg(not(unix))]
                {
                    false
                }
            };
            ensure!(
                same_file,
                "another database already exists at the installation path"
            );
            self.finish_installation()?;
            return Ok(Recovery::Installed);
        }

        // The original is in the backup. Check the replacement before installing it.
        let mut copy = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(self.replacement_path())
                .read_only(true)
                .immutable(true)
                .disable_statement_logging(),
        )
        .await?;
        let checked = super::validate_delete_database(&mut copy)
            .await
            .context("saved replacement failed validation");
        let closed = copy.close().await;
        checked?;
        closed?;
        self.move_sidecars_to_backup()?;
        self.install()?;
        Ok(Recovery::Installed)
    }
}

pub(super) fn regular_file_exists(path: &Path) -> anyhow::Result<bool> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "path already exists and is not a regular file: {}",
                path.display()
            );
            Ok(true)
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}
