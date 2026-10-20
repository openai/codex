//! Owns temporary marketplace checkouts and reclaims abandoned ones.
//! A coordination lock protects publication of each checkout's lifetime lock. Checkouts
//! live below their owner directory so Git and activation cannot replace the lock file.
//! Cleanup claims a limited batch under coordination, then deletes it outside that lock.
//! Legacy directories use age thresholds because older binaries publish no owner lock.

use codex_utils_path::symlink_metadata_if_exists;
use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use tempfile::TempDir;
use tracing::warn;

const LEGACY_ADD_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
// Upgrades have a 30-second timeout per Git command; leave ample room for all phases.
const LEGACY_UPGRADE_MAX_AGE: Duration = Duration::from_secs(10 * 60);
const LEGACY_ADD_PREFIX: &str = "marketplace-add-";
const LEGACY_UPGRADE_PREFIX: &str = "marketplace-upgrade-";
const MAX_CLEANUP_DIRECTORIES: usize = 4;
const COORDINATION_LOCK: &str = ".staging.lock";
const CLEANUP_LOCK: &str = ".staging-cleanup.lock";
const OWNER_LOCK: &str = ".lock";

pub(crate) struct MarketplaceStagingDir {
    // Close the lock before TempDir removes its directory, including on Windows.
    _lock: File,
    _directory: TempDir,
    checkout: PathBuf,
}

impl MarketplaceStagingDir {
    pub(crate) fn new(install_root: &Path) -> io::Result<Self> {
        let staging_root = Self::staging_root(install_root);
        fs::create_dir_all(&staging_root)?;
        let coordination = open_lock(&install_root.join(COORDINATION_LOCK))?;
        coordination.lock()?;
        let directory = tempfile::Builder::new()
            .prefix(&Self::directory_prefix())
            .tempdir_in(&staging_root)?;
        let lock = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join(OWNER_LOCK))?;
        lock.lock()?;
        let checkout = directory.path().join("checkout");
        Ok(Self {
            _lock: lock,
            _directory: directory,
            checkout,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.checkout
    }

    fn staging_root(install_root: &Path) -> PathBuf {
        install_root.join(".staging")
    }

    fn directory_prefix() -> String {
        // Windows and WSL may share a home without sharing a file-lock namespace.
        format!("marketplace-{}-", std::env::consts::OS)
    }
}

pub(crate) fn cleanup_marketplace_staging(install_root: &Path) {
    if let Err(err) = remove_stale_staging_dirs(install_root, SystemTime::now()) {
        warn!("failed to clean up marketplace staging directories: {err}");
    }
}

fn open_lock(path: &Path) -> io::Result<File> {
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

fn try_lock(lock: File) -> io::Result<Option<File>> {
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(err)) => Err(err),
    }
}

fn remove_stale_staging_dirs(install_root: &Path, now: SystemTime) -> io::Result<()> {
    let staging_root = MarketplaceStagingDir::staging_root(install_root);
    let Some(metadata) = symlink_metadata_if_exists(&staging_root)? else {
        return Ok(());
    };
    if !metadata.is_dir() {
        return Ok(());
    }
    // Skip this pass if another process creates a checkout or removes a batch.
    let Some(_cleanup) = try_lock(open_lock(&install_root.join(CLEANUP_LOCK))?)? else {
        return Ok(());
    };
    let Some(coordination) = try_lock(open_lock(&install_root.join(COORDINATION_LOCK))?)? else {
        return Ok(());
    };
    let prefix = MarketplaceStagingDir::directory_prefix();
    let mut abandoned = Vec::new();
    for entry in fs::read_dir(&staging_root)? {
        let entry = entry?;
        let path = entry.path();
        // Continue past an unreadable entry so one failure cannot prevent reclamation.
        match is_abandoned_staging_dir(&entry, &prefix, now) {
            Ok(true) => abandoned.push(path),
            Ok(false) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                warn!(path = %path.display(), "failed to inspect marketplace staging directory: {err}");
            }
        }
        if abandoned.len() == MAX_CLEANUP_DIRECTORIES {
            break;
        }
    }
    // Unique checkout names cannot acquire a new owner after publication.
    // Keep the cleanup lock, but let new checkouts start while deletion runs.
    drop(coordination);
    for path in abandoned {
        if let Err(err) = fs::remove_dir_all(&path)
            && err.kind() != io::ErrorKind::NotFound
        {
            warn!(path = %path.display(), "failed to remove abandoned marketplace staging directory: {err}");
        }
    }
    Ok(())
}

fn is_abandoned_staging_dir(
    entry: &fs::DirEntry,
    prefix: &str,
    now: SystemTime,
) -> io::Result<bool> {
    if !entry.file_type()?.is_dir() {
        return Ok(false);
    }
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
        return Ok(false);
    };
    if !name.starts_with(prefix) {
        // Older binaries did not publish ownership. Leave recent directories
        // alone so an upgrade can coexist with their in-flight clones.
        let max_age = if name.starts_with(LEGACY_ADD_PREFIX) {
            LEGACY_ADD_MAX_AGE
        } else if name.starts_with(LEGACY_UPGRADE_PREFIX) {
            LEGACY_UPGRADE_MAX_AGE
        } else {
            return Ok(false);
        };
        return Ok(now
            .duration_since(entry.metadata()?.modified()?)
            .unwrap_or_default()
            >= max_age);
    }
    let owner_lock = entry.path().join(OWNER_LOCK);
    let Some(metadata) = symlink_metadata_if_exists(&owner_lock)? else {
        // A crash before lock publication can leave an empty directory.
        return Ok(true);
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    // An unlocked checkout has no Codex owner to activate it. A surviving Git
    // process may still write here; deletion can interrupt that abandoned clone.
    let lock = File::options().read(true).write(true).open(owner_lock)?;
    // Close the probe's lock before cleanup removes the directory on Windows.
    Ok(try_lock(lock)?.is_some())
}

#[cfg(test)]
#[path = "marketplace_staging_tests.rs"]
mod tests;
