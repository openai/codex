//! Connection-scoped file handles with bounded positional reads.
//! Semaphore permits bound open handles and in-flight opens; table locks never cross an await.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use codex_file_system::FILE_READ_CHUNK_SIZE;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

const MAX_OPEN_FILES: usize = 128;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileReadBlock {
    pub(crate) bytes: Vec<u8>,
    pub(crate) eof: bool,
}

#[derive(Clone)]
pub(crate) struct FileHandleManager {
    handles: Arc<Mutex<HashMap<String, FileHandleEntry>>>,
    slots: Arc<Semaphore>,
}

impl Default for FileHandleManager {
    fn default() -> Self {
        Self {
            handles: Arc::default(),
            slots: Arc::new(Semaphore::new(MAX_OPEN_FILES)),
        }
    }
}

impl FileHandleManager {
    pub(crate) async fn open(
        &self,
        handle_id: String,
        open_file: impl Future<Output = io::Result<tokio::fs::File>>,
    ) -> io::Result<String> {
        if self.lock_handles().contains_key(&handle_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("file read handle `{handle_id}` already exists"),
            ));
        }
        let permit = Arc::clone(&self.slots).try_acquire_owned().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("at most {MAX_OPEN_FILES} file reads may be open per connection"),
            )
        })?;
        let file = Arc::new(open_file.await?.into_std().await);
        let mut handles = self.lock_handles();
        let Entry::Vacant(entry) = handles.entry(handle_id.clone()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("file read handle `{handle_id}` already exists"),
            ));
        };
        entry.insert(FileHandleEntry {
            file,
            _permit: permit,
        });
        Ok(handle_id)
    }

    pub(crate) async fn read_block(
        &self,
        handle_id: &str,
        offset: u64,
        len: usize,
    ) -> io::Result<FileReadBlock> {
        validate_read_block_len(len)?;
        let file = self.get(handle_id)?;
        let result =
            match tokio::task::spawn_blocking(move || read_block_at(&file, offset, len)).await {
                Ok(result) => result,
                Err(error) => Err(io::Error::other(format!(
                    "file read task stopped unexpectedly: {error}"
                ))),
            };
        if result.is_err() {
            self.close(handle_id);
        }
        result
    }

    fn get(&self, handle_id: &str) -> io::Result<Arc<File>> {
        self.lock_handles()
            .get(handle_id)
            .map(|entry| Arc::clone(&entry.file))
            .ok_or_else(|| unknown_handle_error(handle_id))
    }

    pub(crate) fn close(&self, handle_id: &str) {
        self.lock_handles().remove(handle_id);
    }

    pub(crate) fn close_all(&self) {
        self.lock_handles().clear();
    }

    fn lock_handles(&self) -> MutexGuard<'_, HashMap<String, FileHandleEntry>> {
        self.handles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

struct FileHandleEntry {
    file: Arc<File>,
    // Closing an entry releases capacity even if a read still holds the file.
    _permit: OwnedSemaphorePermit,
}

fn read_block_at(file: &File, offset: u64, len: usize) -> io::Result<FileReadBlock> {
    let mut bytes = vec![0; len];
    let mut bytes_read = 0;
    while bytes_read < len {
        let read_offset = offset.checked_add(bytes_read as u64).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "file read offset overflowed")
        })?;
        match read_file_at(file, &mut bytes[bytes_read..], read_offset) {
            Ok(0) => break,
            Ok(read) => bytes_read += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    bytes.truncate(bytes_read);
    Ok(FileReadBlock {
        eof: bytes_read < len,
        bytes,
    })
}

#[cfg(unix)]
fn read_file_at(file: &File, bytes: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, bytes, offset)
}

#[cfg(windows)]
fn read_file_at(file: &File, bytes: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, bytes, offset)
}

fn validate_read_block_len(len: usize) -> io::Result<()> {
    if !(1..=FILE_READ_CHUNK_SIZE).contains(&len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("file read block length must be between 1 and {FILE_READ_CHUNK_SIZE}"),
        ));
    }
    Ok(())
}

fn unknown_handle_error(handle_id: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("unknown file read handle `{handle_id}`"),
    )
}

#[cfg(test)]
#[path = "file_handle_tests.rs"]
mod tests;
