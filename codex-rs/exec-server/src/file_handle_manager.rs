use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use codex_file_system::ExecutorFileSystem;
use codex_file_system::FileReadHandle;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::FileWriteCommitResult;
use codex_file_system::FileWriteHandle;
use codex_file_system::OpenFileMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileReadChunk {
    pub data: Vec<u8>,
    pub eof: bool,
}

#[derive(Clone, Default)]
pub struct FileHandleManager {
    handles: Arc<Mutex<HashMap<String, FileHandle>>>,
}

enum FileHandle {
    Opening(Arc<OpeningHandleEntry>),
    Read(Arc<ReadHandleEntry>),
    Write(Arc<WriteHandleEntry>),
}

struct OpeningHandleEntry {
    cancellation: CancellationToken,
}

struct ReadHandleEntry {
    handle: Box<dyn FileReadHandle>,
    cancellation: CancellationToken,
    max_chunk_bytes: usize,
}

struct WriteHandleEntry {
    handle: Box<dyn FileWriteHandle>,
    cancellation: CancellationToken,
    max_chunk_bytes: usize,
}

impl FileHandleManager {
    pub async fn open_read(
        &self,
        file_system: Arc<dyn ExecutorFileSystem>,
        handle_id: String,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<usize> {
        let opening = self.reserve_opening(&handle_id).await?;
        let handle = match file_system.open_file_for_read(path, sandbox).await {
            Ok(handle) => handle,
            Err(err) => {
                self.remove_if_opening(&handle_id, &opening).await;
                return if opening.cancellation.is_cancelled() {
                    Err(cancelled_handle_error())
                } else {
                    Err(err)
                };
            }
        };
        let max_chunk_bytes = handle.max_chunk_bytes();
        let entry = FileHandle::Read(Arc::new(ReadHandleEntry {
            handle,
            cancellation: opening.cancellation.clone(),
            max_chunk_bytes,
        }));
        self.promote_opening(handle_id, &opening, entry).await?;
        Ok(max_chunk_bytes)
    }

    pub async fn read(
        &self,
        handle_id: &str,
        offset: u64,
        max_bytes: Option<usize>,
    ) -> io::Result<FileReadChunk> {
        let entry = self.read_entry(handle_id).await?;
        let max_bytes = max_bytes
            .unwrap_or(entry.max_chunk_bytes)
            .min(entry.max_chunk_bytes);
        if max_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file read max bytes must be greater than zero",
            ));
        }
        let result = tokio::select! {
            _ = entry.cancellation.cancelled() => Err(cancelled_handle_error()),
            result = entry.handle.read(offset, max_bytes) => result,
        };
        match result {
            Ok(data) => {
                let eof = if data.len() < max_bytes {
                    true
                } else {
                    let metadata = tokio::select! {
                        _ = entry.cancellation.cancelled() => Err(cancelled_handle_error()),
                        result = entry.handle.metadata() => result,
                    };
                    match metadata {
                        Ok(metadata) => {
                            offset.saturating_add(data.len() as u64) >= metadata.size_bytes
                        }
                        Err(err) => {
                            self.remove_if_read(handle_id, &entry).await;
                            return Err(err);
                        }
                    }
                };
                Ok(FileReadChunk { data, eof })
            }
            Err(err) => {
                self.remove_if_read(handle_id, &entry).await;
                Err(err)
            }
        }
    }

    pub async fn stat_read(&self, handle_id: &str) -> io::Result<OpenFileMetadata> {
        let entry = self.read_entry(handle_id).await?;
        let result = tokio::select! {
            _ = entry.cancellation.cancelled() => Err(cancelled_handle_error()),
            result = entry.handle.metadata() => result,
        };
        if result.is_err() {
            self.remove_if_read(handle_id, &entry).await;
        }
        result
    }

    pub async fn open_write(
        &self,
        file_system: Arc<dyn ExecutorFileSystem>,
        handle_id: String,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<usize> {
        let opening = self.reserve_opening(&handle_id).await?;
        let handle = match file_system.open_file_for_write(path, sandbox).await {
            Ok(handle) => handle,
            Err(err) => {
                self.remove_if_opening(&handle_id, &opening).await;
                return if opening.cancellation.is_cancelled() {
                    Err(cancelled_handle_error())
                } else {
                    Err(err)
                };
            }
        };
        let max_chunk_bytes = handle.max_chunk_bytes();
        let entry = FileHandle::Write(Arc::new(WriteHandleEntry {
            handle,
            cancellation: opening.cancellation.clone(),
            max_chunk_bytes,
        }));
        self.promote_opening(handle_id, &opening, entry).await?;
        Ok(max_chunk_bytes)
    }

    pub async fn write(&self, handle_id: &str, data: &[u8]) -> io::Result<()> {
        let entry = self.write_entry(handle_id).await?;
        if data.len() > entry.max_chunk_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "file write chunk exceeds maximum of {} bytes",
                    entry.max_chunk_bytes
                ),
            ));
        }
        let result = tokio::select! {
            _ = entry.cancellation.cancelled() => Err(cancelled_handle_error()),
            result = entry.handle.write(data) => result,
        };
        if result.is_err() {
            self.remove_if_write(handle_id, &entry).await;
        }
        result
    }

    pub async fn commit_write(&self, handle_id: &str) -> io::Result<FileWriteCommitResult> {
        let entry = self.write_entry(handle_id).await?;
        let result = entry.handle.commit().await;
        self.remove_if_write(handle_id, &entry).await;
        result
    }

    pub async fn close(&self, handle_id: &str) {
        let handle = self.handles.lock().await.remove(handle_id);
        match handle {
            Some(FileHandle::Opening(entry)) => entry.cancellation.cancel(),
            Some(FileHandle::Read(entry)) => entry.cancellation.cancel(),
            Some(FileHandle::Write(entry)) => entry.cancellation.cancel(),
            None => {}
        }
    }

    pub async fn close_all(&self) {
        let handles = std::mem::take(&mut *self.handles.lock().await);
        for handle in handles.into_values() {
            match handle {
                FileHandle::Opening(entry) => entry.cancellation.cancel(),
                FileHandle::Read(entry) => entry.cancellation.cancel(),
                FileHandle::Write(entry) => entry.cancellation.cancel(),
            }
        }
    }

    async fn reserve_opening(&self, handle_id: &str) -> io::Result<Arc<OpeningHandleEntry>> {
        if handle_id.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file handle id cannot be empty",
            ));
        }
        let mut handles = self.handles.lock().await;
        if handles.contains_key(handle_id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("file handle `{handle_id}` is already open"),
            ));
        }
        let opening = Arc::new(OpeningHandleEntry {
            cancellation: CancellationToken::new(),
        });
        handles.insert(
            handle_id.to_string(),
            FileHandle::Opening(Arc::clone(&opening)),
        );
        Ok(opening)
    }

    async fn promote_opening(
        &self,
        handle_id: String,
        opening: &Arc<OpeningHandleEntry>,
        handle: FileHandle,
    ) -> io::Result<()> {
        let mut handles = self.handles.lock().await;
        let owns_reservation = matches!(
            handles.get(&handle_id),
            Some(FileHandle::Opening(current)) if Arc::ptr_eq(current, opening)
        );
        if opening.cancellation.is_cancelled() || !owns_reservation {
            if owns_reservation {
                handles.remove(&handle_id);
            }
            return Err(cancelled_handle_error());
        }
        handles.insert(handle_id, handle);
        Ok(())
    }

    async fn read_entry(&self, handle_id: &str) -> io::Result<Arc<ReadHandleEntry>> {
        match self.handles.lock().await.get(handle_id) {
            Some(FileHandle::Read(entry)) => Ok(Arc::clone(entry)),
            Some(FileHandle::Write(_)) => Err(wrong_handle_type_error(handle_id, "read")),
            Some(FileHandle::Opening(_)) => Err(opening_handle_error(handle_id)),
            None => Err(unknown_handle_error(handle_id)),
        }
    }

    async fn write_entry(&self, handle_id: &str) -> io::Result<Arc<WriteHandleEntry>> {
        match self.handles.lock().await.get(handle_id) {
            Some(FileHandle::Write(entry)) => Ok(Arc::clone(entry)),
            Some(FileHandle::Read(_)) => Err(wrong_handle_type_error(handle_id, "write")),
            Some(FileHandle::Opening(_)) => Err(opening_handle_error(handle_id)),
            None => Err(unknown_handle_error(handle_id)),
        }
    }

    async fn remove_if_opening(&self, handle_id: &str, expected: &Arc<OpeningHandleEntry>) {
        let mut handles = self.handles.lock().await;
        if matches!(
            handles.get(handle_id),
            Some(FileHandle::Opening(entry)) if Arc::ptr_eq(entry, expected)
        ) {
            handles.remove(handle_id);
        }
    }

    async fn remove_if_read(&self, handle_id: &str, expected: &Arc<ReadHandleEntry>) {
        let mut handles = self.handles.lock().await;
        if matches!(
            handles.get(handle_id),
            Some(FileHandle::Read(entry)) if Arc::ptr_eq(entry, expected)
        ) {
            handles.remove(handle_id);
            expected.cancellation.cancel();
        }
    }

    async fn remove_if_write(&self, handle_id: &str, expected: &Arc<WriteHandleEntry>) {
        let mut handles = self.handles.lock().await;
        if matches!(
            handles.get(handle_id),
            Some(FileHandle::Write(entry)) if Arc::ptr_eq(entry, expected)
        ) {
            handles.remove(handle_id);
            expected.cancellation.cancel();
        }
    }
}

fn unknown_handle_error(handle_id: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("unknown file handle `{handle_id}`"),
    )
}

fn wrong_handle_type_error(handle_id: &str, expected: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("file handle `{handle_id}` is not open for {expected}"),
    )
}

fn opening_handle_error(handle_id: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        format!("file handle `{handle_id}` is still opening"),
    )
}

fn cancelled_handle_error() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "file operation was cancelled")
}

#[cfg(test)]
#[path = "file_handle_manager_tests.rs"]
mod tests;
