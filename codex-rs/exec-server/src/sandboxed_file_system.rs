use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_sandbox::FileSystemSandboxRunner;
use crate::local_file_system::LocalFileSystem;
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

    async fn run_sandboxed(
        &self,
        sandbox: &FileSystemSandboxContext,
        request: FsHelperRequest,
    ) -> FileSystemResult<FsHelperPayload> {
        self.sandbox_runner
            .run(sandbox, request)
            .await
            .map_err(map_sandbox_error)
    }
}

#[async_trait]
impl ExecutorFileSystem for SandboxedFileSystem {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.read_file(path, /*sandbox*/ None).await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::ReadFile(FsReadFileParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
            )
            .await?
            .expect_read_file()
            .map_err(map_sandbox_error)?;
        STANDARD.decode(response.data_base64).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("fs/readFile returned invalid base64 dataBase64: {err}"),
            )
        })
    }

    async fn write_file(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .write_file(path, contents, /*sandbox*/ None)
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::WriteFile(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                sandbox: Some(sandbox.clone()),
            }),
        )
        .await?
        .expect_write_file()
        .map_err(map_sandbox_error)?;
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .create_directory(path, options, /*sandbox*/ None)
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::CreateDirectory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                sandbox: Some(sandbox.clone()),
            }),
        )
        .await?
        .expect_create_directory()
        .map_err(map_sandbox_error)?;
        Ok(())
    }

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self.file_system.get_metadata(path, /*sandbox*/ None).await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::GetMetadata(FsGetMetadataParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
            )
            .await?
            .expect_get_metadata()
            .map_err(map_sandbox_error)?;
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
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .read_directory(path, /*sandbox*/ None)
                .await;
        };
        let response = self
            .run_sandboxed(
                sandbox,
                FsHelperRequest::ReadDirectory(FsReadDirectoryParams {
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                }),
            )
            .await?
            .expect_read_directory()
            .map_err(map_sandbox_error)?;
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

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .remove(path, remove_options, /*sandbox*/ None)
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::Remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(remove_options.recursive),
                force: Some(remove_options.force),
                sandbox: Some(sandbox.clone()),
            }),
        )
        .await?
        .expect_remove()
        .map_err(map_sandbox_error)?;
        Ok(())
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        let Some(sandbox) = sandbox.filter(|sandbox| sandbox.should_run_in_sandbox()) else {
            return self
                .file_system
                .copy(
                    source_path,
                    destination_path,
                    options,
                    /*sandbox*/ None,
                )
                .await;
        };
        self.run_sandboxed(
            sandbox,
            FsHelperRequest::Copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: options.recursive,
                sandbox: Some(sandbox.clone()),
            }),
        )
        .await?
        .expect_copy()
        .map_err(map_sandbox_error)?;
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
