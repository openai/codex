use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const COMPRESSED_SUFFIX: &str = ".zst";
const MAX_NOT_FOUND_RETRIES: usize = 3;
const OPEN_ROLLOUT_LINE_READER_RETRY_DELAY: Duration = Duration::from_millis(50);

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
pub(crate) fn compressed_rollout_path(path: &Path) -> PathBuf {
    path::compressed_rollout_path(path)
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

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
