use std::io;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::JSONRPCErrorError;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::FileHandleManager;
use crate::RemoveOptions;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCanonicalizeResponse;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsJoinParams;
use crate::protocol::FsJoinResponse;
use crate::protocol::FsParentParams;
use crate::protocol::FsParentResponse;
use crate::protocol::FsReadDirectoryEntry;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileCloseParams;
use crate::protocol::FsReadFileCloseResponse;
use crate::protocol::FsReadFileOpenParams;
use crate::protocol::FsReadFileOpenResponse;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileReadParams;
use crate::protocol::FsReadFileReadResponse;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsReadFileStatParams;
use crate::protocol::FsReadFileStatResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileCloseParams;
use crate::protocol::FsWriteFileCloseResponse;
use crate::protocol::FsWriteFileOpenParams;
use crate::protocol::FsWriteFileOpenResponse;
use crate::protocol::FsWriteFileParams;
use crate::protocol::FsWriteFileResponse;
use crate::protocol::FsWriteFileWriteParams;
use crate::protocol::FsWriteFileWriteResponse;
use crate::rpc::internal_error;
use crate::rpc::invalid_request;
use crate::rpc::not_found;

#[derive(Clone)]
pub(crate) struct FileSystemHandler {
    file_system: LocalFileSystem,
    handles: FileHandleManager,
}

impl FileSystemHandler {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            file_system: LocalFileSystem::with_runtime_paths(runtime_paths),
            handles: FileHandleManager::default(),
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.handles.close_all().await;
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

    pub(crate) async fn read_file_open(
        &self,
        params: FsReadFileOpenParams,
    ) -> Result<FsReadFileOpenResponse, JSONRPCErrorError> {
        let max_chunk_bytes = self
            .handles
            .open_read(
                std::sync::Arc::new(self.file_system.clone()),
                params.handle_id,
                &params.path,
                params.sandbox.as_ref(),
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsReadFileOpenResponse { max_chunk_bytes })
    }

    pub(crate) async fn read_file_read(
        &self,
        params: FsReadFileReadParams,
    ) -> Result<FsReadFileReadResponse, JSONRPCErrorError> {
        let chunk = self
            .handles
            .read(&params.handle_id, params.offset, params.max_bytes)
            .await
            .map_err(map_fs_error)?;
        Ok(FsReadFileReadResponse {
            data_base64: STANDARD.encode(chunk.data),
            eof: chunk.eof,
        })
    }

    pub(crate) async fn read_file_stat(
        &self,
        params: FsReadFileStatParams,
    ) -> Result<FsReadFileStatResponse, JSONRPCErrorError> {
        let metadata = self
            .handles
            .stat_read(&params.handle_id)
            .await
            .map_err(map_fs_error)?;
        Ok(FsReadFileStatResponse {
            size_bytes: metadata.size_bytes,
            created_at_ms: metadata.created_at_ms,
            modified_at_ms: metadata.modified_at_ms,
        })
    }

    pub(crate) async fn read_file_close(
        &self,
        params: FsReadFileCloseParams,
    ) -> Result<FsReadFileCloseResponse, JSONRPCErrorError> {
        self.handles.close(&params.handle_id).await;
        Ok(FsReadFileCloseResponse {})
    }

    pub(crate) async fn write_file_open(
        &self,
        params: FsWriteFileOpenParams,
    ) -> Result<FsWriteFileOpenResponse, JSONRPCErrorError> {
        let max_chunk_bytes = self
            .handles
            .open_write(
                std::sync::Arc::new(self.file_system.clone()),
                params.handle_id,
                &params.path,
                params.sandbox.as_ref(),
            )
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileOpenResponse { max_chunk_bytes })
    }

    pub(crate) async fn write_file_write(
        &self,
        params: FsWriteFileWriteParams,
    ) -> Result<FsWriteFileWriteResponse, JSONRPCErrorError> {
        let data = STANDARD.decode(params.data_base64).map_err(|err| {
            invalid_request(format!(
                "fs/writeFile/write requires valid base64 dataBase64: {err}"
            ))
        })?;
        self.handles
            .write(&params.handle_id, &data)
            .await
            .map_err(map_fs_error)?;
        Ok(FsWriteFileWriteResponse {})
    }

    pub(crate) async fn write_file_close(
        &self,
        params: FsWriteFileCloseParams,
    ) -> Result<FsWriteFileCloseResponse, JSONRPCErrorError> {
        self.handles.close(&params.handle_id).await;
        Ok(FsWriteFileCloseResponse {})
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

    pub(crate) async fn canonicalize(
        &self,
        params: FsCanonicalizeParams,
    ) -> Result<FsCanonicalizeResponse, JSONRPCErrorError> {
        let path = self
            .file_system
            .canonicalize(&params.path, params.sandbox.as_ref())
            .await
            .map_err(map_fs_error)?;
        Ok(FsCanonicalizeResponse { path })
    }

    pub(crate) async fn join(
        &self,
        params: FsJoinParams,
    ) -> Result<FsJoinResponse, JSONRPCErrorError> {
        let path = self
            .file_system
            .join(&params.base_path, &params.path)
            .await
            .map_err(map_fs_error)?;
        Ok(FsJoinResponse { path })
    }

    pub(crate) async fn parent(
        &self,
        params: FsParentParams,
    ) -> Result<FsParentResponse, JSONRPCErrorError> {
        let path = self
            .file_system
            .parent(&params.path)
            .await
            .map_err(map_fs_error)?;
        Ok(FsParentResponse { path })
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
    use crate::protocol::FsReadFileCloseParams;
    use crate::protocol::FsReadFileOpenParams;
    use crate::protocol::FsReadFileParams;
    use crate::protocol::FsReadFileReadParams;
    use crate::protocol::FsReadFileStatParams;
    use crate::protocol::FsWriteFileCloseParams;
    use crate::protocol::FsWriteFileOpenParams;
    use crate::protocol::FsWriteFileParams;
    use crate::protocol::FsWriteFileWriteParams;

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

    #[tokio::test]
    async fn streamed_file_operations_are_positional_and_write_directly() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let handler = FileSystemHandler::new(runtime_paths);
        let path =
            AbsolutePathBuf::from_absolute_path(temp_dir.path().join("stream.txt").as_path())
                .expect("absolute path");
        let linked_path = temp_dir.path().join("linked.txt");
        std::fs::write(&path, "old").expect("write initial file");
        std::fs::hard_link(&path, &linked_path).expect("create hard link");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&path)
                .expect("stat initial file")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions).expect("make initial file executable");
        }

        handler
            .write_file_open(FsWriteFileOpenParams {
                handle_id: "write-1".to_string(),
                path: path.clone(),
                sandbox: None,
            })
            .await
            .expect("open write handle");
        assert_eq!(std::fs::read_to_string(&path).expect("read file"), "");
        assert_eq!(
            std::fs::read_to_string(&linked_path).expect("read hard link"),
            ""
        );
        handler
            .write_file_write(FsWriteFileWriteParams {
                handle_id: "write-1".to_string(),
                data_base64: STANDARD.encode("new"),
            })
            .await
            .expect("write chunk");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read written file"),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(&linked_path).expect("read written hard link"),
            "new"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::fs::PermissionsExt;

            let metadata = std::fs::metadata(&path).expect("stat written file");
            let linked_metadata = std::fs::metadata(&linked_path).expect("stat written hard link");
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                (linked_metadata.dev(), linked_metadata.ino())
            );
            assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
        }
        handler
            .write_file_close(FsWriteFileCloseParams {
                handle_id: "write-1".to_string(),
            })
            .await
            .expect("close write handle");

        handler
            .read_file_open(FsReadFileOpenParams {
                handle_id: "read-1".to_string(),
                path,
                sandbox: None,
            })
            .await
            .expect("open read handle");
        let stat = handler
            .read_file_stat(FsReadFileStatParams {
                handle_id: "read-1".to_string(),
            })
            .await
            .expect("stat read handle");
        assert_eq!(stat.size_bytes, 3);
        let read = handler
            .read_file_read(FsReadFileReadParams {
                handle_id: "read-1".to_string(),
                offset: 1,
                max_bytes: Some(2),
            })
            .await
            .expect("read chunk");
        assert_eq!(read.data_base64, STANDARD.encode("ew"));
        assert!(read.eof);
        handler
            .read_file_close(FsReadFileCloseParams {
                handle_id: "read-1".to_string(),
            })
            .await
            .expect("close read handle");
    }
}
