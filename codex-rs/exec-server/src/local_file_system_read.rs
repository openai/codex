//! Bounded blocking file reads that stop between chunks when their async caller is dropped.

use crate::FILE_READ_CHUNK_SIZE;
use crate::ReadFileOptions;
use crate::no_follow;
use crate::protocol::FsOpenMode;
use crate::regular_file;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
use std::io::Read;
use tokio_util::sync::CancellationToken;

const MAX_READ_FILE_BYTES: u64 = 512 * 1024 * 1024;

pub(super) async fn read_file(
    path: AbsolutePathBuf,
    options: ReadFileOptions,
) -> io::Result<Vec<u8>> {
    let cancelled = CancellationToken::new();
    let _cancel_on_drop = cancelled.clone().drop_guard();
    tokio::task::spawn_blocking(move || {
        if cancelled.is_cancelled() {
            return Err(io::Error::other("file read cancelled"));
        }
        let file = if options.follow_symlinks {
            regular_file::open_sync(path.as_path(), FsOpenMode::Read)?
        } else {
            no_follow::open_file(path.as_path())?
        };
        let metadata = file.metadata()?;
        if metadata.len() > MAX_READ_FILE_BYTES {
            return Err(file_too_large_error());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        CancellableReader {
            reader: file.take(MAX_READ_FILE_BYTES + 1),
            cancelled,
        }
        .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_READ_FILE_BYTES {
            return Err(file_too_large_error());
        }
        Ok(bytes)
    })
    .await
    .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

struct CancellableReader<R> {
    reader: R,
    cancelled: CancellationToken,
}

impl<R: Read> Read for CancellableReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.cancelled.is_cancelled() {
            // read_to_end retries Interrupted errors, so cancellation must use another kind.
            return Err(io::Error::other("file read cancelled"));
        }
        let count = bytes.len().min(FILE_READ_CHUNK_SIZE);
        self.reader.read(&mut bytes[..count])
    }
}

fn file_too_large_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("file is too large to read: limit is {MAX_READ_FILE_BYTES} bytes"),
    )
}

#[cfg(test)]
#[path = "local_file_system_read_tests.rs"]
mod tests;
