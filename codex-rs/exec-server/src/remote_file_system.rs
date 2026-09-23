use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_utils_path_uri::PathUri;
use tokio::io;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tracing::trace;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerError;
use crate::ExecutorFileSystem;
use crate::ExecutorFileSystemFuture;
use crate::FileMetadata;
use crate::FileSystemReadStream;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::GetMetadataOptions;
use crate::ReadDirectoryEntry;
use crate::ReadFileOptions;
use crate::RemoveOptions;
use crate::WalkOptions;
use crate::WalkOutcome;
use crate::WriteFileOptions;
use crate::client::LazyRemoteExecServerClient;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsWalkParams;
use crate::protocol::FsWriteFileParams;

const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const NOT_FOUND_ERROR_CODE: i64 = -32004;

#[path = "remote_file_stream.rs"]
mod file_stream;

type InFlightMetadataRequest = OnceCell<Result<FileMetadata, Arc<io::Error>>>;

pub(crate) struct RemoteFileSystem {
    client: LazyRemoteExecServerClient,
    metadata_requests: Mutex<HashMap<PathUri, Arc<InFlightMetadataRequest>>>,
}

impl RemoteFileSystem {
    pub(crate) fn new(client: LazyRemoteExecServerClient) -> Self {
        trace!("remote fs new");
        Self {
            client,
            metadata_requests: Mutex::new(HashMap::new()),
        }
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
                sandbox: sandbox.cloned(),
            })
            .await
            .map_err(map_remote_error)?;
        Ok(response.path)
    }

    async fn read_file(
        &self,
        path: &PathUri,
        options: ReadFileOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        trace!("remote fs read_file");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_read_file(FsReadFileParams {
                path: path.clone(),
                follow_symlinks: (!options.follow_symlinks).then_some(false),
                sandbox: sandbox.cloned(),
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
        trace!("remote fs read_file_stream");
        let client = self.client.get().await.map_err(map_remote_error)?;
        file_stream::open(client, path.clone(), sandbox).await
    }

    async fn write_file(
        &self,
        path: &PathUri,
        contents: Vec<u8>,
        options: WriteFileOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs write_file");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let result = client
            .fs_write_file(FsWriteFileParams {
                path: path.clone(),
                data_base64: STANDARD.encode(contents),
                follow_symlinks: (!options.follow_symlinks).then_some(false),
                sandbox: sandbox.cloned(),
            })
            .await;
        self.metadata_requests.lock().await.clear();
        result.map_err(map_remote_error)?;
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
        let result = client
            .fs_create_directory(FsCreateDirectoryParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                follow_symlinks: (!options.follow_symlinks).then_some(false),
                sandbox: sandbox.cloned(),
            })
            .await;
        self.metadata_requests.lock().await.clear();
        result.map_err(map_remote_error)?;
        Ok(())
    }

    /// Shares identical unsandboxed metadata requests only while their RPC remains in flight.
    async fn get_metadata(
        &self,
        path: &PathUri,
        options: GetMetadataOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        if sandbox.is_some() || !options.follow_symlinks {
            return self.get_metadata_uncached(path, options, sandbox).await;
        }

        let request = {
            let mut requests = self.metadata_requests.lock().await;
            requests.retain(|_, in_flight| Arc::strong_count(in_flight) > 1);
            Arc::clone(requests.entry(path.clone()).or_default())
        };
        let result = match request
            .get_or_init(|| async {
                self.get_metadata_uncached(path, options, /*sandbox*/ None)
                    .await
                    .map_err(Arc::new)
            })
            .await
        {
            Ok(metadata) => Ok(metadata.clone()),
            Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
        };

        let mut requests = self.metadata_requests.lock().await;
        if requests
            .get(path)
            .is_some_and(|in_flight| Arc::ptr_eq(in_flight, &request))
        {
            requests.remove(path);
        }

        result
    }

    /// Sends a fresh metadata request without sharing another caller's sandbox or result.
    async fn get_metadata_uncached(
        &self,
        path: &PathUri,
        options: GetMetadataOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        trace!("remote fs get_metadata");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let response = client
            .fs_get_metadata(FsGetMetadataParams {
                path: path.clone(),
                follow_symlinks: (!options.follow_symlinks).then_some(false),
                sandbox: sandbox.cloned(),
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
                sandbox: sandbox.cloned(),
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

    async fn walk(
        &self,
        path: &PathUri,
        options: WalkOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<WalkOutcome> {
        trace!("remote fs walk");
        let client = self.client.get().await.map_err(map_remote_error)?;
        client
            .fs_walk(FsWalkParams {
                path: path.clone(),
                options,
                sandbox: sandbox.cloned(),
            })
            .await
            .map_err(map_remote_error)
    }

    async fn remove(
        &self,
        path: &PathUri,
        options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        trace!("remote fs remove");
        let client = self.client.get().await.map_err(map_remote_error)?;
        let result = client
            .fs_remove(FsRemoveParams {
                path: path.clone(),
                recursive: Some(options.recursive),
                force: Some(options.force),
                follow_symlinks: (!options.follow_symlinks).then_some(false),
                sandbox: sandbox.cloned(),
            })
            .await;
        self.metadata_requests.lock().await.clear();
        result.map_err(map_remote_error)?;
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
        let result = client
            .fs_copy(FsCopyParams {
                source_path: source_path.clone(),
                destination_path: destination_path.clone(),
                recursive: options.recursive,
                sandbox: sandbox.cloned(),
            })
            .await;
        self.metadata_requests.lock().await.clear();
        result.map_err(map_remote_error)?;
        Ok(())
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
        options: ReadFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(RemoteFileSystem::read_file(self, path, options, sandbox))
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
        options: WriteFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(RemoteFileSystem::write_file(
            self, path, contents, options, sandbox,
        ))
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
        options: GetMetadataOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(RemoteFileSystem::get_metadata(self, path, options, sandbox))
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(RemoteFileSystem::read_directory(self, path, sandbox))
    }

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        Box::pin(RemoteFileSystem::walk(self, path, options, sandbox))
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
}

fn map_remote_error(error: ExecServerError) -> io::Error {
    match error {
        ExecServerError::Server { code, message } if code == NOT_FOUND_ERROR_CODE => {
            io::Error::new(io::ErrorKind::NotFound, message)
        }
        ExecServerError::Server { code, message } if code == INVALID_REQUEST_ERROR_CODE => {
            io::Error::new(io::ErrorKind::InvalidInput, message)
        }
        ExecServerError::Closed | ExecServerError::Disconnected(_) => {
            io::Error::new(io::ErrorKind::BrokenPipe, "exec-server transport closed")
        }
        error if error.is_retryable_preparation_error() => {
            io::Error::new(io::ErrorKind::BrokenPipe, error.to_string())
        }
        ExecServerError::Server { message, .. } => io::Error::other(message),
        _ => io::Error::other(error.to_string()),
    }
}

#[cfg(all(test, any(unix, windows)))]
#[path = "remote_file_system_path_uri_tests.rs"]
mod path_uri_tests;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[test]
    fn preparation_retryability_controls_filesystem_error_kind() {
        let errors = [
            ExecServerError::ConnectionAttempt(Arc::new(
                ExecServerError::EnvironmentRegistryHttp {
                    status: http::StatusCode::SERVICE_UNAVAILABLE,
                    code: None,
                    message: "registry unavailable".to_string(),
                },
            )),
            ExecServerError::Protocol("exec-server transport closed".to_string()),
        ];

        let kinds = errors
            .into_iter()
            .map(|error| map_remote_error(error).kind())
            .collect::<Vec<_>>();

        assert_eq!(kinds, vec![io::ErrorKind::BrokenPipe, io::ErrorKind::Other]);
    }
}
