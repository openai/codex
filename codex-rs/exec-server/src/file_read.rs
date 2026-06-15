use std::collections::HashMap;
use std::io;
use std::io::SeekFrom;
use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::sync::Mutex;
use uuid::Uuid;

pub(crate) const FILE_READ_BLOCK_SIZE: usize = 1024 * 1024;
const MAX_OPEN_FILE_READS: usize = 128;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileReadBlock {
    pub(crate) bytes: Vec<u8>,
    pub(crate) eof: bool,
}

#[derive(Clone, Default)]
pub(crate) struct FileReadHandleManager {
    handles: Arc<Mutex<HashMap<String, tokio::fs::File>>>,
}

impl FileReadHandleManager {
    pub(crate) async fn open(&self, file: tokio::fs::File) -> io::Result<String> {
        let mut handles = self.handles.lock().await;
        if handles.len() >= MAX_OPEN_FILE_READS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("at most {MAX_OPEN_FILE_READS} file reads may be open per connection"),
            ));
        }
        let handle_id = Uuid::new_v4().to_string();
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
        let mut file = {
            let mut handles = self.handles.lock().await;
            handles
                .remove(handle_id)
                .ok_or_else(|| unknown_handle_error(handle_id))?
        };
        let result = read_block(&mut file, offset, len).await;
        if result.as_ref().is_ok_and(|block| !block.eof) {
            self.handles
                .lock()
                .await
                .insert(handle_id.to_string(), file);
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

async fn read_block(
    file: &mut tokio::fs::File,
    offset: u64,
    len: usize,
) -> io::Result<FileReadBlock> {
    file.seek(SeekFrom::Start(offset)).await?;
    let mut bytes = Vec::with_capacity(len);
    file.take(len as u64).read_to_end(&mut bytes).await?;
    Ok(FileReadBlock {
        eof: bytes.len() < len,
        bytes,
    })
}

fn validate_read_block_len(len: usize) -> io::Result<()> {
    if !(1..=FILE_READ_BLOCK_SIZE).contains(&len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("file read block length must be between 1 and {FILE_READ_BLOCK_SIZE}"),
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
