use std::ffi::OsStr;
use std::fs::File;
use std::fs::Permissions;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const COMPRESSED_SUFFIX: &str = ".zst";
const MAX_NOT_FOUND_RETRIES: usize = 3;
const OPEN_ROLLOUT_LINE_READER_RETRY_DELAY: Duration = Duration::from_millis(50);
const TEMP_SUFFIX: &str = ".tmp";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Starts a best-effort background job that compresses cold local rollout files.
///
/// The worker is fire-and-forget: failures are logged, startup is not blocked,
/// and a run marker under `codex_home` prevents overlapping or too-frequent
/// compression runs from the same local store.
pub fn spawn_rollout_compression_worker(codex_home: PathBuf) {
    worker::spawn(codex_home)
}

/// Returns the modified time for the existing plain or compressed rollout file.
pub(crate) async fn file_modified_time(path: &Path) -> io::Result<Option<time::OffsetDateTime>> {
    let Some(path) = path::existing_rollout_path(path).await else {
        return Ok(None);
    };
    let meta = tokio::fs::metadata(path).await?;
    let modified = meta.modified().ok();
    Ok(modified.map(time::OffsetDateTime::from))
}

/// Opens a rollout line reader that transparently handles plain `.jsonl` and `.jsonl.zst` files.
///
/// If the requested path disappears during a representation transition, this briefly retries
/// resolution so callers do not need to know which representation is on disk.
pub async fn open_rollout_line_reader(path: &Path) -> io::Result<RolloutLineReader> {
    for _ in 0..MAX_NOT_FOUND_RETRIES {
        match reader::open_once(path).await {
            Ok(reader) => return Ok(reader),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                tokio::time::sleep(OPEN_ROLLOUT_LINE_READER_RETRY_DELAY).await;
            }
            Err(err) => return Err(err),
        }
    }
    reader::open_once(path).await
}

/// Returns the compressed `.jsonl.zst` path for a rollout path.
#[cfg(test)]
pub(crate) fn compressed_rollout_path(path: &Path) -> PathBuf {
    path::compressed_rollout_path(path)
}

/// Materializes a compressed rollout back to plain `.jsonl` for async append paths.
pub(crate) async fn materialize_rollout_for_append(path: &Path) -> io::Result<PathBuf> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || materialize_rollout_for_append_blocking(path.as_path()))
        .await
        .map_err(io::Error::other)?
}

/// Materializes a compressed rollout back to plain `.jsonl` for blocking append paths.
pub(crate) fn materialize_rollout_for_append_blocking(path: &Path) -> io::Result<PathBuf> {
    let plain_path = plain_rollout_path(path);
    if plain_path.exists() {
        metrics::materialize("plain_exists");
        return Ok(plain_path);
    }
    let compressed_path = path::compressed_rollout_path(plain_path.as_path());
    if !compressed_path.exists() {
        metrics::materialize("missing");
        return Ok(plain_path);
    }

    let temp_path = temp_path_for(plain_path.as_path(), "decompress");
    if let Some(parent) = plain_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let result: io::Result<()> = (|| {
        let permissions = std::fs::metadata(compressed_path.as_path())?.permissions();
        {
            let input = File::open(compressed_path.as_path())?;
            let mut decoder = zstd::stream::read::Decoder::new(input)?;
            let mut output = create_file_with_permissions(temp_path.as_path(), &permissions)?;
            io::copy(&mut decoder, &mut output)?;
            output.flush()?;
            output.sync_all()?;
        }
        match std::fs::hard_link(temp_path.as_path(), plain_path.as_path()) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => persist_temp_file_noclobber(temp_path.as_path(), plain_path.as_path())?,
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
        metrics::materialize("failed");
    }
    result?;
    metrics::materialize("decompressed");
    Ok(plain_path)
}

fn persist_temp_file_noclobber(temp_path: &Path, destination: &Path) -> io::Result<()> {
    let temp_path = tempfile::TempPath::try_from_path(temp_path)?;
    match temp_path.persist_noclobber(destination) {
        Ok(()) => Ok(()),
        Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err.error),
    }
}

/// Returns the plain `.jsonl` path for a plain or compressed rollout path.
pub fn plain_rollout_path(path: &Path) -> PathBuf {
    path::plain_rollout_path(path)
}

/// Parses a rollout file name, returning its plain `.jsonl` name when valid.
pub(crate) fn parse_rollout_file_name(name: &str) -> Option<&str> {
    file_name::parse_rollout_file_name(name)
}

/// A discovered rollout file, represented by exactly one physical path.
///
/// This keeps directory walkers from reimplementing the plain/compressed
/// precedence rules. The physical path may point at either `.jsonl` or
/// `.jsonl.zst`, while `plain_file_name` is always the canonical `.jsonl`
/// filename used for timestamp and id parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RolloutFile {
    path: PathBuf,
    plain_file_name: String,
}

impl RolloutFile {
    /// Creates a logical rollout file from a physical path found during discovery.
    ///
    /// Returns `None` for non-rollout names and for compressed siblings hidden by
    /// an existing plain `.jsonl` file.
    pub(crate) fn from_path(path: PathBuf) -> Option<Self> {
        let file_name = path.file_name().and_then(|name| name.to_str())?;
        let plain_file_name = file_name::parse_rollout_file_name(file_name)?.to_string();
        if path::should_skip_compressed_sibling(path.as_path()) {
            return None;
        }

        Some(Self {
            path,
            plain_file_name,
        })
    }

    /// Returns the physical path that should be opened for reads.
    pub(crate) fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Returns the canonical `.jsonl` filename for timestamp and id parsing.
    pub(crate) fn plain_file_name(&self) -> &str {
        self.plain_file_name.as_str()
    }

    /// Returns whether the physical path is the compressed representation.
    pub(crate) fn is_compressed(&self) -> bool {
        path::is_compressed_rollout_path(self.path.as_path())
    }

    /// Consumes the entry and returns the physical path that should be read.
    pub(crate) fn into_path(self) -> PathBuf {
        self.path
    }
}

/// Line-oriented rollout reader returned by [`open_rollout_line_reader`].
pub struct RolloutLineReader {
    inner: RolloutLineReaderInner,
}

enum RolloutLineReaderInner {
    Plain(tokio::io::Lines<tokio::io::BufReader<tokio::fs::File>>),
    Blocking(Option<BlockingLineReader>),
}

impl RolloutLineReader {
    /// Reads the next JSONL record from the rollout.
    pub async fn next_line(&mut self) -> io::Result<Option<String>> {
        match &mut self.inner {
            RolloutLineReaderInner::Plain(lines) => lines.next_line().await,
            RolloutLineReaderInner::Blocking(slot) => {
                let Some(mut reader) = slot.take() else {
                    return Err(io::Error::other("compressed rollout reader is busy"));
                };
                let (line, reader) =
                    tokio::task::spawn_blocking(move || (reader.next().transpose(), reader))
                        .await
                        .map_err(io::Error::other)?;
                *slot = Some(reader);
                line
            }
        }
    }
}

type BlockingLineReader = std::io::Lines<std::io::BufReader<Box<dyn Read + Send>>>;

mod worker {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::fs::FileTimes;
    use std::fs::Permissions;
    use std::io;
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;
    use std::time::Instant;
    use std::time::SystemTime;

    use tracing::debug;
    use tracing::info;
    use tracing::warn;

    use tokio::task::JoinSet;

    use crate::ARCHIVED_SESSIONS_SUBDIR;
    use crate::SESSIONS_SUBDIR;

    use super::RolloutFile;
    use super::metrics;
    use super::path;

    const TEMP_SUFFIX: &str = ".tmp";
    const COMPRESSION_LEVEL: i32 = 3;
    const MIN_ROLLOUT_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
    const RUN_MARKER_STALE_AFTER: Duration = Duration::from_secs(6 * 60 * 60);
    const TEMP_FILE_STALE_AFTER: Duration = RUN_MARKER_STALE_AFTER;
    const WORKER_MAX_RUNTIME: Duration = Duration::from_secs(5 * 60 * 60);
    const RUN_MARKER_FILE_NAME: &str = "rollout-compression.lock";
    const MAX_CONCURRENT_COMPRESSION_JOBS: usize = 2;

    #[derive(Default)]
    struct CompressionStats {
        scanned: usize,
        compressed: usize,
        skipped: usize,
        failed: usize,
    }

    pub(super) struct CompressionRunMarker {
        path: PathBuf,
        remove_on_drop: bool,
    }

    impl CompressionRunMarker {
        pub(super) fn try_claim(codex_home: &Path) -> io::Result<Option<Self>> {
            let marker_dir = codex_home.join(".tmp");
            std::fs::create_dir_all(marker_dir.as_path())?;
            let path = marker_dir.join(RUN_MARKER_FILE_NAME);
            match create_run_marker_file(path.as_path()) {
                Ok(()) => return Ok(Some(Self::new(path))),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }

            let stale = std::fs::metadata(path.as_path())
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age >= RUN_MARKER_STALE_AFTER);
            if !stale {
                return Ok(None);
            }
            match std::fs::remove_file(path.as_path()) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
            match create_run_marker_file(path.as_path()) {
                Ok(()) => Ok(Some(Self::new(path))),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(None),
                Err(err) => Err(err),
            }
        }

        fn new(path: PathBuf) -> Self {
            Self {
                path,
                remove_on_drop: true,
            }
        }

        pub(super) fn persist(mut self) {
            self.remove_on_drop = false;
        }
    }

    impl Drop for CompressionRunMarker {
        fn drop(&mut self) {
            if self.remove_on_drop {
                let _ = std::fs::remove_file(self.path.as_path());
            }
        }
    }

    pub(super) fn spawn(codex_home: PathBuf) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            metrics::run("skipped_no_runtime");
            warn!(
                "failed to start rollout compression worker for {}: no Tokio runtime",
                codex_home.display()
            );
            return;
        };
        handle.spawn(async move {
            if let Err(err) = run(codex_home.clone()).await {
                warn!(
                    "rollout compression worker failed for {}: {err}",
                    codex_home.display()
                );
            }
        });
    }

    pub(super) async fn run(codex_home: PathBuf) -> io::Result<()> {
        let marker = match CompressionRunMarker::try_claim(codex_home.as_path()) {
            Ok(Some(marker)) => marker,
            Ok(None) => {
                metrics::run("skipped_already_running");
                debug!(
                    "rollout compression worker recently ran or is already running for {}",
                    codex_home.display()
                );
                return Ok(());
            }
            Err(err) => {
                metrics::run("failed");
                return Err(err);
            }
        };

        metrics::run("started");
        let started_at = Instant::now();
        let result = async {
            cleanup_stale_temps(codex_home.as_path()).await?;
            let mut stats = CompressionStats::default();
            if started_at.elapsed() < WORKER_MAX_RUNTIME {
                let archived_root = codex_home.join(ARCHIVED_SESSIONS_SUBDIR);
                compress_rollouts_in_root(archived_root.as_path(), started_at, &mut stats).await?;
            }
            Ok::<_, io::Error>(stats)
        }
        .await;
        let stats = match result {
            Ok(stats) => stats,
            Err(err) => {
                metrics::run("failed");
                metrics::run_duration("failed", started_at.elapsed());
                return Err(err);
            }
        };
        info!(
            "rollout compression worker finished: scanned={}, compressed={}, skipped={}, failed={}",
            stats.scanned, stats.compressed, stats.skipped, stats.failed
        );
        metrics::run("completed");
        metrics::run_duration("completed", started_at.elapsed());
        marker.persist();
        Ok(())
    }

    fn create_run_marker_file(path: &Path) -> io::Result<()> {
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
        let mut jobs = JoinSet::new();
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
            loop {
                let entry = match read_dir.next_entry().await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => break,
                    Err(err) => {
                        drain_compression_jobs(&mut jobs, stats).await;
                        return Err(err);
                    }
                };
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
                let Some(rollout_file) = RolloutFile::from_path(path) else {
                    continue;
                };
                if rollout_file.is_compressed() {
                    continue;
                }
                let path = rollout_file.into_path();
                stats.scanned = stats.scanned.saturating_add(1);
                metrics::file("scanned");
                while jobs.len() >= MAX_CONCURRENT_COMPRESSION_JOBS {
                    collect_next_compression_job(&mut jobs, stats).await;
                }
                jobs.spawn_blocking(move || {
                    let started_at = Instant::now();
                    let result = compress_rollout_if_cold_blocking(path.as_path());
                    let duration = started_at.elapsed();
                    (path, duration, result)
                });
            }
        }
        drain_compression_jobs(&mut jobs, stats).await;
        Ok(())
    }

    type CompressionJobResult = (PathBuf, Duration, io::Result<CompressionMeasurement>);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CompressionOutcome {
        Compressed,
        SkippedNotCold,
        SkippedChanged,
        SkippedAlreadyCompressed,
    }

    impl CompressionOutcome {
        fn tag(self) -> &'static str {
            match self {
                CompressionOutcome::Compressed => "compressed",
                CompressionOutcome::SkippedNotCold => "skipped_not_cold",
                CompressionOutcome::SkippedChanged => "skipped_changed",
                CompressionOutcome::SkippedAlreadyCompressed => "skipped_already_compressed",
            }
        }
    }

    struct CompressionMeasurement {
        outcome: CompressionOutcome,
        source_bytes: Option<u64>,
        compressed_bytes: Option<u64>,
    }

    impl CompressionMeasurement {
        fn new(
            outcome: CompressionOutcome,
            source_bytes: Option<u64>,
            compressed_bytes: Option<u64>,
        ) -> Self {
            Self {
                outcome,
                source_bytes,
                compressed_bytes,
            }
        }
    }

    enum ColdFileState {
        Cold(FileState),
        NotCold(Option<FileState>),
    }

    async fn drain_compression_jobs(
        jobs: &mut JoinSet<CompressionJobResult>,
        stats: &mut CompressionStats,
    ) {
        while !jobs.is_empty() {
            collect_next_compression_job(jobs, stats).await;
        }
    }

    async fn collect_next_compression_job(
        jobs: &mut JoinSet<CompressionJobResult>,
        stats: &mut CompressionStats,
    ) {
        let Some(result) = jobs.join_next().await else {
            return;
        };
        match result {
            Ok((_, duration, Ok(measurement))) => {
                let outcome = measurement.outcome;
                match outcome {
                    CompressionOutcome::Compressed => {
                        stats.compressed = stats.compressed.saturating_add(1);
                    }
                    CompressionOutcome::SkippedNotCold
                    | CompressionOutcome::SkippedChanged
                    | CompressionOutcome::SkippedAlreadyCompressed => {
                        stats.skipped = stats.skipped.saturating_add(1);
                    }
                }
                metrics::file(outcome.tag());
                metrics::file_duration(outcome.tag(), duration);
                if let Some(source_bytes) = measurement.source_bytes {
                    metrics::source_bytes(outcome.tag(), source_bytes);
                }
                if let Some(compressed_bytes) = measurement.compressed_bytes {
                    metrics::compressed_bytes(outcome.tag(), compressed_bytes);
                    if let Some(source_bytes) = measurement.source_bytes {
                        metrics::compression_ratio(outcome.tag(), source_bytes, compressed_bytes);
                    }
                }
            }
            Ok((path, duration, Err(err))) => {
                stats.failed = stats.failed.saturating_add(1);
                metrics::file("failed");
                metrics::file_duration("failed", duration);
                warn!("failed to compress rollout {}: {err}", path.display());
            }
            Err(err) => {
                stats.failed = stats.failed.saturating_add(1);
                metrics::file("failed");
                warn!("rollout compression task failed: {err}");
            }
        }
    }

    fn compress_rollout_if_cold_blocking(path: &Path) -> io::Result<CompressionMeasurement> {
        let before = match cold_file_state(path)? {
            ColdFileState::Cold(state) => state,
            ColdFileState::NotCold(state) => {
                return Ok(CompressionMeasurement::new(
                    CompressionOutcome::SkippedNotCold,
                    state.map(|state| state.len),
                    None,
                ));
            }
        };
        let source_bytes = Some(before.len);
        let compressed_path = path::compressed_rollout_path(path);
        if compressed_path.exists() {
            return Ok(CompressionMeasurement::new(
                CompressionOutcome::SkippedAlreadyCompressed,
                source_bytes,
                None,
            ));
        }

        let temp_dir = compressed_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(temp_dir)?;
        let mut temp_file = tempfile::Builder::new()
            .prefix("rollout-compress-")
            .suffix(TEMP_SUFFIX)
            .tempfile_in(temp_dir)?;
        encode_zstd_to_writer(path, temp_file.as_file_mut())?;
        temp_file.as_file_mut().flush()?;
        verify_zstd(temp_file.path())?;
        if !same_file_state(path, &before)? {
            return Ok(CompressionMeasurement::new(
                CompressionOutcome::SkippedChanged,
                source_bytes,
                None,
            ));
        }
        set_file_metadata(temp_file.as_file(), before.modified, &before.permissions)?;
        temp_file.as_file().sync_all()?;
        let compressed_bytes = temp_file.as_file().metadata()?.len();

        match temp_file.persist_noclobber(compressed_path.as_path()) {
            Ok(_) => {}
            Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => {
                return Ok(CompressionMeasurement::new(
                    CompressionOutcome::SkippedAlreadyCompressed,
                    source_bytes,
                    None,
                ));
            }
            Err(err) => return Err(err.error),
        }
        if !same_file_state(path, &before)? {
            let _ = std::fs::remove_file(compressed_path.as_path());
            return Ok(CompressionMeasurement::new(
                CompressionOutcome::SkippedChanged,
                source_bytes,
                None,
            ));
        }
        std::fs::remove_file(path)?;
        Ok(CompressionMeasurement::new(
            CompressionOutcome::Compressed,
            source_bytes,
            Some(compressed_bytes),
        ))
    }

    struct FileState {
        len: u64,
        modified: SystemTime,
        permissions: Permissions,
    }

    fn cold_file_state(path: &Path) -> io::Result<ColdFileState> {
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(ColdFileState::NotCold(None));
            }
            Err(err) => return Err(err),
        };
        if !metadata.is_file() {
            return Ok(ColdFileState::NotCold(None));
        }
        let modified = metadata.modified()?;
        let state = FileState {
            len: metadata.len(),
            modified,
            permissions: metadata.permissions(),
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        if age < MIN_ROLLOUT_AGE {
            return Ok(ColdFileState::NotCold(Some(state)));
        }
        Ok(ColdFileState::Cold(state))
    }

    fn same_file_state(path: &Path, expected: &FileState) -> io::Result<bool> {
        match std::fs::metadata(path) {
            Ok(metadata) => Ok(metadata.len() == expected.len
                && metadata.modified()? == expected.modified
                && metadata.permissions() == expected.permissions),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn encode_zstd_to_writer(source: &Path, output: impl Write) -> io::Result<()> {
        let mut input = File::open(source)?;
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

    fn set_file_metadata(
        file: &File,
        modified: SystemTime,
        permissions: &Permissions,
    ) -> io::Result<()> {
        file.set_times(FileTimes::new().set_modified(modified))?;
        file.set_permissions(permissions.clone())
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
                    let stale = entry
                        .metadata()
                        .await
                        .ok()
                        .and_then(|metadata| metadata.modified().ok())
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age >= TEMP_FILE_STALE_AFTER);
                    if !stale {
                        continue;
                    }
                    match tokio::fs::remove_file(path.as_path()).await {
                        Ok(()) => metrics::temp_cleanup("removed"),
                        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                        Err(err) => {
                            metrics::temp_cleanup("failed");
                            warn!(
                                "failed to remove stale rollout temp {}: {err}",
                                path.display()
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

mod metrics {
    use std::time::Duration;

    const FILE_COMPRESSED_BYTES_HISTOGRAM: &str = "codex.rollout_compression.file.compressed_bytes";
    const FILE_COUNTER: &str = "codex.rollout_compression.file";
    const FILE_DURATION_HISTOGRAM: &str = "codex.rollout_compression.file.duration_ms";
    const FILE_SOURCE_BYTES_HISTOGRAM: &str = "codex.rollout_compression.file.source_bytes";
    const FILE_COMPRESSION_RATIO_HISTOGRAM: &str =
        "codex.rollout_compression.file.compression_ratio";
    const MATERIALIZE_COUNTER: &str = "codex.rollout_compression.materialize";
    const RUN_COUNTER: &str = "codex.rollout_compression.run";
    const RUN_DURATION_HISTOGRAM: &str = "codex.rollout_compression.run.duration_ms";
    const RATIO_BASIS_POINTS: u128 = 10_000;
    const TEMP_CLEANUP_COUNTER: &str = "codex.rollout_compression.temp_cleanup";

    pub(super) fn file(outcome: &'static str) {
        counter(FILE_COUNTER, &[("outcome", outcome)]);
    }

    pub(super) fn file_duration(outcome: &'static str, duration: Duration) {
        duration_histogram(FILE_DURATION_HISTOGRAM, duration, &[("outcome", outcome)]);
    }

    pub(super) fn source_bytes(outcome: &'static str, bytes: u64) {
        histogram(
            FILE_SOURCE_BYTES_HISTOGRAM,
            saturating_i64(bytes),
            &[("outcome", outcome)],
        );
    }

    pub(super) fn compressed_bytes(outcome: &'static str, bytes: u64) {
        histogram(
            FILE_COMPRESSED_BYTES_HISTOGRAM,
            saturating_i64(bytes),
            &[("outcome", outcome)],
        );
    }

    pub(super) fn compression_ratio(
        outcome: &'static str,
        source_bytes: u64,
        compressed_bytes: u64,
    ) {
        if source_bytes == 0 {
            return;
        }
        // Keep the ratio histogram integer-valued while preserving sub-percent precision.
        let ratio = (u128::from(compressed_bytes) * RATIO_BASIS_POINTS) / u128::from(source_bytes);
        histogram(
            FILE_COMPRESSION_RATIO_HISTOGRAM,
            saturating_i64(ratio),
            &[("outcome", outcome)],
        );
    }

    pub(super) fn materialize(outcome: &'static str) {
        counter(MATERIALIZE_COUNTER, &[("outcome", outcome)]);
    }

    pub(super) fn run(status: &'static str) {
        counter(RUN_COUNTER, &[("status", status)]);
    }

    pub(super) fn run_duration(status: &'static str, duration: Duration) {
        duration_histogram(RUN_DURATION_HISTOGRAM, duration, &[("status", status)]);
    }

    pub(super) fn temp_cleanup(outcome: &'static str) {
        counter(TEMP_CLEANUP_COUNTER, &[("outcome", outcome)]);
    }

    fn counter(name: &str, tags: &[(&str, &str)]) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let _ = metrics.counter(name, /*inc*/ 1, tags);
    }

    fn histogram(name: &str, value: i64, tags: &[(&str, &str)]) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let _ = metrics.histogram(name, value, tags);
    }

    fn duration_histogram(name: &str, duration: Duration, tags: &[(&str, &str)]) {
        let Some(metrics) = codex_otel::global() else {
            return;
        };
        let _ = metrics.record_duration(name, duration, tags);
    }

    fn saturating_i64(value: impl TryInto<i64>) -> i64 {
        value.try_into().unwrap_or(i64::MAX)
    }
}

/// Returns the existing rollout path, preferring the plain `.jsonl` file over
/// its `.jsonl.zst` compressed sibling.
pub async fn existing_rollout_path(path: &Path) -> Option<PathBuf> {
    path::existing_rollout_path(path).await
}

mod path {
    use std::ffi::OsStr;
    use std::path::Path;
    use std::path::PathBuf;

    use super::COMPRESSED_SUFFIX;

    pub(super) fn compressed_rollout_path(path: &Path) -> PathBuf {
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

    pub(super) fn plain_rollout_path(path: &Path) -> PathBuf {
        let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
            return path.to_path_buf();
        };
        let Some(plain_file_name) = file_name.strip_suffix(COMPRESSED_SUFFIX) else {
            return path.to_path_buf();
        };
        path.with_file_name(plain_file_name)
    }

    pub(super) fn is_compressed_rollout_path(path: &Path) -> bool {
        path.file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.ends_with(".jsonl.zst"))
    }

    pub(super) fn should_skip_compressed_sibling(path: &Path) -> bool {
        is_compressed_rollout_path(path) && plain_rollout_path(path).exists()
    }

    pub(super) async fn existing_rollout_path(path: &Path) -> Option<PathBuf> {
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
}

mod file_name {
    use super::COMPRESSED_SUFFIX;

    pub(super) fn parse_rollout_file_name(name: &str) -> Option<&str> {
        let name = name.strip_suffix(COMPRESSED_SUFFIX).unwrap_or(name);
        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            Some(name)
        } else {
            None
        }
    }
}

mod reader {
    use std::fs::File;
    use std::io;
    use std::io::BufRead;
    use std::io::Read;
    use std::path::Path;

    use super::RolloutLineReader;
    use super::RolloutLineReaderInner;
    use super::path;
    use tokio::io::AsyncBufReadExt;

    pub(super) async fn open_once(path: &Path) -> io::Result<RolloutLineReader> {
        let path = path::existing_rollout_path(path)
            .await
            .unwrap_or_else(|| path.to_path_buf());
        if path::is_compressed_rollout_path(path.as_path()) {
            let reader = tokio::task::spawn_blocking(move || {
                let input = File::open(path.as_path())?;
                let decoder = zstd::stream::read::Decoder::new(input)?;
                Ok::<_, io::Error>(
                    io::BufReader::new(Box::new(decoder) as Box<dyn Read + Send>).lines(),
                )
            })
            .await
            .map_err(io::Error::other)??;
            return Ok(RolloutLineReader {
                inner: RolloutLineReaderInner::Blocking(Some(reader)),
            });
        }
        let file = tokio::fs::File::open(path).await?;
        Ok(RolloutLineReader {
            inner: RolloutLineReaderInner::Plain(tokio::io::BufReader::new(file).lines()),
        })
    }
}

#[cfg(unix)]
fn create_file_with_permissions(path: &Path, permissions: &Permissions) -> io::Result<File> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(permissions.mode() & 0o7777)
        .open(path)?;
    file.set_permissions(permissions.clone())?;
    Ok(file)
}

#[cfg(not(unix))]
fn create_file_with_permissions(path: &Path, permissions: &Permissions) -> io::Result<File> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.set_permissions(permissions.clone())?;
    Ok(file)
}

fn temp_path_for(path: &Path, operation: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsStr::new("rollout").to_os_string());
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    file_name.push(format!(
        ".{operation}.{}.{counter}{TEMP_SUFFIX}",
        std::process::id()
    ));
    path.with_file_name(file_name)
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
