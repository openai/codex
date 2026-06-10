use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use codex_file_system::ExecutorFileSystem;
use codex_file_system::FileReadChunk;
use codex_file_system::FileReadHandle;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::FileWriteHandle;
use codex_file_system::OpenFileMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

pub const MAX_OPEN_FILE_HANDLES: usize = 64;

#[derive(Clone)]
pub struct FileHandleManager {
    handles: Arc<Mutex<HashMap<String, FileHandle>>>,
    max_handles: usize,
}

impl Default for FileHandleManager {
    fn default() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            max_handles: MAX_OPEN_FILE_HANDLES,
        }
    }
}

#[derive(Clone)]
enum FileHandle {
    Opening(Arc<OpeningHandleEntry>),
    Read(Arc<ReadHandleEntry>),
    Write(Arc<WriteHandleEntry>),
}

struct OpeningHandleEntry {
    cancellation: CancellationToken,
    completed: CancellationToken,
}

struct ReadHandleEntry {
    handle: Box<dyn FileReadHandle>,
    cancellation: CancellationToken,
    operation: Semaphore,
    max_chunk_bytes: usize,
}

struct WriteHandleEntry {
    handle: Box<dyn FileWriteHandle>,
    cancellation: CancellationToken,
    operation: Semaphore,
    max_chunk_bytes: usize,
}

impl FileHandleManager {
    #[cfg(test)]
    fn with_max_handles(max_handles: usize) -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            max_handles,
        }
    }

    pub async fn open_read(
        &self,
        file_system: Arc<dyn ExecutorFileSystem>,
        handle_id: String,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<usize> {
        let opening = self.reserve_opening(&handle_id).await?;
        let _completion = OpeningCompletionGuard(opening.completed.clone());
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
            operation: Semaphore::new(1),
            max_chunk_bytes,
        }));
        match self.promote_opening(handle_id, &opening, entry).await {
            Ok(()) => Ok(max_chunk_bytes),
            Err(handle) => {
                let _ = close_file_handle(handle).await;
                Err(cancelled_handle_error())
            }
        }
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
        let operation = acquire_operation(&entry.operation).await?;
        if entry.cancellation.is_cancelled() {
            return Err(cancelled_handle_error());
        }
        let result = entry.handle.read(offset, max_bytes).await;
        drop(operation);
        match result {
            Ok(chunk) => Ok(chunk),
            Err(err) => {
                self.remove_and_close_read(handle_id, &entry).await;
                Err(err)
            }
        }
    }

    pub async fn stat_read(&self, handle_id: &str) -> io::Result<OpenFileMetadata> {
        let entry = self.read_entry(handle_id).await?;
        let operation = acquire_operation(&entry.operation).await?;
        if entry.cancellation.is_cancelled() {
            return Err(cancelled_handle_error());
        }
        let result = entry.handle.metadata().await;
        drop(operation);
        if result.is_err() {
            self.remove_and_close_read(handle_id, &entry).await;
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
        let _completion = OpeningCompletionGuard(opening.completed.clone());
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
            operation: Semaphore::new(1),
            max_chunk_bytes,
        }));
        match self.promote_opening(handle_id, &opening, entry).await {
            Ok(()) => Ok(max_chunk_bytes),
            Err(handle) => {
                let _ = close_file_handle(handle).await;
                Err(cancelled_handle_error())
            }
        }
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
        let operation = acquire_operation(&entry.operation).await?;
        if entry.cancellation.is_cancelled() {
            return Err(cancelled_handle_error());
        }
        let result = entry.handle.write(data).await;
        drop(operation);
        if result.is_err() {
            self.remove_and_close_write(handle_id, &entry).await;
        }
        result
    }

    pub async fn close(&self, handle_id: &str) -> io::Result<()> {
        let handle = self.handles.lock().await.get(handle_id).cloned();
        let Some(handle) = handle else {
            return Ok(());
        };
        let result = close_file_handle(handle.clone()).await;
        self.remove_if_handle(handle_id, &handle).await;
        result
    }

    pub async fn close_all(&self) {
        let handles = self.handles.lock().await.clone();
        for (handle_id, handle) in handles {
            let _ = close_file_handle(handle.clone()).await;
            self.remove_if_handle(&handle_id, &handle).await;
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
        if handles.len() >= self.max_handles {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("open file handle limit of {} reached", self.max_handles),
            ));
        }
        let opening = Arc::new(OpeningHandleEntry {
            cancellation: CancellationToken::new(),
            completed: CancellationToken::new(),
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
    ) -> Result<(), FileHandle> {
        let mut handles = self.handles.lock().await;
        let owns_reservation = matches!(
            handles.get(&handle_id),
            Some(FileHandle::Opening(current)) if Arc::ptr_eq(current, opening)
        );
        if opening.cancellation.is_cancelled() || !owns_reservation {
            return Err(handle);
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

    async fn remove_if_handle(&self, handle_id: &str, expected: &FileHandle) {
        let mut handles = self.handles.lock().await;
        if handles
            .get(handle_id)
            .is_some_and(|current| current.ptr_eq(expected))
        {
            handles.remove(handle_id);
        }
    }

    async fn remove_and_close_read(&self, handle_id: &str, expected: &Arc<ReadHandleEntry>) {
        let handle = FileHandle::Read(Arc::clone(expected));
        let owns_handle = self
            .handles
            .lock()
            .await
            .get(handle_id)
            .is_some_and(|current| current.ptr_eq(&handle));
        if owns_handle {
            expected.cancellation.cancel();
            let Ok(operation) = acquire_operation(&expected.operation).await else {
                return;
            };
            let _ = expected.handle.close().await;
            drop(operation);
            self.remove_if_handle(handle_id, &handle).await;
        }
    }

    async fn remove_and_close_write(&self, handle_id: &str, expected: &Arc<WriteHandleEntry>) {
        let handle = FileHandle::Write(Arc::clone(expected));
        let owns_handle = self
            .handles
            .lock()
            .await
            .get(handle_id)
            .is_some_and(|current| current.ptr_eq(&handle));
        if owns_handle {
            expected.cancellation.cancel();
            let Ok(operation) = acquire_operation(&expected.operation).await else {
                return;
            };
            let _ = expected.handle.close().await;
            drop(operation);
            self.remove_if_handle(handle_id, &handle).await;
        }
    }
}

impl FileHandle {
    fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Opening(left), Self::Opening(right)) => Arc::ptr_eq(left, right),
            (Self::Read(left), Self::Read(right)) => Arc::ptr_eq(left, right),
            (Self::Write(left), Self::Write(right)) => Arc::ptr_eq(left, right),
            (Self::Opening(_), Self::Read(_) | Self::Write(_))
            | (Self::Read(_), Self::Opening(_) | Self::Write(_))
            | (Self::Write(_), Self::Opening(_) | Self::Read(_)) => false,
        }
    }
}

async fn close_file_handle(handle: FileHandle) -> io::Result<()> {
    match handle {
        FileHandle::Opening(entry) => {
            entry.cancellation.cancel();
            entry.completed.cancelled().await;
            Ok(())
        }
        FileHandle::Read(entry) => {
            entry.cancellation.cancel();
            let operation = acquire_operation(&entry.operation).await?;
            let result = entry.handle.close().await;
            drop(operation);
            result
        }
        FileHandle::Write(entry) => {
            entry.cancellation.cancel();
            let operation = acquire_operation(&entry.operation).await?;
            let result = entry.handle.close().await;
            drop(operation);
            result
        }
    }
}

async fn acquire_operation(operation: &Semaphore) -> io::Result<tokio::sync::SemaphorePermit<'_>> {
    operation
        .acquire()
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "file handle is closed"))
}

struct OpeningCompletionGuard(CancellationToken);

impl Drop for OpeningCompletionGuard {
    fn drop(&mut self) {
        self.0.cancel();
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
