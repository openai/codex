use bytes::Bytes;
use codex_utils_path_uri::PathUri;
use tokio::io;
use uuid::Uuid;

use super::map_remote_error;
use crate::ExecServerClient;
use crate::FILE_READ_CHUNK_SIZE;
use crate::FileSystemReadStream;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::protocol::FS_READ_BLOCK_METHOD;
use crate::protocol::FsCloseParams;
use crate::protocol::FsOpenParams;
use crate::protocol::FsReadBlockParams;

struct FileReadRegistration {
    client: ExecServerClient,
    handle_id: String,
    runtime: Option<tokio::runtime::Handle>,
    active: bool,
}

pub(super) async fn open(
    client: ExecServerClient,
    path: PathUri,
    sandbox: Option<FileSystemSandboxContext>,
) -> FileSystemResult<FileSystemReadStream> {
    let registration = FileReadRegistration {
        client,
        handle_id: Uuid::new_v4().simple().to_string(),
        runtime: tokio::runtime::Handle::try_current().ok(),
        active: true,
    };
    registration
        .client
        .fs_open(FsOpenParams {
            handle_id: registration.handle_id.clone(),
            path,
            sandbox,
        })
        .await
        .map_err(map_remote_error)?;
    Ok(FileSystemReadStream::new(futures::stream::try_unfold(
        Some((registration, 0_u64)),
        |state| async move {
            let Some((mut registration, offset)) = state else {
                return Ok(None);
            };
            let response = registration
                .client
                .fs_read_block(FsReadBlockParams {
                    handle_id: registration.handle_id.clone(),
                    offset,
                    len: FILE_READ_CHUNK_SIZE,
                })
                .await
                .map_err(map_remote_error)?;
            let chunk = Bytes::from(response.chunk.into_inner());
            if chunk.len() > FILE_READ_CHUNK_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{FS_READ_BLOCK_METHOD} returned {} bytes, maximum is {}",
                        chunk.len(),
                        FILE_READ_CHUNK_SIZE
                    ),
                ));
            }
            if response.eof {
                if registration
                    .client
                    .fs_close(FsCloseParams {
                        handle_id: registration.handle_id.clone(),
                    })
                    .await
                    .is_ok()
                {
                    registration.active = false;
                }
                return if chunk.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some((chunk, None)))
                };
            }
            if chunk.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{FS_READ_BLOCK_METHOD} returned an empty non-terminal block"),
                ));
            }
            let next_offset = offset.checked_add(chunk.len() as u64).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{FS_READ_BLOCK_METHOD} offset overflowed after {offset} bytes"),
                )
            })?;
            Ok(Some((chunk, Some((registration, next_offset)))))
        },
    )))
}

impl Drop for FileReadRegistration {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let client = self.client.clone();
        let handle_id = self.handle_id.clone();
        let runtime = self
            .runtime
            .clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok());
        if let Some(runtime) = runtime {
            runtime.spawn(async move {
                let _ = client.fs_close(FsCloseParams { handle_id }).await;
            });
        }
    }
}
