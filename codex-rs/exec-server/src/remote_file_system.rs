use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io;
use tracing::trace;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsWriteFileParams;

const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const NOT_FOUND_ERROR_CODE: i64 = -32004;

#[derive(Clone)]
pub(crate) struct RemoteFileSystem {
    client: ExecServerClient,
}

impl RemoteFileSystem {
    pub(crate) fn new(client: ExecServerClient) -> Self {
        trace!("remote fs new");
        Self { client }
    }
}

#[async_trait]
impl ExecutorFileSystem for RemoteFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>> {
        trace!("remote fs read_file");
        let response = self
            .client
            .fs_read_file(FsReadFileParams {
                path: path.clone(),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        STANDARD.decode(response.data_base64).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("remote fs/readFile returned invalid base64 dataBase64: {err}"),
            )
        })
    }

    async fn read_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<Vec<u8>> {
        trace!("remote fs read_file_with_sandbox_policy");
        let response = self
            .client
            .fs_read_file(FsReadFileParams {
                path: path.clone(),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        STANDARD.decode(response.data_base64).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("remote fs/readFile returned invalid base64 dataBase64: {err}"),
            )
        })
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()> {
        trace!("remote fs write_file");
        self.client
            .fs_write_file(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn write_file_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<()> {
        trace!("remote fs write_file_with_sandbox_policy");
        self.client
            .fs_write_file(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        trace!("remote fs create_directory");
        self.client
            .fs_create_directory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn create_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<()> {
        trace!("remote fs create_directory_with_sandbox_policy");
        self.client
            .fs_create_directory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(create_directory_options.recursive),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata> {
        trace!("remote fs get_metadata");
        let response = self
            .client
            .fs_get_metadata(FsGetMetadataParams {
                path: path.clone(),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(FileMetadata {
            is_directory: response.is_directory,
            is_file: response.is_file,
            created_at_ms: response.created_at_ms,
            modified_at_ms: response.modified_at_ms,
        })
    }

    async fn get_metadata_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<FileMetadata> {
        trace!("remote fs get_metadata_with_sandbox_policy");
        let response = self
            .client
            .fs_get_metadata(FsGetMetadataParams {
                path: path.clone(),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(FileMetadata {
            is_directory: response.is_directory,
            is_file: response.is_file,
            created_at_ms: response.created_at_ms,
            modified_at_ms: response.modified_at_ms,
        })
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        trace!("remote fs read_directory");
        let response = self
            .client
            .fs_read_directory(FsReadDirectoryParams {
                path: path.clone(),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(response
            .entries
            .into_iter()
            .map(|entry| ReadDirectoryEntry {
                file_name: entry.file_name,
                is_directory: entry.is_directory,
                is_file: entry.is_file,
            })
            .collect())
    }

    async fn read_directory_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        trace!("remote fs read_directory_with_sandbox_policy");
        let response = self
            .client
            .fs_read_directory(FsReadDirectoryParams {
                path: path.clone(),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(response
            .entries
            .into_iter()
            .map(|entry| ReadDirectoryEntry {
                file_name: entry.file_name,
                is_directory: entry.is_directory,
                is_file: entry.is_file,
            })
            .collect())
    }

    async fn remove(&self, path: &AbsolutePathBuf, options: RemoveOptions) -> FileSystemResult<()> {
        trace!("remote fs remove");
        self.client
            .fs_remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                force: Some(options.force),
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn remove_with_sandbox_policy(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<()> {
        trace!("remote fs remove_with_sandbox_policy");
        self.client
            .fs_remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(remove_options.recursive),
                force: Some(remove_options.force),
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()> {
        trace!("remote fs copy");
        self.client
            .fs_copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: options.recursive,
                sandbox_policy: None,
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn copy_with_sandbox_policy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
        sandbox_policy: Option<&SandboxPolicy>,
    ) -> FileSystemResult<()> {
        trace!("remote fs copy_with_sandbox_policy");
        self.client
            .fs_copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: copy_options.recursive,
                sandbox_policy: sandbox_policy.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }
}

fn map_remote_error(error: ExecServerError) -> io::Error {
    match error {
        ExecServerError::Server { code, message } if code == NOT_FOUND_ERROR_CODE => {
            io::Error::new(io::ErrorKind::NotFound, message)
        }
        ExecServerError::Server { code, message } if code == INVALID_REQUEST_ERROR_CODE => {
            io::Error::new(io::ErrorKind::InvalidInput, message)
        }
        ExecServerError::Server { message, .. } => io::Error::other(message),
        ExecServerError::Closed => {
            io::Error::new(io::ErrorKind::BrokenPipe, "exec-server transport closed")
        }
        _ => io::Error::other(error.to_string()),
    }
}
