use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io;
use tracing::trace;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerError;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::client::LazyRemoteExecServerClient;
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
    client: LazyRemoteExecServerClient,
}

impl RemoteFileSystem {
    pub(crate) fn new(client: LazyRemoteExecServerClient) -> Self {
        trace!("remote fs new");
        Self { client }
    }
}

#[async_trait]
impl ExecutorFileSystem for RemoteFileSystem {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        trace!("remote fs read_file");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_read_file(FsReadFileParams {
                path: path.clone(),
                sandbox: remote_sandbox_context(sandbox),
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

    async fn write_file(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs write_file");
        let client = self.client.get().await.map_err(map_remote_error)?;
        client
            .fs_write_file(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs create_directory");
        let client = self.client.get().await.map_err(map_remote_error)?;
        client
            .fs_create_directory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        trace!("remote fs get_metadata");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_get_metadata(FsGetMetadataParams {
                path: path.clone(),
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(FileMetadata {
            is_directory: response.is_directory,
            is_file: response.is_file,
            is_symlink: response.is_symlink,
            created_at_ms: response.created_at_ms,
            modified_at_ms: response.modified_at_ms,
        })
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        trace!("remote fs read_directory");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_read_directory(FsReadDirectoryParams {
                path: path.clone(),
                sandbox: remote_sandbox_context(sandbox),
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

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs remove");
        let client = self.client.get().await.map_err(map_remote_error)?;
        client
            .fs_remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                force: Some(options.force),
                sandbox: remote_sandbox_context(sandbox),
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
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs copy");
        let client = self.client.get().await.map_err(map_remote_error)?;
        client
            .fs_copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: options.recursive,
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(())
    }
}

fn remote_sandbox_context(
    sandbox: Option<&FileSystemSandboxContext>,
) -> Option<FileSystemSandboxContext> {
    sandbox
        .cloned()
        .map(FileSystemSandboxContext::drop_cwd_if_unused)
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
        ExecServerError::Closed | ExecServerError::Disconnected(_) => {
            io::Error::new(io::ErrorKind::BrokenPipe, "exec-server transport closed")
        }
        _ => io::Error::other(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn remote_sandbox_context_drops_unused_cwd() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: absolute_test_path("remote-root"),
            },
            access: FileSystemAccessMode::Read,
        }]);
        let permissions =
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
        let sandbox_context = FileSystemSandboxContext::from_permission_profile_with_cwd(
            permissions,
            absolute_test_path("host-checkout"),
        );

        let remote_context =
            remote_sandbox_context(Some(&sandbox_context)).expect("remote sandbox context");

        assert_eq!(remote_context.cwd, None);
    }

    #[test]
    fn remote_sandbox_context_preserves_required_cwd() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
        }]);
        let permissions =
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
        let cwd = absolute_test_path("host-checkout");
        let sandbox_context =
            FileSystemSandboxContext::from_permission_profile_with_cwd(permissions, cwd.clone());

        let remote_context =
            remote_sandbox_context(Some(&sandbox_context)).expect("remote sandbox context");

        assert_eq!(remote_context.cwd, Some(cwd));
    }

    #[test]
    fn transport_errors_map_to_broken_pipe() {
        let errors = [
            ExecServerError::Closed,
            ExecServerError::Disconnected("exec-server transport disconnected".to_string()),
        ];

        let mapped_errors = errors
            .into_iter()
            .map(|error| {
                let error = map_remote_error(error);
                (error.kind(), error.to_string())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            mapped_errors,
            vec![
                (
                    io::ErrorKind::BrokenPipe,
                    "exec-server transport closed".to_string()
                ),
                (
                    io::ErrorKind::BrokenPipe,
                    "exec-server transport closed".to_string()
                ),
            ]
        );
    }

    fn absolute_test_path(name: &str) -> AbsolutePathBuf {
        let path = std::env::temp_dir().join(name);
        AbsolutePathBuf::from_absolute_path(&path).expect("absolute path")
    }
}
