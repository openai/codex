use std::collections::HashMap;
use std::io;
use std::io::SeekFrom;
use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) const FILE_READ_BLOCK_SIZE: usize = 1024 * 1024;
const MAX_OPEN_FILE_READS: usize = 32;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileReadBlock {
    pub(crate) bytes: Vec<u8>,
    pub(crate) eof: bool,
}

#[derive(Clone, Default)]
pub(crate) struct FileReadHandleManager {
    handles: Arc<Mutex<HashMap<String, mpsc::Sender<FileReadRequest>>>>,
}

struct FileReadRequest {
    offset: u64,
    len: usize,
    response: oneshot::Sender<io::Result<FileReadBlock>>,
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
        let (sender, receiver) = mpsc::channel(1);
        handles.insert(handle_id.clone(), sender);
        tokio::spawn(serve_file_reads(file, receiver));
        Ok(handle_id)
    }

    pub(crate) async fn read_block(
        &self,
        handle_id: &str,
        offset: u64,
        len: usize,
    ) -> io::Result<FileReadBlock> {
        validate_read_block_len(len)?;
        let sender = {
            let handles = self.handles.lock().await;
            handles
                .get(handle_id)
                .cloned()
                .ok_or_else(|| unknown_handle_error(handle_id))?
        };
        let (response, result) = oneshot::channel();
        if sender
            .send(FileReadRequest {
                offset,
                len,
                response,
            })
            .await
            .is_err()
        {
            self.close(handle_id).await;
            return Err(unknown_handle_error(handle_id));
        }
        let result = result.await.map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("file read handle `{handle_id}` stopped unexpectedly"),
            )
        })?;
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

async fn serve_file_reads(
    mut file: tokio::fs::File,
    mut requests: mpsc::Receiver<FileReadRequest>,
) {
    while let Some(request) = requests.recv().await {
        let result = read_block(&mut file, request.offset, request.len).await;
        let should_stop = result.is_err();
        let _ = request.response.send(result);
        if should_stop {
            break;
        }
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
