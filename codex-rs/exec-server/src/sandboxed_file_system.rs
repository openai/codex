use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::ExecServerRuntimePaths;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::unexpected_response;
use crate::fs_sandbox::FileSystemSandboxRunner;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsReadDirectoryEntry;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsWriteFileParams;

#[derive(Clone)]
pub struct SandboxedFileSystem {
    file_system: LocalFileSystem,
    sandbox_runner: FileSystemSandboxRunner,
}

impl SandboxedFileSystem {
    pub fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            file_system: LocalFileSystem,
            sandbox_runner: FileSystemSandboxRunner::new(runtime_paths),
        }
    }

    async fn run_sandboxed<T>(
        &self,
        sandbox: &FileSystemSandboxContext,
        request: FsHelperRequest,
        operation: &'static str,
        decode: impl FnOnce(FsHelperPayload) -> Option<T>,
    ) -> FileSystemResult<T> {
        let payload = self
            .sandbox_runner
            .run(sandbox, request)
            .await
            .map_err(map_sandbox_error)?;
        decode(payload).ok_or_else(|| map_sandbox_error(unexpected_response(operation)))
    }
}

#[async_trait]
impl ExecutorFileSystem for SandboxedFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>> {
        self.file_system.read_file(path).await
    }

    async fn read_file_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.read_file(path).await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::ReadFile(FsReadFileParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
                FS_READ_FILE_METHOD,
                FsHelperPayload::into_read_file,
            )
            .await?;
        STANDARD.decode(response.data_base64).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("fs/readFile returned invalid base64 dataBase64: {err}"),
            )
        })
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()> {
        self.file_system.write_file(path, contents).await
    }

    async fn write_file_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.write_file(path, contents).await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::WriteFile(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                sandbox: Some(sandbox.clone()),
            }),
            FS_WRITE_FILE_METHOD,
            FsHelperPayload::into_write_file,
        )
        .await?;
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        self.file_system.create_directory(path, options).await
    }

    async fn create_directory_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .create_directory(path, create_directory_options)
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::CreateDirectory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(create_directory_options.recursive),
                sandbox: Some(sandbox.clone()),
            }),
            FS_CREATE_DIRECTORY_METHOD,
            FsHelperPayload::into_create_directory,
        )
        .await?;
        Ok(())
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata> {
        self.file_system.get_metadata(path).await
    }

    async fn get_metadata_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.get_metadata(path).await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::GetMetadata(FsGetMetadataParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
                FS_GET_METADATA_METHOD,
                FsHelperPayload::into_get_metadata,
            )
            .await?;
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
        self.file_system.read_directory(path).await
    }

    async fn read_directory_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.read_directory(path).await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::ReadDirectory(FsReadDirectoryParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
                FS_READ_DIRECTORY_METHOD,
                FsHelperPayload::into_read_directory,
            )
            .await?;
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
        self.file_system.remove(path, options).await
    }

    async fn remove_with_sandbox(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.remove(path, remove_options).await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::Remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(remove_options.recursive),
                force: Some(remove_options.force),
                sandbox: Some(sandbox.clone()),
            }),
            FS_REMOVE_METHOD,
            FsHelperPayload::into_remove,
        )
        .await?;
        Ok(())
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
    ) -> FileSystemResult<()> {
        self.file_system
            .copy(source_path, destination_path, options)
            .await
    }

    async fn copy_with_sandbox(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .copy(source_path, destination_path, copy_options)
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::Copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: copy_options.recursive,
                sandbox: Some(sandbox.clone()),
            }),
            FS_COPY_METHOD,
            FsHelperPayload::into_copy,
        )
        .await?;
        Ok(())
    }
}

fn map_sandbox_error(error: JSONRPCErrorError) -> io::Error {
    match error.code {
        -32004 => io::Error::new(io::ErrorKind::NotFound, error.message),
        -32600 => io::Error::new(io::ErrorKind::InvalidInput, error.message),
        _ => io::Error::other(error.message),
    }
}
