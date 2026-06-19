use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_utils_path_uri::PathUri;
use tokio::io;
use tracing::trace;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerError;
use crate::ExecutorFileSystem;
use crate::ExecutorFileSystemFuture;
use crate::FileMetadata;
use crate::FileSystemOperation;
use crate::FileSystemOperationOutput;
use crate::FileSystemOperationResult;
use crate::FileSystemReadStream;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use crate::client::LazyRemoteExecServerClient;
use crate::connection::MAX_RPC_BATCH_REQUESTS;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCanonicalizeResponse;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsWriteFileParams;
use crate::rpc::RpcBatchCall;
use codex_file_system::execute_batch_with_scalar_operations;
use serde::Serialize;
use serde::de::DeserializeOwned;

const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const NOT_FOUND_ERROR_CODE: i64 = -32004;

#[path = "remote_file_stream.rs"]
mod file_stream;

pub(crate) struct RemoteFileSystem {
    client: LazyRemoteExecServerClient,
}

impl RemoteFileSystem {
    pub(crate) fn new(client: LazyRemoteExecServerClient) -> Self {
        trace!("remote fs new");
        Self { client }
    }

    async fn canonicalize(
        &self,
        path: &PathUri,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<PathUri> {
        trace!("remote fs canonicalize");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_canonicalize(FsCanonicalizeParams {
                path: path.clone(),
                sandbox: remote_sandbox_context(sandbox),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(response.path)
    }

    async fn read_file(
        &self,
        path: &PathUri,
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

    async fn read_file_stream(
        &self,
        path: &PathUri,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileSystemReadStream> {
        if sandbox.is_some_and(FileSystemSandboxContext::should_run_in_sandbox) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "streaming file reads do not support platform sandboxing",
            ));
        }
        trace!("remote fs read_file_stream");
        let client = self.client.get().await.map_err(map_remote_error)?;
        file_stream::open(client, path.clone(), remote_sandbox_context(sandbox)).await
    }

    async fn write_file(
        &self,
        path: &PathUri,
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
        path: &PathUri,
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
        path: &PathUri,
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
            size: response.size,
            created_at_ms: response.created_at_ms,
            modified_at_ms: response.modified_at_ms,
        })
    }

    async fn read_directory(
        &self,
        path: &PathUri,
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
        path: &PathUri,
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
        source_path: &PathUri,
        destination_path: &PathUri,
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

    async fn execute_batch(
        &self,
        operations: Vec<FileSystemOperation>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<FileSystemOperationResult>> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }

        let client = self.client.get().await.map_err(map_remote_error)?;
        if !client.supports_rpc_batch() {
            return execute_batch_with_scalar_operations(self, operations, sandbox).await;
        }

        let sandbox = remote_sandbox_context(sandbox);
        let mut output_kinds = Vec::with_capacity(operations.len());
        let mut calls = Vec::with_capacity(operations.len());
        for operation in operations {
            let (output_kind, call) = remote_batch_call(operation, sandbox.clone())?;
            output_kinds.push(output_kind);
            calls.push(call);
        }

        let mut decoded = Vec::with_capacity(output_kinds.len());
        let mut output_kinds = output_kinds.into_iter();
        let mut calls = calls.into_iter();
        loop {
            let output_kind_chunk = output_kinds
                .by_ref()
                .take(MAX_RPC_BATCH_REQUESTS)
                .collect::<Vec<_>>();
            if output_kind_chunk.is_empty() {
                break;
            }
            let call_chunk = calls
                .by_ref()
                .take(MAX_RPC_BATCH_REQUESTS)
                .collect::<Vec<_>>();
            let results = client
                .call_batch(call_chunk)
                .await
                .map_err(map_remote_error)?;
            if results.len() != output_kind_chunk.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "exec-server returned {} batch results for {} operations",
                        results.len(),
                        output_kind_chunk.len()
                    ),
                ));
            }
            decoded.extend(
                output_kind_chunk
                    .into_iter()
                    .zip(results)
                    .map(|(output_kind, result)| decode_remote_batch_result(output_kind, result)),
            );
        }
        Ok(decoded)
    }
}

impl ExecutorFileSystem for RemoteFileSystem {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        Box::pin(RemoteFileSystem::canonicalize(self, path, sandbox))
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(RemoteFileSystem::read_file(self, path, sandbox))
    }

    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        Box::pin(RemoteFileSystem::read_file_stream(self, path, sandbox))
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(RemoteFileSystem::write_file(self, path, contents, sandbox))
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(RemoteFileSystem::create_directory(
            self, path, options, sandbox,
        ))
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(RemoteFileSystem::get_metadata(self, path, sandbox))
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(RemoteFileSystem::read_directory(self, path, sandbox))
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(RemoteFileSystem::remove(self, path, options, sandbox))
    }

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(RemoteFileSystem::copy(
            self,
            source_path,
            destination_path,
            options,
            sandbox,
        ))
    }

    fn execute_batch<'a>(
        &'a self,
        operations: Vec<FileSystemOperation>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<FileSystemOperationResult>> {
        Box::pin(RemoteFileSystem::execute_batch(self, operations, sandbox))
    }
}

#[derive(Clone, Copy)]
enum RemoteBatchOutputKind {
    Canonicalize,
    ReadFile,
    GetMetadata,
    ReadDirectory,
}

fn remote_batch_call(
    operation: FileSystemOperation,
    sandbox: Option<FileSystemSandboxContext>,
) -> io::Result<(RemoteBatchOutputKind, RpcBatchCall)> {
    match operation {
        FileSystemOperation::Canonicalize { path } => Ok((
            RemoteBatchOutputKind::Canonicalize,
            rpc_batch_call(
                crate::protocol::FS_CANONICALIZE_METHOD,
                FsCanonicalizeParams { path, sandbox },
            )?,
        )),
        FileSystemOperation::ReadFile { path } => Ok((
            RemoteBatchOutputKind::ReadFile,
            rpc_batch_call(
                crate::protocol::FS_READ_FILE_METHOD,
                FsReadFileParams { path, sandbox },
            )?,
        )),
        FileSystemOperation::GetMetadata { path } => Ok((
            RemoteBatchOutputKind::GetMetadata,
            rpc_batch_call(
                crate::protocol::FS_GET_METADATA_METHOD,
                FsGetMetadataParams { path, sandbox },
            )?,
        )),
        FileSystemOperation::ReadDirectory { path } => Ok((
            RemoteBatchOutputKind::ReadDirectory,
            rpc_batch_call(
                crate::protocol::FS_READ_DIRECTORY_METHOD,
                FsReadDirectoryParams { path, sandbox },
            )?,
        )),
    }
}

fn rpc_batch_call(method: &str, params: impl Serialize) -> io::Result<RpcBatchCall> {
    Ok(RpcBatchCall {
        method: method.to_string(),
        params: serde_json::to_value(params)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    })
}

fn decode_remote_batch_result(
    output_kind: RemoteBatchOutputKind,
    result: Result<serde_json::Value, ExecServerError>,
) -> FileSystemOperationResult {
    let value = result.map_err(map_remote_error)?;
    match output_kind {
        RemoteBatchOutputKind::Canonicalize => {
            let response: FsCanonicalizeResponse = decode_batch_value(value)?;
            Ok(FileSystemOperationOutput::Canonicalize(response.path))
        }
        RemoteBatchOutputKind::ReadFile => {
            let response: FsReadFileResponse = decode_batch_value(value)?;
            let contents = STANDARD.decode(response.data_base64).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("remote fs/readFile returned invalid base64 dataBase64: {error}"),
                )
            })?;
            Ok(FileSystemOperationOutput::ReadFile(contents))
        }
        RemoteBatchOutputKind::GetMetadata => {
            let response: FsGetMetadataResponse = decode_batch_value(value)?;
            Ok(FileSystemOperationOutput::GetMetadata(FileMetadata {
                is_directory: response.is_directory,
                is_file: response.is_file,
                is_symlink: response.is_symlink,
                size: response.size,
                created_at_ms: response.created_at_ms,
                modified_at_ms: response.modified_at_ms,
            }))
        }
        RemoteBatchOutputKind::ReadDirectory => {
            let response: crate::protocol::FsReadDirectoryResponse = decode_batch_value(value)?;
            Ok(FileSystemOperationOutput::ReadDirectory(
                response
                    .entries
                    .into_iter()
                    .map(|entry| ReadDirectoryEntry {
                        file_name: entry.file_name,
                        is_directory: entry.is_directory,
                        is_file: entry.is_file,
                    })
                    .collect(),
            ))
        }
    }
}

fn decode_batch_value<T: DeserializeOwned>(value: serde_json::Value) -> io::Result<T> {
    serde_json::from_value(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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

#[cfg(all(test, any(unix, windows)))]
#[path = "remote_file_system_path_uri_tests.rs"]
mod path_uri_tests;

#[cfg(test)]
mod tests {
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
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
            path_uri("host-checkout"),
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
        let cwd = path_uri("host-checkout");
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

    fn path_uri(name: &str) -> PathUri {
        PathUri::from_abs_path(&absolute_test_path(name))
    }
}
