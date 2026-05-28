use std::ffi::OsStr;
use std::fs::File;
use std::fs::FileTimes;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use tokio::io::AsyncBufReadExt;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::ARCHIVED_SESSIONS_SUBDIR;
use crate::SESSIONS_SUBDIR;

const COMPRESSED_SUFFIX: &str = ".zst";
const TEMP_SUFFIX: &str = ".tmp";
const COMPRESSION_LEVEL: i32 = 3;
const MIN_ROLLOUT_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const GLOBAL_LOCK_STALE_AFTER: Duration = Duration::from_secs(6 * 60 * 60);
const WORKER_MAX_RUNTIME: Duration = Duration::from_secs(5 * 60 * 60);
const LOCK_FILE_NAME: &str = "rollout-compression.lock";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Starts a best-effort background job that compresses cold local rollout files.
///
/// The worker is fire-and-forget: failures are logged, startup is not blocked,
/// and a process-wide lock under `codex_home` prevents overlapping compression
/// runs from the same local store.
pub fn spawn_rollout_compression_worker(codex_home: PathBuf) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        warn!(
            "failed to start rollout compression worker for {}: no Tokio runtime",
            codex_home.display()
        );
        return;
    };
    handle.spawn(async move {
        if let Err(err) = run_rollout_compression_worker(codex_home.clone()).await {
            warn!(
                "rollout compression worker failed for {}: {err}",
                codex_home.display()
            );
        }
    });
}

async fn run_rollout_compression_worker(codex_home: PathBuf) -> io::Result<()> {
    let Some(_lock) = CompressionLock::try_acquire(codex_home.as_path())? else {
        debug!(
            "rollout compression worker already running for {}",
            codex_home.display()
        );
        return Ok(());
    };

    let started_at = Instant::now();
    cleanup_stale_temps(codex_home.as_path()).await?;
    let mut stats = CompressionStats::default();
    for root in [
        codex_home.join(SESSIONS_SUBDIR),
        codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
    ] {
        if started_at.elapsed() >= WORKER_MAX_RUNTIME {
            break;
        }
        compress_rollouts_in_root(root.as_path(), started_at, &mut stats).await?;
    }
    info!(
        "rollout compression worker finished: scanned={}, compressed={}, skipped={}, failed={}",
        stats.scanned, stats.compressed, stats.skipped, stats.failed
    );
    Ok(())
}

pub(crate) async fn file_modified_time(path: &Path) -> io::Result<Option<time::OffsetDateTime>> {
    let Some(path) = existing_rollout_path(path).await else {
        return Ok(None);
    };
    let meta = tokio::fs::metadata(path).await?;
    let modified = meta.modified().ok();
    Ok(modified.map(time::OffsetDateTime::from))
}

/// Opens a rollout line reader that transparently handles plain `.jsonl` and `.jsonl.zst` files.
///
/// If the requested path disappears during a compression or decompression transition, this retries
/// the matching plain/compressed sibling once so readers do not need to know which representation is
/// currently stored on disk.
pub async fn open_rollout_line_reader(path: &Path) -> io::Result<RolloutLineReader> {
    match open_rollout_line_reader_once(path).await {
        Ok(reader) => Ok(reader),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            open_rollout_line_reader_alternate(path).await
        }
        Err(err) => Err(err),
    }
}

pub(crate) async fn materialize_rollout_for_append(path: &Path) -> io::Result<PathBuf> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || materialize_rollout_for_append_blocking(path.as_path()))
        .await
        .map_err(io::Error::other)?
}

pub(crate) fn materialize_rollout_for_append_blocking(path: &Path) -> io::Result<PathBuf> {
    let plain_path = plain_rollout_path(path);
    if plain_path.exists() {
        return Ok(plain_path);
    }
    let compressed_path = compressed_rollout_path(plain_path.as_path());
    if !compressed_path.exists() {
        return Ok(plain_path);
    }

    let temp_path = temp_path_for(plain_path.as_path(), "decompress");
    if let Some(parent) = plain_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let result = (|| {
        let input = File::open(compressed_path.as_path())?;
        let mut decoder = zstd::stream::read::Decoder::new(input)?;
        let mut output = File::create(temp_path.as_path())?;
        io::copy(&mut decoder, &mut output)?;
        output.flush()?;
        match std::fs::hard_link(temp_path.as_path(), plain_path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
        let _ = std::fs::remove_file(temp_path.as_path());
        match std::fs::remove_file(compressed_path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp_path.as_path());
    }
    result?;
    Ok(plain_path)
}

pub(crate) fn compressed_rollout_path(path: &Path) -> PathBuf {
    if is_compressed_rollout_path(path) {
        return path.to_path_buf();
    }
    let mut file_name = path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsStr::new("rollout.jsonl").to_os_string());
    file_name.push(COMPRESSED_SUFFIX);
    path.with_file_name(file_name)
}

pub(crate) fn plain_rollout_path(path: &Path) -> PathBuf {
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return path.to_path_buf();
    };
    let Some(plain_file_name) = file_name.strip_suffix(COMPRESSED_SUFFIX) else {
        return path.to_path_buf();
    };
    path.with_file_name(plain_file_name)
}

pub(crate) fn is_compressed_rollout_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
}

pub(crate) fn is_rollout_file_name(name: &str) -> bool {
    parse_rollout_file_name(name).is_some()
}

pub(crate) fn parse_rollout_file_name(name: &str) -> Option<&str> {
    let name = name.strip_suffix(COMPRESSED_SUFFIX).unwrap_or(name);
    if name.starts_with("rollout-") && name.ends_with(".jsonl") {
        Some(name)
    } else {
        None
    }
}

pub(crate) fn should_skip_compressed_sibling(path: &Path) -> bool {
    is_compressed_rollout_path(path) && plain_rollout_path(path).exists()
}

/// Line-oriented rollout reader returned by [`open_rollout_line_reader`].
pub struct RolloutLineReader {
    inner: RolloutLineReaderInner,
}

enum RolloutLineReaderInner {
    Plain(tokio::io::Lines<tokio::io::BufReader<tokio::fs::File>>),
    Memory(std::io::Lines<std::io::Cursor<String>>),
}

impl RolloutLineReader {
    /// Reads the next JSONL record from the rollout.
    pub async fn next_line(&mut self) -> io::Result<Option<String>> {
        match &mut self.inner {
            RolloutLineReaderInner::Plain(lines) => lines.next_line().await,
            RolloutLineReaderInner::Memory(lines) => lines.next().transpose(),
        }
    }
}

#[derive(Default)]
struct CompressionStats {
    scanned: usize,
    compressed: usize,
    skipped: usize,
    failed: usize,
}

struct CompressionLock {
    path: PathBuf,
}

impl CompressionLock {
    fn try_acquire(codex_home: &Path) -> io::Result<Option<Self>> {
        let lock_dir = codex_home.join(".tmp");
        std::fs::create_dir_all(lock_dir.as_path())?;
        let path = lock_dir.join(LOCK_FILE_NAME);
        match create_lock_file(path.as_path()) {
            Ok(()) => return Ok(Some(Self { path })),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }

        let stale = std::fs::metadata(path.as_path())
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= GLOBAL_LOCK_STALE_AFTER);
        if !stale {
            return Ok(None);
        }
        match std::fs::remove_file(path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        match create_lock_file(path.as_path()) {
            Ok(()) => Ok(Some(Self { path })),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(None),
            Err(err) => Err(err),
        }
    }
}

impl Drop for CompressionLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path.as_path());
    }
}

fn create_lock_file(path: &Path) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    writeln!(
        file,
        "pid={} started_at={:?}",
        std::process::id(),
        SystemTime::now()
    )?;
    Ok(())
}

async fn compress_rollouts_in_root(
    root: &Path,
    started_at: Instant,
    stats: &mut CompressionStats,
) -> io::Result<()> {
    if !tokio::fs::try_exists(root).await.unwrap_or(false) {
        return Ok(());
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if started_at.elapsed() >= WORKER_MAX_RUNTIME {
            break;
        }
        let mut read_dir = match tokio::fs::read_dir(dir.as_path()).await {
            Ok(read_dir) => read_dir,
            Err(err) => {
                warn!(
                    "failed to read rollout compression directory {}: {err}",
                    dir.display()
                );
                continue;
            }
        };
        while let Some(entry) = read_dir.next_entry().await? {
            if started_at.elapsed() >= WORKER_MAX_RUNTIME {
                break;
            }
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(file_type) => file_type,
                Err(err) => {
                    warn!(
                        "failed to read rollout compression file type {}: {err}",
                        path.display()
                    );
                    continue;
                }
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                continue;
            };
            if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
                continue;
            }
            stats.scanned = stats.scanned.saturating_add(1);
            match compress_rollout_if_cold(path.as_path()).await {
                Ok(true) => stats.compressed = stats.compressed.saturating_add(1),
                Ok(false) => stats.skipped = stats.skipped.saturating_add(1),
                Err(err) => {
                    stats.failed = stats.failed.saturating_add(1);
                    warn!("failed to compress rollout {}: {err}", path.display());
                }
            }
        }
    }
    Ok(())
}

async fn compress_rollout_if_cold(path: &Path) -> io::Result<bool> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || compress_rollout_if_cold_blocking(path.as_path()))
        .await
        .map_err(io::Error::other)?
}

fn compress_rollout_if_cold_blocking(path: &Path) -> io::Result<bool> {
    let before = match cold_file_state(path)? {
        Some(state) => state,
        None => return Ok(false),
    };
    let compressed_path = compressed_rollout_path(path);
    let temp_path = temp_path_for(compressed_path.as_path(), "compress");
    let result = (|| {
        encode_zstd(path, temp_path.as_path())?;
        verify_zstd(temp_path.as_path())?;
        if !same_file_state(path, &before)? {
            return Ok(false);
        }
        set_modified_time(temp_path.as_path(), before.modified)?;
        match std::fs::remove_file(compressed_path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        std::fs::rename(temp_path.as_path(), compressed_path.as_path())?;
        if !same_file_state(path, &before)? {
            let _ = std::fs::remove_file(compressed_path.as_path());
            return Ok(false);
        }
        std::fs::remove_file(path)?;
        Ok(true)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp_path.as_path());
    }
    result
}

#[derive(Clone, Copy)]
struct FileState {
    len: u64,
    modified: SystemTime,
}

fn cold_file_state(path: &Path) -> io::Result<Option<FileState>> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    let modified = metadata.modified()?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    if age < MIN_ROLLOUT_AGE {
        return Ok(None);
    }
    Ok(Some(FileState {
        len: metadata.len(),
        modified,
    }))
}

fn same_file_state(path: &Path, expected: &FileState) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            Ok(metadata.len() == expected.len && metadata.modified()? == expected.modified)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn encode_zstd(source: &Path, temp_path: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let output = File::create(temp_path)?;
    let mut encoder = zstd::stream::write::Encoder::new(output, COMPRESSION_LEVEL)?;
    io::copy(&mut input, &mut encoder)?;
    encoder.finish()?;
    Ok(())
}

fn verify_zstd(path: &Path) -> io::Result<()> {
    let input = File::open(path)?;
    let mut decoder = zstd::stream::read::Decoder::new(input)?;
    let mut sink = io::sink();
    io::copy(&mut decoder, &mut sink)?;
    Ok(())
}

fn set_modified_time(path: &Path, modified: SystemTime) -> io::Result<()> {
    let times = FileTimes::new().set_modified(modified);
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(times)
}

async fn cleanup_stale_temps(codex_home: &Path) -> io::Result<()> {
    for root in [
        codex_home.join(SESSIONS_SUBDIR),
        codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
    ] {
        cleanup_stale_temps_in_root(root.as_path()).await?;
    }
    Ok(())
}

async fn cleanup_stale_temps_in_root(root: &Path) -> io::Result<()> {
    if !tokio::fs::try_exists(root).await.unwrap_or(false) {
        return Ok(());
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(dir.as_path()).await {
            Ok(read_dir) => read_dir,
            Err(err) => {
                warn!(
                    "failed to read rollout temp cleanup directory {}: {err}",
                    dir.display()
                );
                continue;
            }
        };
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(file_type) => file_type,
                Err(err) => {
                    warn!(
                        "failed to read rollout temp cleanup file type {}: {err}",
                        path.display()
                    );
                    continue;
                }
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file()
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.ends_with(TEMP_SUFFIX))
            {
                match tokio::fs::remove_file(path.as_path()).await {
                    Ok(()) => {}
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                    Err(err) => warn!(
                        "failed to remove stale rollout temp {}: {err}",
                        path.display()
                    ),
                }
            }
        }
    }
    Ok(())
}

/// Returns the existing rollout path, preferring the plain `.jsonl` file over
/// its `.jsonl.zst` compressed sibling.
pub async fn existing_rollout_path(path: &Path) -> Option<PathBuf> {
    let plain_path = plain_rollout_path(path);
    if tokio::fs::try_exists(plain_path.as_path())
        .await
        .unwrap_or(false)
    {
        return Some(plain_path);
    }
    let compressed_path = compressed_rollout_path(plain_path.as_path());
    if tokio::fs::try_exists(compressed_path.as_path())
        .await
        .unwrap_or(false)
    {
        return Some(compressed_path);
    }
    None
}

async fn open_rollout_line_reader_once(path: &Path) -> io::Result<RolloutLineReader> {
    let path = existing_rollout_path(path)
        .await
        .unwrap_or_else(|| path.to_path_buf());
    if is_compressed_rollout_path(path.as_path()) {
        return open_compressed_reader(path).await;
    }
    let file = tokio::fs::File::open(path).await?;
    Ok(RolloutLineReader {
        inner: RolloutLineReaderInner::Plain(tokio::io::BufReader::new(file).lines()),
    })
}

async fn open_rollout_line_reader_alternate(path: &Path) -> io::Result<RolloutLineReader> {
    let plain_path = plain_rollout_path(path);
    let compressed_path = compressed_rollout_path(plain_path.as_path());
    if is_compressed_rollout_path(path) {
        let file = tokio::fs::File::open(plain_path).await?;
        return Ok(RolloutLineReader {
            inner: RolloutLineReaderInner::Plain(tokio::io::BufReader::new(file).lines()),
        });
    }
    open_compressed_reader(compressed_path).await
}

async fn open_compressed_reader(path: PathBuf) -> io::Result<RolloutLineReader> {
    let text = tokio::task::spawn_blocking(move || decode_zstd_to_string(path.as_path()))
        .await
        .map_err(io::Error::other)??;
    Ok(RolloutLineReader {
        inner: RolloutLineReaderInner::Memory(std::io::BufRead::lines(io::Cursor::new(text))),
    })
}

fn decode_zstd_to_string(path: &Path) -> io::Result<String> {
    let input = File::open(path)?;
    let mut decoder = zstd::stream::read::Decoder::new(input)?;
    let mut text = String::new();
    decoder.read_to_string(&mut text)?;
    Ok(text)
}

fn temp_path_for(path: &Path, operation: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsStr::new("rollout").to_os_string());
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    file_name.push(format!(".{operation}.{}.{counter}.tmp", std::process::id()));
    path.with_file_name(file_name)
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
