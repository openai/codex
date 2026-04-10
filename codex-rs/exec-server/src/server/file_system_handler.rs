use std::io;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_protocol::protocol::SandboxPolicy;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::RemoveOptions;
use crate::fs_helper::FsHelperPayload;
use crate::fs_helper::FsHelperRequest;
use crate::fs_helper::unexpected_response;
use crate::fs_sandbox::FileSystemSandboxRunner;
use crate::local_file_system::LocalFileSystem;
use crate::local_file_system::enforce_copy_source_read_access;
use crate::local_file_system::enforce_read_access;
use crate::local_file_system::enforce_write_access;
use crate::local_file_system::enforce_write_access_preserving_leaf;
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
    sandbox_runner: FileSystemSandboxRunner,
}

impl FileSystemHandler {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            file_system: LocalFileSystem,
            sandbox_runner: FileSystemSandboxRunner::new(runtime_paths),
        }
    }

    async fn run_sandboxed<T>(
        &self,
        sandbox_policy: &SandboxPolicy,
        access_check: io::Result<()>,
        request: FsHelperRequest,
        operation: &'static str,
        decode: impl FnOnce(FsHelperPayload) -> Option<T>,
    ) -> Result<T, JSONRPCErrorError> {
        access_check.map_err(map_fs_error)?;
        let payload = self.sandbox_runner.run(sandbox_policy, request).await?;
        decode(payload).ok_or_else(|| unexpected_response(operation))
    }

    pub(crate) async fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, JSONRPCErrorError> {
        let bytes = match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_read_access(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::ReadFile { path: params.path },
                    "readFile",
                    |payload| match payload {
                        FsHelperPayload::ReadFile { data } => Some(data.into_inner()),
                        _ => None,
                    },
                )
                .await?
            }
            None => self
                .file_system
                .read_file(&params.path)
                .await
                .map_err(map_fs_error)?,
        };
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
                "fs/writeFile requires valid base64 dataBase64: {err}"
            ))
        })?;
        match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_write_access(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::WriteFile {
                        path: params.path,
                        data: bytes.into(),
                    },
                    "writeFile",
                    |payload| match payload {
                        FsHelperPayload::WriteFile => Some(()),
                        _ => None,
                    },
                )
                .await?;
            }
            None => self
                .file_system
                .write_file(&params.path, bytes)
                .await
                .map_err(map_fs_error)?,
        }
        Ok(FsWriteFileResponse {})
    }

    pub(crate) async fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, JSONRPCErrorError> {
        let recursive = params.recursive.unwrap_or(true);
        match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_write_access(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::CreateDirectory {
                        path: params.path,
                        recursive,
                    },
                    "createDirectory",
                    |payload| match payload {
                        FsHelperPayload::CreateDirectory => Some(()),
                        _ => None,
                    },
                )
                .await?;
            }
            None => self
                .file_system
                .create_directory(&params.path, CreateDirectoryOptions { recursive })
                .await
                .map_err(map_fs_error)?,
        }
        Ok(FsCreateDirectoryResponse {})
    }

    pub(crate) async fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, JSONRPCErrorError> {
        let metadata = match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_read_access(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::GetMetadata { path: params.path },
                    "getMetadata",
                    |payload| match payload {
                        FsHelperPayload::GetMetadata {
                            is_directory,
                            is_file,
                            created_at_ms,
                            modified_at_ms,
                        } => Some(FileMetadata {
                            is_directory,
                            is_file,
                            created_at_ms,
                            modified_at_ms,
                        }),
                        _ => None,
                    },
                )
                .await?
            }
            None => self
                .file_system
                .get_metadata(&params.path)
                .await
                .map_err(map_fs_error)?,
        };
        Ok(FsGetMetadataResponse {
            is_directory: metadata.is_directory,
            is_file: metadata.is_file,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, JSONRPCErrorError> {
        let entries = match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_read_access(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::ReadDirectory { path: params.path },
                    "readDirectory",
                    |payload| match payload {
                        FsHelperPayload::ReadDirectory { entries } => Some(entries),
                        _ => None,
                    },
                )
                .await?
            }
            None => self
                .file_system
                .read_directory(&params.path)
                .await
                .map_err(map_fs_error)?
                .into_iter()
                .map(|entry| FsReadDirectoryEntry {
                    file_name: entry.file_name,
                    is_directory: entry.is_directory,
                    is_file: entry.is_file,
                })
                .collect(),
        };
        Ok(FsReadDirectoryResponse { entries })
    }

    pub(crate) async fn remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, JSONRPCErrorError> {
        let recursive = params.recursive.unwrap_or(true);
        let force = params.force.unwrap_or(true);
        match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                self.run_sandboxed(
                    sandbox_policy,
                    enforce_write_access_preserving_leaf(&params.path, Some(sandbox_policy)),
                    FsHelperRequest::Remove {
                        path: params.path,
                        recursive,
                        force,
                    },
                    "remove",
                    |payload| match payload {
                        FsHelperPayload::Remove => Some(()),
                        _ => None,
                    },
                )
                .await?;
            }
            None => self
                .file_system
                .remove(&params.path, RemoveOptions { recursive, force })
                .await
                .map_err(map_fs_error)?,
        }
        Ok(FsRemoveResponse {})
    }

    pub(crate) async fn copy(
        &self,
        params: FsCopyParams,
    ) -> Result<FsCopyResponse, JSONRPCErrorError> {
        match fs_sandbox_policy(params.sandbox_policy.as_ref()) {
            Some(sandbox_policy) => {
                let access_check =
                    enforce_copy_source_read_access(&params.source_path, Some(sandbox_policy))
                        .and_then(|()| {
                            enforce_write_access(&params.destination_path, Some(sandbox_policy))
                        });
                self.run_sandboxed(
                    sandbox_policy,
                    access_check,
                    FsHelperRequest::Copy {
                        source_path: params.source_path,
                        destination_path: params.destination_path,
                        recursive: params.recursive,
                    },
                    "copy",
                    |payload| match payload {
                        FsHelperPayload::Copy => Some(()),
                        _ => None,
                    },
                )
                .await?;
            }
            None => self
                .file_system
                .copy(
                    &params.source_path,
                    &params.destination_path,
                    CopyOptions {
                        recursive: params.recursive,
                    },
                )
                .await
                .map_err(map_fs_error)?,
        }
        Ok(FsCopyResponse {})
    }
}

fn fs_sandbox_policy(sandbox_policy: Option<&SandboxPolicy>) -> Option<&SandboxPolicy> {
    match sandbox_policy {
        Some(policy @ (SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. })) => {
            Some(policy)
        }
        Some(SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }) | None => {
            None
        }
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
    use crate::protocol::FsReadFileParams;
    use crate::protocol::FsWriteFileParams;

    #[tokio::test]
    async fn no_platform_sandbox_policies_do_not_require_configured_sandbox_helper() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let handler = FileSystemHandler::new(ExecServerRuntimePaths::default());

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
                    sandbox_policy: Some(sandbox_policy.clone()),
                })
                .await
                .expect("write file");

            let response = handler
                .read_file(FsReadFileParams {
                    path,
                    sandbox_policy: Some(sandbox_policy),
                })
                .await
                .expect("read file");

            assert_eq!(response.data_base64, STANDARD.encode("ok"));
        }
    }
}
