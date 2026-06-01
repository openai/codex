use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;

use codex_utils_path::SymlinkWritePaths;
use codex_utils_path::resolve_symlink_write_paths;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

static CONFIG_WRITE_LOCKS: LazyLock<StdMutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// A path-keyed config write lock that can be acquired by async or blocking writers.
pub struct ConfigWriteLock {
    write_paths: SymlinkWritePaths,
    lock: Arc<Mutex<()>>,
}

impl ConfigWriteLock {
    pub fn new(config_path: &Path) -> io::Result<Self> {
        let write_paths = resolve_symlink_write_paths(config_path)?;
        let lock = lock_for_path(&write_paths.write_path);
        Ok(Self { write_paths, lock })
    }

    pub async fn lock(self) -> ConfigWriteGuard {
        let guard = self.lock.lock_owned().await;
        ConfigWriteGuard {
            write_paths: self.write_paths,
            _guard: guard,
        }
    }

    pub fn blocking_lock(self) -> ConfigWriteGuard {
        let guard = futures::executor::block_on(self.lock.lock_owned());
        ConfigWriteGuard {
            write_paths: self.write_paths,
            _guard: guard,
        }
    }
}

/// Holds exclusive access to one resolved config file path.
pub struct ConfigWriteGuard {
    write_paths: SymlinkWritePaths,
    _guard: OwnedMutexGuard<()>,
}

impl ConfigWriteGuard {
    pub fn write_paths(&self) -> &SymlinkWritePaths {
        &self.write_paths
    }
}

/// Execute a config read-modify-write transaction under a path-keyed lock.
///
/// The callback receives paths resolved before the lock is acquired so all
/// writers targeting the same symlink destination share one lock.
pub fn with_config_write_lock<T, E>(
    config_path: &Path,
    write: impl FnOnce(&SymlinkWritePaths) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<io::Error>,
{
    let guard = ConfigWriteLock::new(config_path)?.blocking_lock();
    write(guard.write_paths())
}

fn lock_for_path(path: &Path) -> Arc<Mutex<()>> {
    let mut locks = CONFIG_WRITE_LOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks.retain(|_, lock| lock.strong_count() > 0);
    locks
        .entry(path.to_path_buf())
        .or_default()
        .upgrade()
        .unwrap_or_else(|| {
            let lock = Arc::new(Mutex::new(()));
            locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
            lock
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn serializes_writes_to_the_same_path() {
        let tmp = tempdir().expect("tempdir");
        let config_path = tmp.path().join("config.toml");
        let acquired = Arc::new(Barrier::new(2));
        let (release_tx, release_rx) = mpsc::channel();

        let first_path = config_path.clone();
        let first_acquired = acquired.clone();
        let first = thread::spawn(move || {
            with_config_write_lock(&first_path, |_| -> io::Result<()> {
                first_acquired.wait();
                release_rx.recv().expect("release first writer");
                Ok(())
            })
            .expect("first write");
        });
        acquired.wait();

        let (second_tx, second_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            with_config_write_lock(&config_path, |_| -> io::Result<()> {
                second_tx.send(()).expect("record second acquisition");
                Ok(())
            })
            .expect("second write");
        });

        assert!(second_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release_tx.send(()).expect("release first writer");
        first.join().expect("first writer thread");
        second.join().expect("second writer thread");
        second_rx.recv().expect("second writer acquired lock");
    }

    #[tokio::test]
    async fn blocking_lock_can_be_acquired_from_tokio_runtime() {
        let tmp = tempdir().expect("tempdir");
        let config_path = tmp.path().join("config.toml");

        with_config_write_lock(&config_path, |_| -> io::Result<()> { Ok(()) })
            .expect("config write lock");
    }
}
