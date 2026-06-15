use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::sync::Arc;

use codex_file_system::FILE_READ_CHUNK_SIZE;
use tokio::sync::Mutex;

const MAX_OPEN_FILE_READS: usize = 128;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileReadBlock {
    pub(crate) bytes: Vec<u8>,
    pub(crate) eof: bool,
}

#[derive(Clone, Default)]
pub(crate) struct FileReadHandleManager {
    handles: Arc<Mutex<HashMap<String, Arc<File>>>>,
}

impl FileReadHandleManager {
    pub(crate) async fn open(
        &self,
        handle_id: String,
        file: tokio::fs::File,
    ) -> io::Result<String> {
        let file = Arc::new(file.into_std().await);
        let mut handles = self.handles.lock().await;
        if handles.contains_key(&handle_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("file read handle `{handle_id}` already exists"),
            ));
        }
        if handles.len() >= MAX_OPEN_FILE_READS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("at most {MAX_OPEN_FILE_READS} file reads may be open per connection"),
            ));
        }
        handles.insert(handle_id.clone(), file);
        Ok(handle_id)
    }

    pub(crate) async fn read_block(
        &self,
        handle_id: &str,
        offset: u64,
        len: usize,
    ) -> io::Result<FileReadBlock> {
        validate_read_block_len(len)?;
        let file = {
            let handles = self.handles.lock().await;
            handles
                .get(handle_id)
                .cloned()
                .ok_or_else(|| unknown_handle_error(handle_id))?
        };
        let result =
            match tokio::task::spawn_blocking(move || read_block_at(&file, offset, len)).await {
                Ok(result) => result,
                Err(error) => Err(io::Error::other(format!(
                    "file read task stopped unexpectedly: {error}"
                ))),
            };
        if result.is_err() || result.as_ref().is_ok_and(|block| block.eof) {
            self.close(handle_id).await;
        }
        result
    }

    pub(crate) async fn close(&self, handle_id: &str) {
        self.handles.lock().await.remove(handle_id);
    }

    pub(crate) async fn close_all(&self) {
        self.handles.lock().await.clear();
    }
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
