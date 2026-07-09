use crate::InventoryCache;
use crate::find_repository_root;
use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tempfile::NamedTempFile;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

/// Owns ephemeral, watched ripgrep cache generations for one exec-server.
#[derive(Clone)]
pub struct RgCacheManager {
    inner: Arc<Inner>,
}

struct Inner {
    cache_root: TempDir,
    real_rg: PathBuf,
    repositories: Mutex<HashSet<PathBuf>>,
}

impl fmt::Debug for RgCacheManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RgCacheManager")
            .field("cache_root", &self.inner.cache_root.path())
            .field("real_rg", &self.inner.real_rg)
            .finish_non_exhaustive()
    }
}

impl RgCacheManager {
    /// Creates a process-scoped cache generation below `cache_parent`.
    pub fn new(real_rg: PathBuf, cache_parent: &Path) -> io::Result<Self> {
        fs::create_dir_all(cache_parent)?;
        let cache_root = tempfile::Builder::new()
            .prefix("generation-")
            .tempdir_in(cache_parent)?;
        Ok(Self {
            inner: Arc::new(Inner {
                cache_root,
                real_rg,
                repositories: Mutex::new(HashSet::new()),
            }),
        })
    }

    /// Returns the private cache root exposed read-only to eligible children.
    pub fn cache_root(&self) -> &Path {
        self.inner.cache_root.path()
    }

    /// Returns the canonical Git worktree containing `cwd`.
    pub fn repository_root(cwd: &Path) -> Option<PathBuf> {
        find_repository_root(cwd)
    }

    /// Starts watching and building an inventory for `repository_root`.
    pub fn observe_repository(&self, repository_root: PathBuf) {
        let mut repositories = self
            .inner
            .repositories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !repositories.insert(repository_root.clone()) {
            return;
        }
        drop(repositories);

        let manager = self.clone();
        let worker_root = repository_root.clone();
        tokio::spawn(async move {
            if let Err(error) = manager.run_repository(worker_root.clone()).await {
                warn!(
                    repository = %worker_root.display(),
                    %error,
                    "ripgrep inventory worker stopped"
                );
            }
        });
    }

    async fn run_repository(&self, repository_root: PathBuf) -> io::Result<()> {
        let cache = InventoryCache::new(self.cache_root(), &repository_root)
            .ok_or_else(|| io::Error::other("repository path is not valid UTF-8"))?;
        fs::create_dir_all(&cache.directory)?;
        fs::write(
            &cache.root,
            repository_root
                .to_str()
                .ok_or_else(|| io::Error::other("repository path is not valid UTF-8"))?,
        )?;

        let (event_tx, mut event_rx) = mpsc::channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let callback_generation = Arc::clone(&generation);
        let watch_failed = Arc::new(AtomicBool::new(false));
        let callback_watch_failed = Arc::clone(&watch_failed);
        let watch_root = repository_root.clone();
        let _watcher = tokio::task::spawn_blocking(move || {
            let mut watcher =
                notify::recommended_watcher(move |event: notify::Result<Event>| match event {
                    Ok(event) if event_is_mutating(&event) => {
                        callback_generation.fetch_add(1, Ordering::Release);
                        let _ = event_tx.try_send(());
                    }
                    Ok(_) => {}
                    Err(error) => {
                        warn!(%error, "ripgrep inventory watcher failed");
                        callback_watch_failed.store(true, Ordering::Release);
                        callback_generation.fetch_add(1, Ordering::Release);
                        let _ = event_tx.try_send(());
                    }
                })
                .map_err(io::Error::other)?;
            watcher
                .watch(&watch_root, RecursiveMode::Recursive)
                .map_err(io::Error::other)?;
            Ok::<RecommendedWatcher, io::Error>(watcher)
        })
        .await
        .map_err(io::Error::other)??;

        loop {
            if !self
                .build_clean_generation(
                    &repository_root,
                    &cache,
                    &generation,
                    &watch_failed,
                    &mut event_rx,
                )
                .await?
            {
                return Ok(());
            }
            match event_rx.recv().await {
                Some(()) if !watch_failed.load(Ordering::Acquire) => invalidate(&cache),
                Some(()) | None => {
                    invalidate(&cache);
                    return Ok(());
                }
            }
        }
    }

    async fn build_clean_generation(
        &self,
        repository_root: &Path,
        cache: &InventoryCache,
        generation: &AtomicU64,
        watch_failed: &AtomicBool,
        event_rx: &mut mpsc::Receiver<()>,
    ) -> io::Result<bool> {
        loop {
            while event_rx.try_recv().is_ok() {}
            if event_rx.is_closed() || watch_failed.load(Ordering::Acquire) {
                return Ok(false);
            }
            invalidate(cache);
            let started_at = Instant::now();
            let generation_before = generation.load(Ordering::Acquire);
            let real_rg = self.inner.real_rg.clone();
            let build_root = repository_root.to_path_buf();
            let cache_directory = cache.directory.clone();
            let mut build = tokio::task::spawn_blocking(move || {
                build_inventory(&real_rg, &build_root, &cache_directory)
            });
            let mut dirty = false;
            let inventory = loop {
                tokio::select! {
                    result = &mut build => {
                        break result.map_err(io::Error::other)??;
                    }
                    event = event_rx.recv() => {
                        match event {
                            Some(()) if !watch_failed.load(Ordering::Acquire) => dirty = true,
                            Some(()) | None => return Ok(false),
                        }
                    }
                }
            };
            if watch_failed.load(Ordering::Acquire) {
                return Ok(false);
            }
            if dirty || generation.load(Ordering::Acquire) != generation_before {
                continue;
            }

            publish(cache, inventory)?;
            debug!(
                repository = %repository_root.display(),
                elapsed_ms = started_at.elapsed().as_millis(),
                "ripgrep file inventory is ready"
            );
            return Ok(true);
        }
    }
}

fn build_inventory(
    real_rg: &Path,
    repository_root: &Path,
    cache_directory: &Path,
) -> io::Result<NamedTempFile> {
    let inventory = NamedTempFile::new_in(cache_directory)?;
    let output = inventory.reopen()?;
    let status = Command::new(real_rg)
        .arg("--files")
        .current_dir(repository_root)
        .env_remove(crate::CACHE_ROOT_ENV)
        .env_remove("RIPGREP_CONFIG_PATH")
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "ripgrep inventory exited with {status}"
        )));
    }
    Ok(inventory)
}

fn publish(cache: &InventoryCache, inventory: NamedTempFile) -> io::Result<()> {
    match fs::remove_file(&cache.files) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    inventory
        .persist(&cache.files)
        .map_err(|error| error.error)?;
    let mut ready = NamedTempFile::new_in(&cache.directory)?;
    writeln!(ready, "{}", Uuid::new_v4())?;
    ready.flush()?;
    ready.persist(&cache.ready).map_err(|error| error.error)?;
    Ok(())
}

fn invalidate(cache: &InventoryCache) {
    match fs::remove_file(&cache.ready) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => warn!(%error, "failed to invalidate ripgrep inventory"),
    }
}

fn event_is_mutating(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_))
}

#[cfg(all(test, unix))]
#[path = "manager_tests.rs"]
mod tests;
