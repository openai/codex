use std::io;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::RemoveOptions;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryEntry;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileParams;
use crate::protocol::FsWriteFileResponse;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;
use crate::rpc::not_found;

#[derive(Clone)]
pub(crate) struct FileSystemHandler {
    file_system: LocalFileSystem,
}

impl FileSystemHandler {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            file_system: LocalFileSystem::with_runtime_paths(runtime_paths),
        }
    }

    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let bytes = self
            .file_system
            .read_file(&params.path, params.sandbox.as_ref())
            .await
            .map_err(map_fs_error)?;
        Ok(FsReadFileResponse {
            data_base64: STANDARD.encode(bytes),
        })
    }

    pub(crate) async fn write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, JSONRPCErrorError> {
        let bytes = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "{FS_WRITE_FILE_METHOD} requires valid base64 dataBase64: {err}"
            ))
        })?;
        self.file_system
            .write_file(&params.path, bytes, params.sandbox.as_ref())
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        let recursive = params.recursive.unwrap_or(true);
        self.file_system
            .create_directory(
                &params.path,
                CreateDirectoryOptions { recursive },
                params.sandbox.as_ref(),
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCreateDirectoryResponse {})
    }

    pub(crate) async fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        let metadata = self
            .file_system
            .get_metadata(&params.path, params.sandbox.as_ref())
            .await
            .map_err(map_fs_error)?;
        Ok(FsGetMetadataResponse {
            is_directory: metadata.is_directory,
            is_file: metadata.is_file,
            is_symlink: metadata.is_symlink,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        let entries = self
            .file_system
            .read_directory(&params.path, params.sandbox.as_ref())
            .await
            .map_err(map_fs_error)?
            .into_iter()
            .map(|entry| FsReadDirectoryEntry {
                file_name: entry.file_name,
                is_directory: entry.is_directory,
                is_file: entry.is_file,
            })
            .collect();
        Ok(FsReadDirectoryResponse { entries })
    }

    pub(crate) async fn remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, JSONRPCErrorError> {
        let recursive = params.recursive.unwrap_or(true);
        let force = params.force.unwrap_or(true);
        self.file_system
            .remove(
                &params.path,
                RemoveOptions { recursive, force },
                params.sandbox.as_ref(),
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsRemoveResponse {})
    }

    pub(crate) async fn copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        self.file_system
            .copy(
                &params.source_path,
                &params.destination_path,
                CopyOptions {
                    recursive: params.recursive,
                },
                params.sandbox.as_ref(),
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsCopyResponse {})
    }
}

fn map_fs_error(err: io::Error) -> JSONRPCErrorError {
    match err.kind() {
        io::ErrorKind::NotFound => not_found(err.to_string()),
        io::ErrorKind::InvalidInput | io::ErrorKind::PermissionDenied => {
            invalid_request(err.to_string())
        }
        _ => internal_error(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::NetworkAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::FileSystemSandboxContext;
    use crate::protocol::FsReadFileParams;
    use crate::protocol::FsWriteFileParams;

    #[tokio::test]
    async fn no_platform_sandbox_policies_do_not_require_configured_sandbox_helper() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let handler = FileSystemHandler::new(runtime_paths);
        let sandbox_cwd =
            AbsolutePathBuf::from_absolute_path(temp_dir.path()).expect("absolute tempdir");

        for (file_name, sandbox_policy) in [
            ("danger.txt", SandboxPolicy::DangerFullAccess),
            (
                "external.txt",
                SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                },
            ),
        ] {
            let path =
                AbsolutePathBuf::from_absolute_path(temp_dir.path().join(file_name).as_path())
                    .expect("absolute path");

            handler
                .write_file(FsWriteFileParams {
                    path: path.clone(),
                    data_base64: STANDARD.encode("ok"),
                    sandbox: Some(FileSystemSandboxContext::from_legacy_sandbox_policy(
                        sandbox_policy.clone(),
                        sandbox_cwd.clone(),
                    )),
                })
                .await
                .expect("write file");

            let response = handler
                .read_file(FsReadFileParams {
                    path,
                    sandbox: Some(FileSystemSandboxContext::from_legacy_sandbox_policy(
                        sandbox_policy,
                        sandbox_cwd.clone(),
                    )),
                })
                .await
                .expect("read file");

            assert_eq!(response.data_base64, STANDARD.encode("ok"));
        }
    }
}
